//! H.264/AVC Scaling List parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::Result;
use smallvec::SmallVec;

/// Zigzag scan order for 4x4 blocks
///
/// Maps linear index (0..16) to raster scan order position
#[rustfmt::skip]
pub const ZIGZAG_4X4: [u8; 16] = [
     0,  1,  4,  8,
     5,  2,  3,  6,
     9, 12, 13, 10,
     7, 11, 14, 15,
];

/// Zigzag scan order for 8x8 blocks
///
/// Maps linear index (0..64) to raster scan order position
#[rustfmt::skip]
pub const ZIGZAG_8X8: [u8; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63
];

/// Flat 4x4 scaling list (all values are 16)
///
/// Used as the default when no scaling list is specified
pub const FLAT_4X4: [u8; 16] = [16; 16];

/// Flat 8x8 scaling list (all values are 16)
///
/// Used as the default when no scaling list is specified
pub const FLAT_8X8: [u8; 64] = [16; 64];

/// Default 4x4 intra scaling list in raster scan order
///
/// From Rec. ITU-T H.264 Table 7-3
#[rustfmt::skip]
pub const DEFAULT_SCALING_4X4_INTRA: [u8; 16] = [
     6, 13, 20, 28,
    13, 20, 28, 32,
    20, 28, 32, 37,
    28, 32, 37, 42,
];

/// Default 4x4 inter scaling list in raster scan order
///
/// From Rec. ITU-T H.264 Table 7-3
#[rustfmt::skip]
pub const DEFAULT_SCALING_4X4_INTER: [u8; 16] = [
    10, 14, 20, 24,
    14, 20, 24, 27,
    20, 24, 27, 30,
    24, 27, 30, 34,
];

/// Default 8x8 intra scaling list in raster scan order
///
/// From Rec. ITU-T H.264 Table 7-4
#[rustfmt::skip]
pub const DEFAULT_SCALING_8X8_INTRA: [u8; 64] = [
     6, 10, 13, 16, 18, 23, 25, 27,
    10, 11, 16, 18, 23, 25, 27, 29,
    13, 16, 18, 23, 25, 27, 29, 31,
    16, 18, 23, 25, 27, 29, 31, 33,
    18, 23, 25, 27, 29, 31, 33, 36,
    23, 25, 27, 29, 31, 33, 36, 38,
    25, 27, 29, 31, 33, 36, 38, 40,
    27, 29, 31, 33, 36, 38, 40, 42,
];

/// Default 8x8 inter scaling list in raster scan order
///
/// From Rec. ITU-T H.264 Table 7-4
#[rustfmt::skip]
pub const DEFAULT_SCALING_8X8_INTER: [u8; 64] = [
     9, 13, 15, 17, 19, 21, 22, 24,
    13, 13, 17, 19, 21, 22, 24, 25,
    15, 17, 19, 21, 22, 24, 25, 27,
    17, 19, 21, 22, 24, 25, 27, 28,
    19, 21, 22, 24, 25, 27, 28, 30,
    21, 22, 24, 25, 27, 28, 30, 32,
    22, 24, 25, 27, 28, 30, 32, 33,
    24, 25, 27, 28, 30, 32, 33, 35,
];

/// Source of scaling list values
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScalingListSource {
    /// Use default scaling list (decoder should use built-in defaults)
    #[default]
    Default,
    /// Fallback to a previous scaling list
    Fallback,
    /// Explicitly specified in the bitstream
    Explicit,
}

/// A scaling list with N elements (typically 16 for 4x4 or 64 for 8x8)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalingList<const N: usize> {
    /// Scaling list values in raster scan order (only valid when source is
    /// Explicit)
    values: Option<[u8; N]>,
    /// Source of this scaling list
    pub source: ScalingListSource,
}

impl<const N: usize> Default for ScalingList<N> {
    fn default() -> Self {
        Self {
            values: None,
            source: ScalingListSource::Default,
        }
    }
}

impl<const N: usize> ScalingList<N> {
    /// Create a scaling list that uses the default values
    pub fn use_default() -> Self {
        Self {
            values: None,
            source: ScalingListSource::Default,
        }
    }

    /// Create a scaling list that falls back to a previous list
    pub fn fallback() -> Self {
        Self {
            values: None,
            source: ScalingListSource::Fallback,
        }
    }

    /// Create a scaling list with explicit values
    pub fn explicit(values: [u8; N]) -> Self {
        Self {
            values: Some(values),
            source: ScalingListSource::Explicit,
        }
    }

    /// Parse a scaling list from a bit reader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        // Get the appropriate zigzag scan table
        let zigzag: &[u8] = if N == 16 {
            &ZIGZAG_4X4
        } else {
            &ZIGZAG_8X8
        };

        let mut values = [0u8; N];
        let mut last_scale = 8i32;
        let mut next_scale = 8i32;

        for i in 0..N {
            if next_scale != 0 {
                let delta_scale = reader.read_se()?;
                next_scale = (last_scale + delta_scale + 256) % 256;
                if i == 0 && next_scale == 0 {
                    return Ok(Self::use_default());
                }
            }
            let scale = if next_scale == 0 {
                last_scale as u8
            } else {
                next_scale as u8
            };
            // Convert from zigzag order (bitstream) to raster order (storage)
            values[zigzag[i] as usize] = scale;
            last_scale = scale as i32;
        }

        Ok(Self::explicit(values))
    }

    /// Check if this scaling list uses default values
    #[inline]
    pub fn is_default(&self) -> bool {
        matches!(self.source, ScalingListSource::Default)
    }

    /// Check if this scaling list falls back to a previous list
    #[inline]
    pub fn is_fallback(&self) -> bool {
        matches!(self.source, ScalingListSource::Fallback)
    }

    /// Check if this scaling list has explicit values
    #[inline]
    pub fn is_explicit(&self) -> bool {
        matches!(self.source, ScalingListSource::Explicit)
    }

    /// Get the explicit values if available
    #[inline]
    pub fn values(&self) -> Option<&[u8; N]> {
        self.values.as_ref()
    }

    /// Get value at index (returns None if using default/fallback)
    #[inline]
    pub fn get(&self, index: usize) -> Option<u8> {
        self.values.as_ref().map(|v| v[index])
    }
}

/// Type alias for 4x4 scaling list (16 elements)
pub type ScalingList4x4 = ScalingList<16>;

/// Type alias for 8x8 scaling list (64 elements)
pub type ScalingList8x8 = ScalingList<64>;

/// Scaling matrices for H.264 transform coefficients
///
/// Contains up to 6 4x4 scaling lists and up to 6 8x8 scaling lists:
/// - Lists 0-2: 4x4 Intra Y, Cb, Cr
/// - Lists 3-5: 4x4 Inter Y, Cb, Cr
/// - Lists 6, 8, 10: 8x8 Intra Y (and Cb, Cr for chroma_format_idc == 3)
/// - Lists 7, 9, 11: 8x8 Inter Y (and Cb, Cr for chroma_format_idc == 3)
///
/// Note: This structure does NOT fill in default values. When `source` is
/// `Default` or `Fallback`, the decoder should use its built-in default tables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalingMatrix {
    /// 4x4 scaling lists (always 6)
    pub scaling_list_4x4: [ScalingList4x4; 6],
    /// 8x8 scaling lists (6 for chroma_format_idc == 3, otherwise 2)
    pub scaling_list_8x8: SmallVec<[ScalingList8x8; 6]>,
    /// Flags indicating which 4x4 lists are present in the bitstream
    pub scaling_list_4x4_present: [bool; 6],
    /// Flags indicating which 8x8 lists are present in the bitstream
    pub scaling_list_8x8_present: SmallVec<[bool; 6]>,
}

impl Default for ScalingMatrix {
    fn default() -> Self {
        Self {
            scaling_list_4x4: [ScalingList4x4::default(); 6],
            scaling_list_8x8: smallvec::smallvec![ScalingList8x8::default(); 2],
            scaling_list_4x4_present: [false; 6],
            scaling_list_8x8_present: smallvec::smallvec![false, false],
        }
    }
}

impl ScalingMatrix {
    /// Create a new scaling matrix with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create scaling matrix with the specified number of 8x8 lists
    pub fn with_8x8_count(count: usize) -> Self {
        Self {
            scaling_list_4x4: [ScalingList4x4::default(); 6],
            scaling_list_8x8: smallvec::smallvec![ScalingList8x8::default(); count],
            scaling_list_4x4_present: [false; 6],
            scaling_list_8x8_present: smallvec::smallvec![false; count],
        }
    }

    /// Parse scaling matrix from SPS
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, chroma_format_idc: u32) -> Result<Self> {
        let num_8x8_lists = if chroma_format_idc == 3 {
            6
        } else {
            2
        };
        let mut matrix = Self::with_8x8_count(num_8x8_lists);

        // Parse 4x4 scaling lists (always 6)
        for i in 0..6 {
            let present = reader.read_bit()?;
            matrix.scaling_list_4x4_present[i] = present;

            if present {
                matrix.scaling_list_4x4[i] = ScalingList4x4::parse(reader)?;
            } else {
                // Fallback rule: use previous list or default
                matrix.scaling_list_4x4[i] = match i {
                    0 | 3 => ScalingList4x4::use_default(),
                    _ => ScalingList4x4::fallback(),
                };
            }
        }

        // Parse 8x8 scaling lists
        for i in 0..num_8x8_lists {
            let present = reader.read_bit()?;
            matrix.scaling_list_8x8_present[i] = present;

            if present {
                matrix.scaling_list_8x8[i] = ScalingList8x8::parse(reader)?;
            } else {
                // Fallback rule: use previous list of same type or default
                matrix.scaling_list_8x8[i] = match i {
                    0 | 1 => ScalingList8x8::use_default(),
                    _ => ScalingList8x8::fallback(),
                };
            }
        }

        Ok(matrix)
    }

    /// Get 4x4 intra Y scaling list
    #[inline]
    pub fn intra_y_4x4(&self) -> &ScalingList4x4 {
        &self.scaling_list_4x4[0]
    }

    /// Get 4x4 intra Cb scaling list
    #[inline]
    pub fn intra_cb_4x4(&self) -> &ScalingList4x4 {
        &self.scaling_list_4x4[1]
    }

    /// Get 4x4 intra Cr scaling list
    #[inline]
    pub fn intra_cr_4x4(&self) -> &ScalingList4x4 {
        &self.scaling_list_4x4[2]
    }

    /// Get 4x4 inter Y scaling list
    #[inline]
    pub fn inter_y_4x4(&self) -> &ScalingList4x4 {
        &self.scaling_list_4x4[3]
    }

    /// Get 4x4 inter Cb scaling list
    #[inline]
    pub fn inter_cb_4x4(&self) -> &ScalingList4x4 {
        &self.scaling_list_4x4[4]
    }

    /// Get 4x4 inter Cr scaling list
    #[inline]
    pub fn inter_cr_4x4(&self) -> &ScalingList4x4 {
        &self.scaling_list_4x4[5]
    }

    /// Get 8x8 intra Y scaling list
    #[inline]
    pub fn intra_y_8x8(&self) -> Option<&ScalingList8x8> {
        self.scaling_list_8x8.get(0)
    }

    /// Get 8x8 inter Y scaling list
    #[inline]
    pub fn inter_y_8x8(&self) -> Option<&ScalingList8x8> {
        self.scaling_list_8x8.get(1)
    }

    /// Get the fallback list index for a 4x4 scaling list
    #[inline]
    pub fn fallback_index_4x4(i: usize) -> Option<usize> {
        match i {
            1 | 2 => Some(i - 1),
            4 | 5 => Some(i - 1),
            _ => None, // 0 and 3 use default, not fallback
        }
    }

    /// Get the fallback list index for an 8x8 scaling list
    #[inline]
    pub fn fallback_index_8x8(i: usize) -> Option<usize> {
        if i >= 2 {
            Some(i - 2)
        } else {
            None
        } // 0 and 1 use default, not fallback
    }
}
