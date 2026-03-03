//! H.265/HEVC constants

/// Maximum number of VPS in the stream [0, 15]
pub const MAX_VPS_COUNT: usize = 16;

/// Maximum number of SPS in the stream [0, 15]
pub const MAX_SPS_COUNT: usize = 16;

/// Maximum number of PPS in the stream [0, 63]
pub const MAX_PPS_COUNT: usize = 64;

/// Maximum number of sub-layers in VPS/SPS [0, 6]
pub const MAX_SUB_LAYERS: usize = 7;

/// Maximum CPB (Coded Picture Buffer) count for HRD parameters
pub const MAX_CPB_COUNT: usize = 32;

/// Maximum number of reference pictures
pub const MAX_REFS: usize = 16;

/// Maximum number of long-term reference pictures in SPS
pub const MAX_LONG_TERM_REF_PICS: usize = 32;

/// Maximum number of tile rows
pub const MAX_TILE_ROWS: usize = 22;

/// Maximum number of tile columns
pub const MAX_TILE_COLUMNS: usize = 20;

/// Maximum number of entry point offsets (tile columns * max CTU rows)
pub const MAX_ENTRY_POINT_OFFSETS: usize = MAX_TILE_COLUMNS * 135;

/// Number of 4x4 scaling list matrices
pub const NUM_4X4_MATRICES: usize = 6;

/// Number of 8x8 scaling list matrices
pub const NUM_8X8_MATRICES: usize = 6;

/// Number of 16x16 scaling list matrices
pub const NUM_16X16_MATRICES: usize = 6;

/// Number of 32x32 scaling list matrices (full, for 4:4:4 chroma)
pub const NUM_32X32_MATRICES_FULL: usize = 6;

/// Number of 32x32 scaling list matrices (reduced, for non-4:4:4 chroma)
pub const NUM_32X32_MATRICES_REDUCED: usize = 2;

/// Default DC coefficient for 16x16 and 32x32 scaling lists
pub const DEFAULT_DC_COEFF: u8 = 16;

/// Extended SAR aspect_ratio_idc value (indicates sar_width/sar_height follow)
pub const EXTENDED_SAR: u8 = 255;

/// Maximum number of layers in VPS (1-63)
pub const MAX_VPS_LAYERS: usize = 63;

/// Minimum QP offset for chroma (-12)
pub const MIN_QP_OFFSET: i32 = -12;

/// Maximum QP offset for chroma (12)
pub const MAX_QP_OFFSET: i32 = 12;
