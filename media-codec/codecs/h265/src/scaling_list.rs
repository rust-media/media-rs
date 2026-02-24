//! H.265/HEVC Scaling List Data parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::Result;
use smallvec::SmallVec;

/// Diagonal scan order for 4x4 blocks
///
/// Maps linear index (0..16) to (x, y) coordinates in scan order.
/// From Table 6-4 in H.265 specification.
#[rustfmt::skip]
pub const DIAGONAL_SCAN_4X4: [(u8, u8); 16] = [
    (0, 0), (1, 0), (0, 1), (2, 0),
    (1, 1), (0, 2), (3, 0), (2, 1),
    (1, 2), (0, 3), (3, 1), (2, 2),
    (1, 3), (3, 2), (2, 3), (3, 3),
];

/// Diagonal scan order for 8x8 blocks (used for 8x8, 16x16, 32x32 scaling lists)
///
/// The 16x16 and 32x32 lists use 8x8 DC coefficient scaling,
/// so this table is used for those as well.
/// From Table 6-4 in H.265 specification.
#[rustfmt::skip]
pub const DIAGONAL_SCAN_8X8: [(u8, u8); 64] = [
    (0, 0), (1, 0), (0, 1), (2, 0), (1, 1), (0, 2), (3, 0), (2, 1),
    (1, 2), (0, 3), (4, 0), (3, 1), (2, 2), (1, 3), (0, 4), (5, 0),
    (4, 1), (3, 2), (2, 3), (1, 4), (0, 5), (6, 0), (5, 1), (4, 2),
    (3, 3), (2, 4), (1, 5), (0, 6), (7, 0), (6, 1), (5, 2), (4, 3),
    (3, 4), (2, 5), (1, 6), (0, 7), (7, 1), (6, 2), (5, 3), (4, 4),
    (3, 5), (2, 6), (1, 7), (7, 2), (6, 3), (5, 4), (4, 5), (3, 6),
    (2, 7), (7, 3), (6, 4), (5, 5), (4, 6), (3, 7), (7, 4), (6, 5),
    (5, 6), (4, 7), (7, 5), (6, 6), (5, 7), (7, 6), (6, 7), (7, 7),
];

/// Flat 4x4 scaling list (all values are 16)
///
/// Used as the default when no scaling list is specified.
pub const FLAT_4X4: [u8; 16] = [16; 16];

/// Flat 8x8 scaling list (all values are 16)
///
/// Used as the default when no scaling list is specified.
pub const FLAT_8X8: [u8; 64] = [16; 64];

/// Default 4x4 intra scaling list (all values are 16)
///
/// From Table 7-5 in H.265 specification.
/// H.265 uses flat default for 4x4 lists.
pub const DEFAULT_SCALING_4X4_INTRA: [u8; 16] = [16; 16];

/// Default 4x4 inter scaling list (all values are 16)
///
/// From Table 7-5 in H.265 specification.
/// H.265 uses flat default for 4x4 lists.
pub const DEFAULT_SCALING_4X4_INTER: [u8; 16] = [16; 16];

/// Default 8x8 intra scaling list
///
/// From Table 7-6 in H.265 specification.
/// This is used for 8x8, 16x16, and 32x32 intra scaling lists.
#[rustfmt::skip]
pub const DEFAULT_SCALING_8X8_INTRA: [u8; 64] = [
    16, 16, 16, 16, 17, 18, 21, 24,
    16, 16, 16, 16, 17, 19, 22, 25,
    16, 16, 17, 18, 20, 22, 25, 29,
    16, 16, 18, 21, 24, 27, 31, 36,
    17, 17, 20, 24, 30, 35, 41, 47,
    18, 19, 22, 27, 35, 44, 54, 65,
    21, 22, 25, 31, 41, 54, 70, 88,
    24, 25, 29, 36, 47, 65, 88, 115,
];

/// Default 8x8 inter scaling list
///
/// From Table 7-6 in H.265 specification.
/// This is used for 8x8, 16x16, and 32x32 inter scaling lists.
#[rustfmt::skip]
pub const DEFAULT_SCALING_8X8_INTER: [u8; 64] = [
    16, 16, 16, 16, 17, 18, 20, 24,
    16, 16, 16, 17, 18, 20, 24, 25,
    16, 16, 17, 18, 20, 24, 25, 28,
    16, 17, 18, 20, 24, 25, 28, 33,
    17, 18, 20, 24, 25, 28, 33, 41,
    18, 20, 24, 25, 28, 33, 41, 54,
    20, 24, 25, 28, 33, 41, 54, 71,
    24, 25, 28, 33, 41, 54, 71, 91,
];

/// Size ID values for scaling list
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ScalingListSizeId {
    /// 4x4 block size
    Size4x4   = 0,
    /// 8x8 block size
    Size8x8   = 1,
    /// 16x16 block size
    Size16x16 = 2,
    /// 32x32 block size
    Size32x32 = 3,
}

impl ScalingListSizeId {
    /// Get the number of coefficients for this size
    #[inline]
    pub const fn number_of_coefficients(&self) -> usize {
        match self {
            Self::Size4x4 => 16,
            Self::Size8x8 | Self::Size16x16 | Self::Size32x32 => 64,
        }
    }

    /// Get the block size in pixels
    #[inline]
    pub const fn block_size(&self) -> usize {
        match self {
            Self::Size4x4 => 4,
            Self::Size8x8 => 8,
            Self::Size16x16 => 16,
            Self::Size32x32 => 32,
        }
    }

    /// Get the number of matrix IDs for this size
    ///
    /// - 4x4 and 8x8: 6 matrices (3 intra Y/Cb/Cr + 3 inter Y/Cb/Cr)
    /// - 16x16 and 32x32: 6 matrices (3 intra Y/Cb/Cr + 3 inter Y/Cb/Cr) but
    ///   32x32 only has 2 when chroma_format_idc != 3
    #[inline]
    pub const fn number_of_matrix_ids(&self) -> usize {
        match self {
            Self::Size4x4 | Self::Size8x8 | Self::Size16x16 => 6,
            Self::Size32x32 => 6, // Can be 2 when chroma_format_idc != 3
        }
    }
}

/// Matrix ID values for scaling list
///
/// For 4x4 and 8x8 blocks:
/// - 0, 1, 2: Intra Y, Cb, Cr
/// - 3, 4, 5: Inter Y, Cb, Cr
///
/// For 16x16 blocks:
/// - 0, 1, 2: Intra Y, Cb, Cr
/// - 3, 4, 5: Inter Y, Cb, Cr
///
/// For 32x32 blocks:
/// - 0: Intra Y
/// - 3: Inter Y
/// (Cb/Cr matrices exist only when chroma_format_idc == 3)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MatrixId {
    /// Intra Y component
    IntraY  = 0,
    /// Intra Cb component
    IntraCb = 1,
    /// Intra Cr component
    IntraCr = 2,
    /// Inter Y component
    InterY  = 3,
    /// Inter Cb component
    InterCb = 4,
    /// Inter Cr component
    InterCr = 5,
}

impl MatrixId {
    /// Check if this is an intra matrix
    #[inline]
    pub const fn is_intra(&self) -> bool {
        (*self as u8) < 3
    }

    /// Check if this is an inter matrix
    #[inline]
    pub const fn is_inter(&self) -> bool {
        (*self as u8) >= 3
    }

    /// Check if this is a luma (Y) matrix
    #[inline]
    pub const fn is_luma(&self) -> bool {
        matches!(self, Self::IntraY | Self::InterY)
    }

    /// Get the fallback matrix ID for prediction
    pub const fn fallback_4x4(&self) -> Option<Self> {
        match self {
            // First of each type uses default
            Self::IntraY | Self::InterY => None,
            // Cb uses Y
            Self::IntraCb => Some(Self::IntraY),
            Self::InterCb => Some(Self::InterY),
            // Cr uses Cb
            Self::IntraCr => Some(Self::IntraCb),
            Self::InterCr => Some(Self::InterCb),
        }
    }

    /// Get the reference matrix ID for prediction from size_id - 1
    pub const fn reference_matrix_id(&self) -> Self {
        match self {
            Self::IntraY | Self::IntraCb | Self::IntraCr => Self::IntraY,
            Self::InterY | Self::InterCb | Self::InterCr => Self::InterY,
        }
    }
}

/// Get the default scaling list for a given size and matrix ID
pub fn get_default_scaling_list(size_id: ScalingListSizeId, matrix_id: MatrixId) -> &'static [u8] {
    match size_id {
        ScalingListSizeId::Size4x4 => {
            if matrix_id.is_intra() {
                &DEFAULT_SCALING_4X4_INTRA
            } else {
                &DEFAULT_SCALING_4X4_INTER
            }
        }
        ScalingListSizeId::Size8x8 | ScalingListSizeId::Size16x16 | ScalingListSizeId::Size32x32 => {
            if matrix_id.is_intra() {
                &DEFAULT_SCALING_8X8_INTRA
            } else {
                &DEFAULT_SCALING_8X8_INTER
            }
        }
    }
}

/// Get the default DC coefficient for 16x16 and 32x32 scaling lists
pub const fn get_default_dc_coefficient(matrix_id: MatrixId) -> u8 {
    // Default DC coefficient is always 16 for H.265
    if matrix_id.is_intra() {
        16
    } else {
        16
    }
}

pub const NUM_4X4_MATRICES: usize = 6;
pub const NUM_8X8_MATRICES: usize = 6;
pub const NUM_16X16_MATRICES: usize = 6;
pub const NUM_32X32_MATRICES_FULL: usize = 6;
pub const NUM_32X32_MATRICES_REDUCED: usize = 2;

/// Source of scaling list values
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScalingListSource {
    /// Use default scaling list (decoder should use built-in defaults)
    #[default]
    Default,
    /// Copy from a reference scaling list
    Copy,
    /// Explicitly specified in the bitstream
    Explicit,
}

/// A single scaling list with its source and optional DC coefficient
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalingList {
    /// The scaling list coefficients in raster scan order
    pub coefficients: Vec<u8>,
    /// DC coefficient for 16x16 and 32x32 blocks (None for 4x4 and 8x8)
    pub dc_coeff: Option<u8>,
    /// Source of this scaling list
    pub source: ScalingListSource,
}

impl Default for ScalingList {
    fn default() -> Self {
        Self {
            coefficients: vec![16; 16], // Default 4x4 size
            dc_coeff: None,
            source: ScalingListSource::Default,
        }
    }
}

impl ScalingList {
    /// Create a new scaling list with the specified size
    pub fn new(size_id: ScalingListSizeId) -> Self {
        let num_coeffs = size_id.number_of_coefficients();
        Self {
            coefficients: vec![16; num_coeffs],
            dc_coeff: match size_id {
                ScalingListSizeId::Size16x16 | ScalingListSizeId::Size32x32 => Some(16),
                _ => None,
            },
            source: ScalingListSource::Default,
        }
    }

    /// Create a scaling list with default values for the given size and matrix
    pub fn with_defaults(size_id: ScalingListSizeId, matrix_id: MatrixId) -> Self {
        let defaults = get_default_scaling_list(size_id, matrix_id);
        let dc_coeff = match size_id {
            ScalingListSizeId::Size16x16 | ScalingListSizeId::Size32x32 => Some(get_default_dc_coefficient(matrix_id)),
            _ => None,
        };

        Self {
            coefficients: defaults.to_vec(),
            dc_coeff,
            source: ScalingListSource::Default,
        }
    }

    /// Create a scaling list by copying from another
    pub fn copy_from(other: &ScalingList) -> Self {
        Self {
            coefficients: other.coefficients.clone(),
            dc_coeff: other.dc_coeff,
            source: ScalingListSource::Copy,
        }
    }

    /// Get the DC coefficient value
    #[inline]
    pub fn dc(&self) -> u8 {
        self.dc_coeff.unwrap_or_else(|| self.coefficients.first().copied().unwrap_or(16))
    }

    /// Get coefficient at the specified index
    #[inline]
    pub fn get(&self, index: usize) -> Option<u8> {
        self.coefficients.get(index).copied()
    }

    /// Check if this scaling list uses default values
    #[inline]
    pub fn is_default(&self) -> bool {
        matches!(self.source, ScalingListSource::Default)
    }

    /// Check if this scaling list was copied from another
    #[inline]
    pub fn is_copy(&self) -> bool {
        matches!(self.source, ScalingListSource::Copy)
    }

    /// Check if this scaling list has explicit values
    #[inline]
    pub fn is_explicit(&self) -> bool {
        matches!(self.source, ScalingListSource::Explicit)
    }

    /// Get the coefficients as a slice
    #[inline]
    pub fn coefficients(&self) -> &[u8] {
        &self.coefficients
    }
}

/// Complete scaling list data structure
///
/// Contains all scaling list matrices for an SPS or PPS.
/// H.265 has the following scaling lists:
/// - 6 4x4 matrices (Intra Y/Cb/Cr, Inter Y/Cb/Cr)
/// - 6 8x8 matrices (Intra Y/Cb/Cr, Inter Y/Cb/Cr)
/// - 6 16x16 matrices (Intra Y/Cb/Cr, Inter Y/Cb/Cr)
/// - 2 or 6 32x32 matrices (Intra Y, Inter Y; or full set for 4:4:4)
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalingListData {
    /// 4x4 scaling lists (6 matrices)
    pub scaling_list_4x4: [ScalingList; NUM_4X4_MATRICES],
    /// 8x8 scaling lists (6 matrices)
    pub scaling_list_8x8: [ScalingList; NUM_8X8_MATRICES],
    /// 16x16 scaling lists (6 matrices)
    pub scaling_list_16x16: [ScalingList; NUM_16X16_MATRICES],
    /// 32x32 scaling lists (2 or 6 matrices)
    pub scaling_list_32x32: SmallVec<[ScalingList; NUM_32X32_MATRICES_FULL]>,
}

impl Default for ScalingListData {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ScalingListData {
    /// Create a new ScalingListData with default scaling lists
    pub fn new(is_444: bool) -> Self {
        let num_32x32 = if is_444 {
            NUM_32X32_MATRICES_FULL
        } else {
            NUM_32X32_MATRICES_REDUCED
        };

        Self {
            scaling_list_4x4: std::array::from_fn(|i| {
                let matrix_id = unsafe { std::mem::transmute::<u8, MatrixId>(i as u8) };
                ScalingList::with_defaults(ScalingListSizeId::Size4x4, matrix_id)
            }),
            scaling_list_8x8: std::array::from_fn(|i| {
                let matrix_id = unsafe { std::mem::transmute::<u8, MatrixId>(i as u8) };
                ScalingList::with_defaults(ScalingListSizeId::Size8x8, matrix_id)
            }),
            scaling_list_16x16: std::array::from_fn(|i| {
                let matrix_id = unsafe { std::mem::transmute::<u8, MatrixId>(i as u8) };
                ScalingList::with_defaults(ScalingListSizeId::Size16x16, matrix_id)
            }),
            scaling_list_32x32: (0..num_32x32)
                .map(|i| {
                    // For 32x32, matrix_id is 0 and 3 (Y intra/inter) in reduced mode
                    // or 0-5 in full mode
                    let matrix_id = if is_444 {
                        unsafe { std::mem::transmute::<u8, MatrixId>(i as u8) }
                    } else {
                        // Map 0 -> IntraY (0), 1 -> InterY (3)
                        unsafe { std::mem::transmute::<u8, MatrixId>((i * 3) as u8) }
                    };
                    ScalingList::with_defaults(ScalingListSizeId::Size32x32, matrix_id)
                })
                .collect(),
        }
    }

    /// Parse scaling_list_data() from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, is_444: bool) -> Result<Self> {
        let mut data = Self::new(is_444);

        // Parse 4x4 scaling lists (sizeId == 0)
        for matrix_id in 0..NUM_4X4_MATRICES {
            data.scaling_list_4x4[matrix_id] = Self::parse_scaling_list(reader, ScalingListSizeId::Size4x4, matrix_id, &data)?;
        }

        // Parse 8x8 scaling lists (sizeId == 1)
        for matrix_id in 0..NUM_8X8_MATRICES {
            data.scaling_list_8x8[matrix_id] = Self::parse_scaling_list(reader, ScalingListSizeId::Size8x8, matrix_id, &data)?;
        }

        // Parse 16x16 scaling lists (sizeId == 2)
        for matrix_id in 0..NUM_16X16_MATRICES {
            data.scaling_list_16x16[matrix_id] = Self::parse_scaling_list(reader, ScalingListSizeId::Size16x16, matrix_id, &data)?;
        }

        // Parse 32x32 scaling lists (sizeId == 3)
        let num_32x32 = if is_444 {
            NUM_32X32_MATRICES_FULL
        } else {
            NUM_32X32_MATRICES_REDUCED
        };
        for i in 0..num_32x32 {
            // For 32x32, matrix_id is 0 and 3 when not 4:4:4
            let matrix_id = if is_444 {
                i
            } else {
                i * 3
            };
            data.scaling_list_32x32[i] = Self::parse_scaling_list(reader, ScalingListSizeId::Size32x32, matrix_id, &data)?;
        }

        Ok(data)
    }

    /// Parse a single scaling list
    fn parse_scaling_list<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        size_id: ScalingListSizeId,
        matrix_id: usize,
        current_data: &ScalingListData,
    ) -> Result<ScalingList> {
        // scaling_list_pred_mode_flag
        let pred_mode_flag = reader.read_bit()?;

        if !pred_mode_flag {
            // Copy mode - copy from reference list
            let pred_matrix_id_delta = reader.read_ue()? as usize;

            if pred_matrix_id_delta == 0 {
                // Use default scaling list
                let mat_id = unsafe { std::mem::transmute::<u8, MatrixId>((matrix_id % 6) as u8) };
                return Ok(ScalingList::with_defaults(size_id, mat_id));
            }

            // Copy from reference matrix
            let ref_matrix_id = matrix_id - pred_matrix_id_delta;

            let ref_list = match size_id {
                ScalingListSizeId::Size4x4 => &current_data.scaling_list_4x4[ref_matrix_id],
                ScalingListSizeId::Size8x8 => &current_data.scaling_list_8x8[ref_matrix_id],
                ScalingListSizeId::Size16x16 => &current_data.scaling_list_16x16[ref_matrix_id],
                ScalingListSizeId::Size32x32 => {
                    // For 32x32, need to handle the mapping
                    let ref_idx = if current_data.scaling_list_32x32.len() == NUM_32X32_MATRICES_REDUCED {
                        ref_matrix_id / 3
                    } else {
                        ref_matrix_id
                    };
                    &current_data.scaling_list_32x32[ref_idx]
                }
            };

            return Ok(ScalingList::copy_from(ref_list));
        }

        // Explicit mode - parse delta values
        let num_coeffs = size_id.number_of_coefficients();
        let mut coefficients = vec![0u8; num_coeffs];
        let mut next_coef = 8i32;

        // For 16x16 and 32x32, parse DC coefficient separately
        let dc_coeff = match size_id {
            ScalingListSizeId::Size16x16 | ScalingListSizeId::Size32x32 => {
                let scaling_list_dc_coef_minus8 = reader.read_se()?;
                let dc = ((scaling_list_dc_coef_minus8 + 8) & 0xFF) as u8;
                next_coef = dc as i32;
                Some(dc)
            }
            _ => None,
        };

        // Get the appropriate scan order
        let scan: &[(u8, u8)] = match size_id {
            ScalingListSizeId::Size4x4 => &DIAGONAL_SCAN_4X4,
            _ => &DIAGONAL_SCAN_8X8,
        };

        // Parse coefficients in scan order
        for i in 0..num_coeffs {
            let scaling_list_delta_coef = reader.read_se()?;
            next_coef = (next_coef + scaling_list_delta_coef + 256) % 256;

            // Convert from scan order to raster order
            let (x, y) = scan[i];
            let block_size = match size_id {
                ScalingListSizeId::Size4x4 => 4,
                _ => 8,
            };
            let raster_idx = (y as usize) * block_size + (x as usize);
            coefficients[raster_idx] = next_coef as u8;
        }

        Ok(ScalingList {
            coefficients,
            dc_coeff,
            source: ScalingListSource::Explicit,
        })
    }

    /// Get 4x4 intra Y scaling list
    #[inline]
    pub fn intra_y_4x4(&self) -> &ScalingList {
        &self.scaling_list_4x4[MatrixId::IntraY as usize]
    }

    /// Get 4x4 intra Cb scaling list
    #[inline]
    pub fn intra_cb_4x4(&self) -> &ScalingList {
        &self.scaling_list_4x4[MatrixId::IntraCb as usize]
    }

    /// Get 4x4 intra Cr scaling list
    #[inline]
    pub fn intra_cr_4x4(&self) -> &ScalingList {
        &self.scaling_list_4x4[MatrixId::IntraCr as usize]
    }

    /// Get 4x4 inter Y scaling list
    #[inline]
    pub fn inter_y_4x4(&self) -> &ScalingList {
        &self.scaling_list_4x4[MatrixId::InterY as usize]
    }

    /// Get 4x4 inter Cb scaling list
    #[inline]
    pub fn inter_cb_4x4(&self) -> &ScalingList {
        &self.scaling_list_4x4[MatrixId::InterCb as usize]
    }

    /// Get 4x4 inter Cr scaling list
    #[inline]
    pub fn inter_cr_4x4(&self) -> &ScalingList {
        &self.scaling_list_4x4[MatrixId::InterCr as usize]
    }

    /// Get 8x8 intra Y scaling list
    #[inline]
    pub fn intra_y_8x8(&self) -> &ScalingList {
        &self.scaling_list_8x8[MatrixId::IntraY as usize]
    }

    /// Get 8x8 intra Cb scaling list
    #[inline]
    pub fn intra_cb_8x8(&self) -> &ScalingList {
        &self.scaling_list_8x8[MatrixId::IntraCb as usize]
    }

    /// Get 8x8 intra Cr scaling list
    #[inline]
    pub fn intra_cr_8x8(&self) -> &ScalingList {
        &self.scaling_list_8x8[MatrixId::IntraCr as usize]
    }

    /// Get 8x8 inter Y scaling list
    #[inline]
    pub fn inter_y_8x8(&self) -> &ScalingList {
        &self.scaling_list_8x8[MatrixId::InterY as usize]
    }

    /// Get 8x8 inter Cb scaling list
    #[inline]
    pub fn inter_cb_8x8(&self) -> &ScalingList {
        &self.scaling_list_8x8[MatrixId::InterCb as usize]
    }

    /// Get 8x8 inter Cr scaling list
    #[inline]
    pub fn inter_cr_8x8(&self) -> &ScalingList {
        &self.scaling_list_8x8[MatrixId::InterCr as usize]
    }

    /// Get 16x16 intra Y scaling list
    #[inline]
    pub fn intra_y_16x16(&self) -> &ScalingList {
        &self.scaling_list_16x16[MatrixId::IntraY as usize]
    }

    /// Get 16x16 intra Cb scaling list
    #[inline]
    pub fn intra_cb_16x16(&self) -> &ScalingList {
        &self.scaling_list_16x16[MatrixId::IntraCb as usize]
    }

    /// Get 16x16 intra Cr scaling list
    #[inline]
    pub fn intra_cr_16x16(&self) -> &ScalingList {
        &self.scaling_list_16x16[MatrixId::IntraCr as usize]
    }

    /// Get 16x16 inter Y scaling list
    #[inline]
    pub fn inter_y_16x16(&self) -> &ScalingList {
        &self.scaling_list_16x16[MatrixId::InterY as usize]
    }

    /// Get 16x16 inter Cb scaling list
    #[inline]
    pub fn inter_cb_16x16(&self) -> &ScalingList {
        &self.scaling_list_16x16[MatrixId::InterCb as usize]
    }

    /// Get 16x16 inter Cr scaling list
    #[inline]
    pub fn inter_cr_16x16(&self) -> &ScalingList {
        &self.scaling_list_16x16[MatrixId::InterCr as usize]
    }

    /// Get 32x32 intra Y scaling list
    #[inline]
    pub fn intra_y_32x32(&self) -> Option<&ScalingList> {
        self.scaling_list_32x32.first()
    }

    /// Get 32x32 inter Y scaling list
    #[inline]
    pub fn inter_y_32x32(&self) -> Option<&ScalingList> {
        if self.scaling_list_32x32.len() == NUM_32X32_MATRICES_REDUCED {
            self.scaling_list_32x32.get(1)
        } else {
            self.scaling_list_32x32.get(MatrixId::InterY as usize)
        }
    }

    /// Get a scaling list by size and matrix ID
    pub fn get(&self, size_id: ScalingListSizeId, matrix_id: usize) -> Option<&ScalingList> {
        match size_id {
            ScalingListSizeId::Size4x4 => self.scaling_list_4x4.get(matrix_id),
            ScalingListSizeId::Size8x8 => self.scaling_list_8x8.get(matrix_id),
            ScalingListSizeId::Size16x16 => self.scaling_list_16x16.get(matrix_id),
            ScalingListSizeId::Size32x32 => {
                if self.scaling_list_32x32.len() == NUM_32X32_MATRICES_REDUCED {
                    // Map matrix_id (0 or 3) to index (0 or 1)
                    let idx = matrix_id / 3;
                    self.scaling_list_32x32.get(idx)
                } else {
                    self.scaling_list_32x32.get(matrix_id)
                }
            }
        }
    }
}
