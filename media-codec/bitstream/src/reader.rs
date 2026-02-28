use std::io::{Cursor, Error, ErrorKind, Read, Result, Seek, SeekFrom};

use bitstream_io::{BigEndian, BitRead, Endianness, Integer};

pub struct BitReader<R: Read, E: Endianness> {
    inner: bitstream_io::BitReader<R, E>,
    total_bits: usize,
}

impl<R: Read, E: Endianness> BitReader<R, E> {
    /// Create a new `BitReader` wrapping the given reader
    #[inline]
    pub fn new(reader: R) -> Self {
        Self {
            inner: bitstream_io::BitReader::new(reader),
            total_bits: 0,
        }
    }

    /// Reads a single bit from the stream
    #[inline]
    pub fn read_bit(&mut self) -> Result<bool> {
        self.inner.read_bit()
    }

    /// Reads up to `BITS` bits as a value of type `T`
    #[inline]
    pub fn read<const BITS: u32, T: Integer>(&mut self) -> Result<T> {
        self.inner.read::<BITS, T>()
    }

    /// Reads `bits` number of bits as a value of type `T`
    #[inline]
    pub fn read_var<T: Integer>(&mut self, bits: u32) -> Result<T> {
        self.inner.read_var(bits)
    }

    /// Skips the given number of bits
    #[inline]
    pub fn skip(&mut self, bits: u32) -> Result<()> {
        self.inner.skip(bits)
    }

    /// Returns true if the stream is byte-aligned
    #[inline]
    pub fn byte_aligned(&self) -> bool {
        self.inner.byte_aligned()
    }

    /// Discards any remaining bits until the stream is byte-aligned
    #[inline]
    pub fn byte_align(&mut self) {
        self.inner.byte_align()
    }

    /// Reads exactly `buf.len()` bytes into the buffer
    #[inline]
    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()> {
        self.inner.read_bytes(buf)
    }

    /// Reads `bytes` number of bytes into a vector
    #[inline]
    pub fn read_to_vec(&mut self, bytes: usize) -> Result<Vec<u8>> {
        self.inner.read_to_vec(bytes)
    }

    /// Reads bits until a stop bit is encountered
    #[inline]
    pub fn read_unary<const STOP_BIT: u8>(&mut self) -> Result<u32> {
        self.inner.read_unary::<STOP_BIT>()
    }
}

impl<'a, E: Endianness> BitReader<Cursor<&'a [u8]>, E> {
    /// Create a BitReader from a byte slice, automatically tracking length
    pub fn from_slice(data: &'a [u8]) -> Self {
        Self {
            inner: bitstream_io::BitReader::new(Cursor::new(data)),
            total_bits: data.len() * 8,
        }
    }

    pub fn bits_left(&mut self) -> Result<usize> {
        Ok(self.total_bits.saturating_sub(self.inner.position_in_bits()? as usize))
    }
}

// Exp-Golomb methods for BigEndian
impl<R: Read> BitReader<R, BigEndian> {
    /// Reads an unsigned Exp-Golomb coded value (ue(v))
    pub fn read_ue(&mut self) -> Result<u32> {
        // Count leading zeros until stop bit 1
        let leading_zeros = self.inner.read_unary::<1>()?;

        if leading_zeros == 0 {
            return Ok(0);
        } else if leading_zeros <= 31 {
            // Read the suffix bits
            let suffix: u32 = self.inner.read_var(leading_zeros)?;
            return Ok((1 << leading_zeros) - 1 + suffix);
        }

        Err(Error::new(ErrorKind::InvalidData, "Exp-Golomb code overflow"))
    }

    /// Reads a signed Exp-Golomb coded value (se(v))
    pub fn read_se(&mut self) -> Result<i32> {
        let unsigned = self.read_ue()?;
        Ok(((unsigned >> 1) as i32 + (unsigned & 1) as i32) * ((((unsigned & 1) as i32) << 1) - 1))
    }
}

// Peek functionality requires Seek support
impl<R: Read + Seek, E: Endianness> BitReader<R, E> {
    /// Gets the current position in bits from the start of the stream
    #[inline]
    pub fn position_in_bits(&mut self) -> Result<u64> {
        self.inner.position_in_bits()
    }

    /// Seeks to a position in the stream by bits
    #[inline]
    pub fn seek_bits(&mut self, from: SeekFrom) -> Result<u64> {
        self.inner.seek_bits(from)
    }

    /// Peeks the next `BITS` bits without consuming them
    pub fn peek<const BITS: u32, T: Integer>(&mut self) -> Result<T> {
        let pos = self.inner.position_in_bits()?;
        let value = self.inner.read::<BITS, T>()?;
        self.inner.seek_bits(SeekFrom::Start(pos))?;
        Ok(value)
    }

    /// Peeks `bits` number of bits without consuming them
    pub fn peek_var<T: Integer>(&mut self, bits: u32) -> Result<T> {
        let pos = self.inner.position_in_bits()?;
        let value = self.inner.read_var(bits)?;
        self.inner.seek_bits(SeekFrom::Start(pos))?;
        Ok(value)
    }

    /// Peeks a single bit without consuming it
    pub fn peek_bit(&mut self) -> Result<bool> {
        let pos = self.inner.position_in_bits()?;
        let value = self.inner.read_bit()?;
        self.inner.seek_bits(SeekFrom::Start(pos))?;
        Ok(value)
    }
}

// Peek with Exp-Golomb for BigEndian + Seek
impl<R: Read + Seek> BitReader<R, BigEndian> {
    /// Peeks an unsigned Exp-Golomb coded value without consuming it
    pub fn peek_ue(&mut self) -> Result<u32> {
        let pos = self.inner.position_in_bits()?;
        let value = self.read_ue()?;
        self.inner.seek_bits(SeekFrom::Start(pos))?;
        Ok(value)
    }

    /// Peeks a signed Exp-Golomb coded value without consuming it
    pub fn peek_se(&mut self) -> Result<i32> {
        let pos = self.inner.position_in_bits()?;
        let value = self.read_se()?;
        self.inner.seek_bits(SeekFrom::Start(pos))?;
        Ok(value)
    }
}
