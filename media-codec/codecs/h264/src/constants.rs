//! H.264/AVC constants

/// Maximum number of SPS in the stream
pub const MAX_SPS_COUNT: usize = 32;

/// Maximum number of PPS in the stream
pub const MAX_PPS_COUNT: usize = 256;

/// Maximum number of DPB (Decoded Picture Buffer) frames
pub const MAX_DPB_FRAMES: usize = 16;

/// Maximum number of reference pictures (2 fields per frame in interlaced mode)
pub const MAX_REFS: usize = 2 * MAX_DPB_FRAMES;

/// Maximum number of reference picture list modification entries
pub const MAX_RPLM_COUNT: usize = MAX_REFS + 1;

/// Maximum number of MMCO (Memory Management Control Operation) commands
pub const MAX_MMCO_COUNT: usize = MAX_REFS * 2 + 3;

/// Maximum number of slice groups
pub const MAX_SLICE_GROUPS: usize = 8;

/// Maximum CPB (Coded Picture Buffer) count for HRD parameters
pub const MAX_CPB_COUNT: usize = 32;

/// Maximum QP count (base QP range + High profile chroma QP offset)
pub const MAX_QP_COUNT: usize = 52 + 6 * 6;

/// Maximum picture size in macroblocks
pub const MAX_MB_PIC_SIZE: usize = 139264;

/// Maximum picture width in macroblocks
pub const MAX_MB_WIDTH: usize = 1055;

/// Maximum picture height in macroblocks
pub const MAX_MB_HEIGHT: usize = 1055;

/// Maximum picture width in pixels
pub const MAX_WIDTH: usize = MAX_MB_WIDTH * 16;

/// Maximum picture height in pixels
pub const MAX_HEIGHT: usize = MAX_MB_HEIGHT * 16;

/// Extended SAR aspect_ratio_idc value (indicates sar_width/sar_height follow)
pub const EXTENDED_SAR: u8 = 255;
