//! NAL unit parser

use std::{marker::PhantomData, sync::Arc};

use media_core::{
    buffer::{Buffer, BufferPool},
    error::Error,
    Result,
};

use crate::header::NalHeader;

/// Internal storage for NAL unit payload (RBSP with EPB removed)
#[derive(Debug)]
enum NalPayload<'a> {
    /// Reference to original data (no EPB, zero-copy)
    Borrowed(&'a [u8]),
    /// Owned Vec with EPB removed
    Owned(Vec<u8>),
    /// Owned buffer from pool with EPB removed
    Buffer(Arc<Buffer>),
}

impl NalPayload<'_> {
    /// Get the payload as a slice
    #[inline]
    fn as_slice(&self) -> &[u8] {
        match self {
            NalPayload::Borrowed(data) => data,
            NalPayload::Owned(vec) => vec,
            NalPayload::Buffer(buffer) => buffer.data(),
        }
    }
}

impl Clone for NalPayload<'_> {
    fn clone(&self) -> Self {
        match self {
            NalPayload::Borrowed(data) => NalPayload::Borrowed(data),
            NalPayload::Owned(vec) => NalPayload::Owned(vec.clone()),
            NalPayload::Buffer(buffer) => NalPayload::Buffer(Arc::clone(buffer)),
        }
    }
}

/// NAL unit with header and RBSP payload
#[derive(Debug)]
pub struct NalUnit<'a, T: NalHeader> {
    header: T,
    /// RBSP with EPB removed
    payload: NalPayload<'a>,
}

impl<'a, T: NalHeader> NalUnit<'a, T> {
    /// Create a new NAL unit with the given header and payload
    #[inline]
    fn new(header: T, payload: NalPayload<'a>) -> Self {
        Self {
            header,
            payload,
        }
    }

    /// Get the NAL header
    #[inline]
    pub fn header(&self) -> &T {
        &self.header
    }

    /// Get the complete NAL unit data (header + RBSP payload)
    #[inline]
    pub fn data(&self) -> &[u8] {
        self.payload.as_slice()
    }

    /// Get the RBSP payload (excluding header, with EPB removed)
    #[inline]
    pub fn rbsp(&self) -> &[u8] {
        &self.payload.as_slice()[T::HEADER_SIZE..]
    }

    /// Get the NAL unit type
    #[inline]
    pub fn nal_unit_type(&self) -> u8 {
        self.header.nal_unit_type()
    }

    /// Check if this is a VCL NAL unit
    #[inline]
    pub fn is_vcl(&self) -> bool {
        self.header.is_vcl()
    }

    /// Check if this is an IDR picture
    #[inline]
    pub fn is_idr(&self) -> bool {
        self.header.is_idr()
    }

    /// Check if this is a parameter set
    #[inline]
    pub fn is_parameter_set(&self) -> bool {
        self.header.is_parameter_set()
    }

    /// Check if the data is borrowed (zero-copy)
    #[inline]
    pub fn is_borrowed(&self) -> bool {
        matches!(self.payload, NalPayload::Borrowed(_))
    }

    /// Convert to owned data, making this NAL unit independent of the input
    /// lifetime
    pub fn into_owned(self) -> NalUnit<'static, T> {
        match self.payload {
            NalPayload::Owned(vec) => NalUnit {
                header: self.header,
                payload: NalPayload::Owned(vec),
            },
            NalPayload::Buffer(buffer) => NalUnit {
                header: self.header,
                payload: NalPayload::Buffer(buffer),
            },
            NalPayload::Borrowed(data) => NalUnit {
                header: self.header,
                payload: NalPayload::Owned(data.to_vec()),
            },
        }
    }
}

impl<T: NalHeader> Clone for NalUnit<'_, T> {
    fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            payload: self.payload.clone(),
        }
    }
}

/// NAL unit parser
pub struct NalParser<T: NalHeader> {
    /// Buffer pool for RBSP allocation (used only when EPB removal is needed)
    pool: Option<Arc<BufferPool>>,
    _marker: PhantomData<T>,
}

impl<T: NalHeader> NalParser<T> {
    /// Create a new NAL parser with the given buffer pool
    pub fn new(pool: Option<Arc<BufferPool>>) -> Self {
        Self {
            pool,
            _marker: PhantomData,
        }
    }

    /// Get a reference to the buffer pool
    pub fn pool(&self) -> Option<&Arc<BufferPool>> {
        self.pool.as_ref()
    }

    /// Parse a single NAL unit from raw NAL data (without start code)
    pub fn parse<'a>(&self, data: &'a [u8]) -> Result<NalUnit<'a, T>> {
        if data.len() < T::HEADER_SIZE {
            return Err(Error::InvalidData(format!("NAL data too short: expected at least {} bytes, got {}", T::HEADER_SIZE, data.len()).into()));
        }

        // Parse the header
        let header = T::parse(data)?;

        // Check if EPB removal is needed (only in payload, not header)
        let payload = &data[T::HEADER_SIZE..];
        let epb_count = Self::count_epb(payload);

        if epb_count == 0 {
            // Zero-copy: no EPB in payload, use borrowed reference
            Ok(NalUnit::new(header, NalPayload::Borrowed(data)))
        } else {
            // Need to remove EPB
            let new_len = data.len() - epb_count;

            if let Some(pool) = &self.pool {
                let mut buffer = pool.get_buffer_with_length(new_len);
                if let Some(buf) = Arc::get_mut(&mut buffer) {
                    let output = buf.data_mut();
                    output[..T::HEADER_SIZE].copy_from_slice(&data[..T::HEADER_SIZE]);
                    Self::remove_epb(payload, &mut output[T::HEADER_SIZE..]);
                    return Ok(NalUnit::new(header, NalPayload::Buffer(buffer)));
                }
            }

            let mut vec = vec![0u8; new_len];
            vec[..T::HEADER_SIZE].copy_from_slice(&data[..T::HEADER_SIZE]);
            Self::remove_epb(payload, &mut vec[T::HEADER_SIZE..]);
            Ok(NalUnit::new(header, NalPayload::Owned(vec)))
        }
    }

    /// Check if data contains any emulation prevention bytes
    #[inline]
    pub fn has_epb(data: &[u8]) -> bool {
        Self::count_epb(data) > 0
    }

    /// Count emulation prevention bytes in data
    fn count_epb(data: &[u8]) -> usize {
        let mut count = 0;
        let mut i = 0;

        while i + 2 < data.len() {
            if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x03 {
                let is_epb = i + 3 >= data.len() || data[i + 3] <= 0x03;
                if is_epb {
                    count += 1;
                    i += 3;
                    continue;
                }
            }
            i += 1;
        }

        count
    }

    /// Remove emulation prevention bytes from data
    fn remove_epb(input: &[u8], output: &mut [u8]) {
        let mut r = 0;
        let mut w = 0;

        while r < input.len() {
            if r + 2 < input.len() && input[r] == 0x00 && input[r + 1] == 0x00 && input[r + 2] == 0x03 {
                let is_epb = r + 3 >= input.len() || input[r + 3] <= 0x03;
                if is_epb {
                    output[w] = 0x00;
                    output[w + 1] = 0x00;
                    w += 2;
                    r += 3;
                    continue;
                }
            }
            output[w] = input[r];
            w += 1;
            r += 1;
        }
    }
}
