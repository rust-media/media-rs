//! H.264/AVC Sequence Parameter Set (SPS) parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, Result};
use smallvec::SmallVec;

use crate::{
    constants::{MAX_CPB_COUNT, MAX_SPS_COUNT},
    scaling_list::ScalingMatrix,
};

/// This struct stores them in the same order as the bitstream (MSB first)
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConstraintSetFlags(u8);

#[rustfmt::skip]
impl ConstraintSetFlags {
    /// Bit mask for constraint_set0_flag (bit 7)
    const FLAG0: u8 = 1 << 7;       // 0b1000_0000
    /// Bit mask for constraint_set1_flag (bit 6)
    const FLAG1: u8 = 1 << (7 - 1); // 0b0100_0000
    /// Bit mask for constraint_set2_flag (bit 5)
    const FLAG2: u8 = 1 << (7 - 2); // 0b0010_0000
    /// Bit mask for constraint_set3_flag (bit 4)
    const FLAG3: u8 = 1 << (7 - 3); // 0b0001_0000
    /// Bit mask for constraint_set4_flag (bit 3)
    const FLAG4: u8 = 1 << (7 - 4); // 0b0000_1000
    /// Bit mask for constraint_set5_flag (bit 2)
    const FLAG5: u8 = 1 << (7 - 5); // 0b0000_0100

    /// Create ConstraintSetFlags from raw byte value
    #[inline]
    pub const fn from_raw(value: u8) -> Self {
        Self(value)
    }

    /// Get the raw byte value
    #[inline]
    pub const fn raw(&self) -> u8 {
        self.0
    }

    /// Check if constraint_set0_flag is set
    #[inline]
    pub const fn has_flag0(&self) -> bool {
        self.0 & Self::FLAG0 != 0
    }

    /// Check if constraint_set1_flag is set
    #[inline]
    pub const fn has_flag1(&self) -> bool {
        self.0 & Self::FLAG1 != 0
    }

    /// Check if constraint_set2_flag is set
    #[inline]
    pub const fn has_flag2(&self) -> bool {
        self.0 & Self::FLAG2 != 0
    }

    /// Check if constraint_set3_flag is set
    #[inline]
    pub const fn has_flag3(&self) -> bool {
        self.0 & Self::FLAG3 != 0
    }

    /// Check if constraint_set4_flag is set
    #[inline]
    pub const fn has_flag4(&self) -> bool {
        self.0 & Self::FLAG4 != 0
    }

    /// Check if constraint_set5_flag is set
    #[inline]
    pub const fn has_flag5(&self) -> bool {
        self.0 & Self::FLAG5 != 0
    }
}

/// H.264/AVC Profile
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Profile {
    /// Baseline Profile (profile_idc = 66)
    Baseline                    = 66,
    /// Constrained Baseline Profile (profile_idc = 66, constraint_set1_flag =
    /// 1)
    ConstrainedBaseline         = ((ConstraintSetFlags::FLAG1 as u16) << 8) | 66,
    /// Main Profile (profile_idc = 77)
    Main                        = 77,
    /// Extended Profile (profile_idc = 88)
    Extended                    = 88,
    /// High Profile (profile_idc = 100)
    High                        = 100,
    /// Progressive High Profile (profile_idc = 100, constraint_set4_flag = 1)
    ProgressiveHigh             = ((ConstraintSetFlags::FLAG4 as u16) << 8) | 100,
    /// Constrained High Profile (profile_idc = 100, constraint_set4_flag = 1,
    /// constraint_set5_flag = 1)
    ConstrainedHigh             = (((ConstraintSetFlags::FLAG4 | ConstraintSetFlags::FLAG5) as u16) << 8) | 100,
    /// High 10 Profile (profile_idc = 110)
    High10                      = 110,
    /// High 10 Intra Profile (profile_idc = 110, constraint_set3_flag = 1)
    High10Intra                 = ((ConstraintSetFlags::FLAG3 as u16) << 8) | 110,
    /// High 4:2:2 Profile (profile_idc = 122)
    High422                     = 122,
    /// High 4:2:2 Intra Profile (profile_idc = 122, constraint_set3_flag = 1)
    High422Intra                = ((ConstraintSetFlags::FLAG3 as u16) << 8) | 122,
    /// High 4:4:4 Predictive Profile (profile_idc = 244)
    High444Predictive           = 244,
    /// High 4:4:4 Intra Profile (profile_idc = 244, constraint_set3_flag = 1)
    High444Intra                = ((ConstraintSetFlags::FLAG3 as u16) << 8) | 244,
    /// CAVLC 4:4:4 Intra Profile (profile_idc = 44)
    Cavlc444Intra               = 44,
    /// Scalable Baseline Profile (profile_idc = 83)
    ScalableBaseline            = 83,
    /// Scalable Constrained Baseline Profile (profile_idc = 83,
    /// constraint_set5_flag = 1)
    ScalableConstrainedBaseline = ((ConstraintSetFlags::FLAG5 as u16) << 8) | 83,
    /// Scalable High Profile (profile_idc = 86)
    ScalableHigh                = 86,
    /// Scalable High Intra Profile (profile_idc = 86, constraint_set3_flag = 1)
    ScalableHighIntra           = ((ConstraintSetFlags::FLAG3 as u16) << 8) | 86,
    /// Scalable Constrained High Profile (profile_idc = 86,
    /// constraint_set5_flag = 1)
    ScalableConstrainedHigh     = ((ConstraintSetFlags::FLAG5 as u16) << 8) | 86,
    /// Multiview High Profile (profile_idc = 118)
    MultiviewHigh               = 118,
    /// Stereo High Profile (profile_idc = 128)
    StereoHigh                  = 128,
    /// MFC High Profile (profile_idc = 134)
    MFCHigh                     = 134,
    /// MFC Depth High Profile (profile_idc = 135)
    MFCDepthHigh                = 135,
    /// Multiview Depth High Profile (profile_idc = 138)
    MultiviewDepthHigh          = 138,
    /// Enhanced Multiview Depth High Profile (profile_idc = 139)
    EnhancedMultiviewDepthHigh  = 139,
}

impl Profile {
    /// Create a Profile from profile_idc and constraint_set_flags
    pub fn from_raw(profile_idc: u8, constraint_set_flags: ConstraintSetFlags) -> Result<Self> {
        let profile = match profile_idc {
            66 => {
                if constraint_set_flags.has_flag1() {
                    Profile::ConstrainedBaseline
                } else {
                    Profile::Baseline
                }
            }
            77 => Profile::Main,
            88 => Profile::Extended,
            100 => {
                if constraint_set_flags.has_flag4() {
                    Profile::ProgressiveHigh
                } else if constraint_set_flags.has_flag4() | constraint_set_flags.has_flag5() {
                    Profile::ConstrainedHigh
                } else {
                    Profile::High
                }
            }
            110 => {
                if constraint_set_flags.has_flag3() {
                    Profile::High10Intra
                } else {
                    Profile::High10
                }
            }
            122 => {
                if constraint_set_flags.has_flag3() {
                    Profile::High422Intra
                } else {
                    Profile::High422
                }
            }
            244 => {
                if constraint_set_flags.has_flag3() {
                    Profile::High444Intra
                } else {
                    Profile::High444Predictive
                }
            }
            44 => Profile::Cavlc444Intra,
            83 => {
                if constraint_set_flags.has_flag5() {
                    Profile::ScalableConstrainedBaseline
                } else {
                    Profile::ScalableBaseline
                }
            }
            86 => {
                if constraint_set_flags.has_flag3() {
                    Profile::ScalableHighIntra
                } else if constraint_set_flags.has_flag5() {
                    Profile::ScalableConstrainedHigh
                } else {
                    Profile::ScalableHigh
                }
            }
            118 => Profile::MultiviewHigh,
            128 => Profile::StereoHigh,
            134 => Profile::MFCHigh,
            135 => Profile::MFCDepthHigh,
            138 => Profile::MultiviewDepthHigh,
            139 => Profile::EnhancedMultiviewDepthHigh,
            _ => return Err(invalid_data_error!("profile_idc", profile_idc)),
        };

        Ok(profile)
    }

    /// Get the profile_idc value
    pub fn idc(&self) -> u8 {
        (*self as u16 & 0xFF) as u8
    }

    /// Check if profile has extended chroma format support
    pub fn has_chroma_format_extension(&self) -> bool {
        let profile_idc = self.idc();
        matches!(profile_idc, 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135)
    }
}

/// Chroma format IDC values
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ChromaFormat {
    Monochrome = 0,
    #[default]
    YUV420     = 1,
    YUV422     = 2,
    YUV444     = 3,
}

impl From<u8> for ChromaFormat {
    fn from(value: u8) -> Self {
        match value {
            0 => ChromaFormat::Monochrome,
            1 => ChromaFormat::YUV420,
            2 => ChromaFormat::YUV422,
            3 => ChromaFormat::YUV444,
            _ => ChromaFormat::YUV420,
        }
    }
}

/// Extended SAR aspect_ratio_idc value (indicates sar_width/sar_height follow)
pub const EXTENDED_SAR: u8 = 255;

/// Aspect Ratio Information (SAR - Sample Aspect Ratio)
///
/// Each variant stores (aspect_ratio_idc, sar_width, sar_height).
/// Defined in ITU-T H.264 Table E-1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AspectRatioInfo {
    Unspecified,
    Ratio1x1,
    Ratio12x11,
    Ratio10x11,
    Ratio16x11,
    Ratio40x33,
    Ratio24x11,
    Ratio20x11,
    Ratio32x11,
    Ratio80x33,
    Ratio18x11,
    Ratio15x11,
    Ratio64x33,
    Ratio160x99,
    Ratio4x3,
    Ratio3x2,
    Ratio2x1,
    Reserved(u8),
    /// Extended SAR (aspect_ratio_idc = 255) with custom width and height
    Extended(u16, u16),
}

impl Default for AspectRatioInfo {
    fn default() -> Self {
        Self::Unspecified
    }
}

impl AspectRatioInfo {
    /// Parse AspectRatioInfo from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        let aspect_ratio_idc = reader.read::<8, u8>()?;

        if aspect_ratio_idc == EXTENDED_SAR {
            let sar_width = reader.read::<16, u16>()?;
            let sar_height = reader.read::<16, u16>()?;
            Ok(Self::Extended(sar_width, sar_height))
        } else {
            Ok(Self::from_idc(aspect_ratio_idc))
        }
    }

    /// Get the aspect_ratio_idc value
    #[inline]
    pub const fn idc(&self) -> u8 {
        match self {
            Self::Unspecified => 0,
            Self::Ratio1x1 => 1,
            Self::Ratio12x11 => 2,
            Self::Ratio10x11 => 3,
            Self::Ratio16x11 => 4,
            Self::Ratio40x33 => 5,
            Self::Ratio24x11 => 6,
            Self::Ratio20x11 => 7,
            Self::Ratio32x11 => 8,
            Self::Ratio80x33 => 9,
            Self::Ratio18x11 => 10,
            Self::Ratio15x11 => 11,
            Self::Ratio64x33 => 12,
            Self::Ratio160x99 => 13,
            Self::Ratio4x3 => 14,
            Self::Ratio3x2 => 15,
            Self::Ratio2x1 => 16,
            Self::Reserved(idc) => *idc,
            Self::Extended(_, _) => EXTENDED_SAR,
        }
    }

    /// Get the sample aspect ratio as (width, height)
    /// Returns None if unspecified
    pub const fn sample_aspect_ratio(&self) -> Option<(u16, u16)> {
        match self {
            Self::Unspecified => None,
            Self::Ratio1x1 => Some((1, 1)),
            Self::Ratio12x11 => Some((12, 11)),
            Self::Ratio10x11 => Some((10, 11)),
            Self::Ratio16x11 => Some((16, 11)),
            Self::Ratio40x33 => Some((40, 33)),
            Self::Ratio24x11 => Some((24, 11)),
            Self::Ratio20x11 => Some((20, 11)),
            Self::Ratio32x11 => Some((32, 11)),
            Self::Ratio80x33 => Some((80, 33)),
            Self::Ratio18x11 => Some((18, 11)),
            Self::Ratio15x11 => Some((15, 11)),
            Self::Ratio64x33 => Some((64, 33)),
            Self::Ratio160x99 => Some((160, 99)),
            Self::Ratio4x3 => Some((4, 3)),
            Self::Ratio3x2 => Some((3, 2)),
            Self::Ratio2x1 => Some((2, 1)),
            Self::Reserved(_) => None,
            Self::Extended(w, h) => Some((*w, *h)),
        }
    }

    /// Create from aspect_ratio_idc value
    pub const fn from_idc(idc: u8) -> Self {
        match idc {
            0 => Self::Unspecified,
            1 => Self::Ratio1x1,
            2 => Self::Ratio12x11,
            3 => Self::Ratio10x11,
            4 => Self::Ratio16x11,
            5 => Self::Ratio40x33,
            6 => Self::Ratio24x11,
            7 => Self::Ratio20x11,
            8 => Self::Ratio32x11,
            9 => Self::Ratio80x33,
            10 => Self::Ratio18x11,
            11 => Self::Ratio15x11,
            12 => Self::Ratio64x33,
            13 => Self::Ratio160x99,
            14 => Self::Ratio4x3,
            15 => Self::Ratio3x2,
            16 => Self::Ratio2x1,
            // Reserved values (17-254) treated as unspecified
            _ => Self::Unspecified,
        }
    }

    /// Check if the aspect ratio is extended
    #[inline]
    pub const fn is_extended(&self) -> bool {
        matches!(self, Self::Extended(_, _))
    }

    /// Check if the aspect ratio is specified
    #[inline]
    pub const fn is_specified(&self) -> bool {
        !matches!(self, Self::Unspecified | Self::Reserved(_))
    }
}

/// Video Format values
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum VideoFormat {
    Component   = 0,
    PAL         = 1,
    NTSC        = 2,
    SECAM       = 3,
    MAC         = 4,
    #[default]
    Unspecified = 5,
}

impl From<u8> for VideoFormat {
    fn from(value: u8) -> Self {
        match value {
            0 => VideoFormat::Component,
            1 => VideoFormat::PAL,
            2 => VideoFormat::NTSC,
            3 => VideoFormat::SECAM,
            4 => VideoFormat::MAC,
            _ => VideoFormat::Unspecified,
        }
    }
}

/// Colour Description (colour_primaries, transfer_characteristics,
/// matrix_coefficients)
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColourDescription {
    /// Colour primaries (Table E-3)
    pub colour_primaries: u8,
    /// Transfer characteristics (Table E-4)
    pub transfer_characteristics: u8,
    /// Matrix coefficients (Table E-5)
    pub matrix_coefficients: u8,
}

impl ColourDescription {
    /// Parse ColourDescription from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        Ok(Self {
            colour_primaries: reader.read::<8, u8>()?,
            transfer_characteristics: reader.read::<8, u8>()?,
            matrix_coefficients: reader.read::<8, u8>()?,
        })
    }
}

/// Video Signal Type
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VideoSignalType {
    /// Video format
    pub video_format: VideoFormat,
    /// Video full range flag (0=limited range, 1=full range)
    pub video_full_range_flag: bool,
    /// Colour description (if present)
    pub colour_description: Option<ColourDescription>,
}

impl VideoSignalType {
    /// Parse VideoSignalType from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        let video_format = VideoFormat::from(reader.read::<3, u8>()?);
        let video_full_range_flag = reader.read_bit()?;
        let colour_description_present_flag = reader.read_bit()?;
        let colour_description = if colour_description_present_flag {
            Some(ColourDescription::parse(reader)?)
        } else {
            None
        };

        Ok(Self {
            video_format,
            video_full_range_flag,
            colour_description,
        })
    }

    /// Check if video uses full range
    #[inline]
    pub fn is_full_range(&self) -> bool {
        self.video_full_range_flag
    }

    /// Get colour description if present
    #[inline]
    pub fn colour_description(&self) -> Option<&ColourDescription> {
        self.colour_description.as_ref()
    }
}

/// Chroma Sample Location Information
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChromaLocInfo {
    /// Chroma sample location type for top field
    pub chroma_sample_loc_type_top_field: u32,
    /// Chroma sample location type for bottom field
    pub chroma_sample_loc_type_bottom_field: u32,
}

impl ChromaLocInfo {
    /// Parse ChromaLocInfo from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        Ok(Self {
            chroma_sample_loc_type_top_field: reader.read_ue()?,
            chroma_sample_loc_type_bottom_field: reader.read_ue()?,
        })
    }
}

/// Timing Information
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimingInfo {
    /// Number of units in tick
    pub num_units_in_tick: u32,
    /// Time scale
    pub time_scale: u32,
    /// Fixed frame rate flag
    pub fixed_frame_rate_flag: bool,
}

impl TimingInfo {
    /// Parse TimingInfo from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        Ok(Self {
            num_units_in_tick: reader.read::<32, u32>()?,
            time_scale: reader.read::<32, u32>()?,
            fixed_frame_rate_flag: reader.read_bit()?,
        })
    }

    /// Get frame rate as (numerator, denominator)
    pub fn frame_rate(&self) -> Option<(u32, u32)> {
        if self.num_units_in_tick > 0 {
            // frame_rate = time_scale / (2 * num_units_in_tick)
            Some((self.time_scale, self.num_units_in_tick * 2))
        } else {
            None
        }
    }

    /// Get frame rate as floating point
    pub fn frame_rate_fps(&self) -> Option<f64> {
        self.frame_rate().map(|(num, den)| num as f64 / den as f64)
    }

    /// Check if the stream has a fixed frame rate
    #[inline]
    pub fn is_fixed_frame_rate(&self) -> bool {
        self.fixed_frame_rate_flag
    }
}

/// HRD (Hypothetical Reference Decoder) Parameters
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HrdParameters {
    /// CPB (Coded Picture Buffer) count
    pub cpb_cnt: u32,
    /// Bit rate scale
    pub bit_rate_scale: u8,
    /// CPB size scale
    pub cpb_size_scale: u8,
    /// Bit rate values for each CPB
    pub bit_rate_value: SmallVec<[u32; MAX_CPB_COUNT]>,
    /// CPB size values for each CPB
    pub cpb_size_value: SmallVec<[u32; MAX_CPB_COUNT]>,
    /// CBR (Constant Bit Rate) flags for each CPB
    pub cbr_flag: SmallVec<[bool; MAX_CPB_COUNT]>,
    /// Initial CPB removal delay length
    pub initial_cpb_removal_delay_length: u8,
    /// CPB removal delay length
    pub cpb_removal_delay_length: u8,
    /// DPB output delay length
    pub dpb_output_delay_length: u8,
    /// Time offset length
    pub time_offset_length: u8,
}

impl HrdParameters {
    /// Parse HrdParameters from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        let cpb_cnt = reader.read_ue()? + 1;
        let bit_rate_scale = reader.read::<4, u8>()?;
        let cpb_size_scale = reader.read::<4, u8>()?;

        let cpb_cnt_usize = cpb_cnt as usize;
        let mut bit_rate_value = SmallVec::with_capacity(cpb_cnt_usize);
        let mut cpb_size_value = SmallVec::with_capacity(cpb_cnt_usize);
        let mut cbr_flag = SmallVec::with_capacity(cpb_cnt_usize);

        for _ in 0..cpb_cnt_usize {
            bit_rate_value.push(reader.read_ue()? + 1);
            cpb_size_value.push(reader.read_ue()? + 1);
            cbr_flag.push(reader.read_bit()?);
        }

        let initial_cpb_removal_delay_length = reader.read::<5, u8>()? + 1;
        let cpb_removal_delay_length = reader.read::<5, u8>()? + 1;
        let dpb_output_delay_length = reader.read::<5, u8>()? + 1;
        let time_offset_length = reader.read::<5, u8>()?;

        Ok(Self {
            cpb_cnt,
            bit_rate_scale,
            cpb_size_scale,
            bit_rate_value,
            cpb_size_value,
            cbr_flag,
            initial_cpb_removal_delay_length,
            cpb_removal_delay_length,
            dpb_output_delay_length,
            time_offset_length,
        })
    }

    /// Get bit rate for a specific CPB index (in bits/second)
    pub fn bit_rate(&self, cpb_index: usize) -> Option<u64> {
        self.bit_rate_value.get(cpb_index).map(|&val| (val as u64) << (6 + self.bit_rate_scale))
    }

    /// Get CPB size for a specific CPB index (in bits)
    pub fn cpb_size(&self, cpb_index: usize) -> Option<u64> {
        self.cpb_size_value.get(cpb_index).map(|&val| (val as u64) << (4 + self.cpb_size_scale))
    }

    /// Check if a specific CPB operates in CBR mode
    pub fn is_cbr(&self, cpb_index: usize) -> Option<bool> {
        self.cbr_flag.get(cpb_index).copied()
    }
}

/// Bitstream Restriction Information
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BitstreamRestriction {
    /// Motion vectors over picture boundaries flag
    pub motion_vectors_over_pic_boundaries_flag: bool,
    /// Max bytes per pic denom
    pub max_bytes_per_pic_denom: u32,
    /// Max bits per mb denom
    pub max_bits_per_mb_denom: u32,
    /// Log2 max mv length horizontal
    pub log2_max_mv_length_horizontal: u32,
    /// Log2 max mv length vertical
    pub log2_max_mv_length_vertical: u32,
    /// Max num reorder frames
    pub max_num_reorder_frames: u32,
    /// Max dec frame buffering
    pub max_dec_frame_buffering: u32,
}

impl BitstreamRestriction {
    /// Parse BitstreamRestriction from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        Ok(Self {
            motion_vectors_over_pic_boundaries_flag: reader.read_bit()?,
            max_bytes_per_pic_denom: reader.read_ue()?,
            max_bits_per_mb_denom: reader.read_ue()?,
            log2_max_mv_length_horizontal: reader.read_ue()?,
            log2_max_mv_length_vertical: reader.read_ue()?,
            max_num_reorder_frames: reader.read_ue()?,
            max_dec_frame_buffering: reader.read_ue()?,
        })
    }

    /// Check if motion vectors can cross picture boundaries
    #[inline]
    pub fn allows_mv_over_pic_boundaries(&self) -> bool {
        self.motion_vectors_over_pic_boundaries_flag
    }

    /// Get the maximum number of frames that can be reordered
    #[inline]
    pub fn max_reorder_frames(&self) -> u32 {
        self.max_num_reorder_frames
    }

    /// Get the DPB (Decoded Picture Buffer) size
    #[inline]
    pub fn dpb_size(&self) -> u32 {
        self.max_dec_frame_buffering
    }
}

/// Video Usability Information (VUI) parameters
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VuiParameters {
    /// Aspect ratio information (if present)
    pub aspect_ratio_info: Option<AspectRatioInfo>,
    /// Overscan info present flag
    pub overscan_info_present_flag: bool,
    /// Overscan appropriate flag
    pub overscan_appropriate_flag: bool,
    /// Video signal type (if present)
    pub video_signal_type: Option<VideoSignalType>,
    /// Chroma location info (if present)
    pub chroma_loc_info: Option<ChromaLocInfo>,
    /// Timing info (if present)
    pub timing_info: Option<TimingInfo>,
    /// NAL HRD parameters (if present)
    pub nal_hrd_parameters: Option<HrdParameters>,
    /// VCL HRD parameters (if present)
    pub vcl_hrd_parameters: Option<HrdParameters>,
    /// Low delay HRD flag
    pub low_delay_hrd_flag: bool,
    /// Picture struct present flag
    pub pic_struct_present_flag: bool,
    /// Bitstream restriction (if present)
    pub bitstream_restriction: Option<BitstreamRestriction>,
}

impl VuiParameters {
    /// Parse VUI parameters from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        // Aspect ratio info
        let aspect_ratio_info_present_flag = reader.read_bit()?;
        let aspect_ratio_info = if aspect_ratio_info_present_flag {
            Some(AspectRatioInfo::parse(reader)?)
        } else {
            None
        };

        // Overscan info
        let overscan_info_present_flag = reader.read_bit()?;
        let overscan_appropriate_flag = if overscan_info_present_flag {
            reader.read_bit()?
        } else {
            false
        };

        // Video signal type
        let video_signal_type_present_flag = reader.read_bit()?;
        let video_signal_type = if video_signal_type_present_flag {
            Some(VideoSignalType::parse(reader)?)
        } else {
            None
        };

        // Chroma location info
        let chroma_loc_info_present_flag = reader.read_bit()?;
        let chroma_loc_info = if chroma_loc_info_present_flag {
            Some(ChromaLocInfo::parse(reader)?)
        } else {
            None
        };

        // Timing info
        let timing_info_present_flag = reader.read_bit()?;
        let timing_info = if timing_info_present_flag {
            Some(TimingInfo::parse(reader)?)
        } else {
            None
        };

        // NAL HRD parameters
        let nal_hrd_parameters_present_flag = reader.read_bit()?;
        let nal_hrd_parameters = if nal_hrd_parameters_present_flag {
            Some(HrdParameters::parse(reader)?)
        } else {
            None
        };

        // VCL HRD parameters
        let vcl_hrd_parameters_present_flag = reader.read_bit()?;
        let vcl_hrd_parameters = if vcl_hrd_parameters_present_flag {
            Some(HrdParameters::parse(reader)?)
        } else {
            None
        };

        let low_delay_hrd_flag = if nal_hrd_parameters.is_some() || vcl_hrd_parameters.is_some() {
            reader.read_bit()?
        } else {
            false
        };

        let pic_struct_present_flag = reader.read_bit()?;

        // Bitstream restriction
        let bitstream_restriction_flag = reader.read_bit()?;
        let bitstream_restriction = if bitstream_restriction_flag {
            Some(BitstreamRestriction::parse(reader)?)
        } else {
            None
        };

        Ok(Self {
            aspect_ratio_info,
            overscan_info_present_flag,
            overscan_appropriate_flag,
            video_signal_type,
            chroma_loc_info,
            timing_info,
            nal_hrd_parameters,
            vcl_hrd_parameters,
            low_delay_hrd_flag,
            pic_struct_present_flag,
            bitstream_restriction,
        })
    }

    /// Get sample aspect ratio if available
    pub fn sample_aspect_ratio(&self) -> Option<(u16, u16)> {
        self.aspect_ratio_info.as_ref().and_then(|ar| ar.sample_aspect_ratio())
    }

    /// Get frame rate if timing info is available
    pub fn frame_rate(&self) -> Option<(u32, u32)> {
        self.timing_info.as_ref().and_then(|ti| ti.frame_rate())
    }

    /// Get frame rate as floating point
    pub fn frame_rate_fps(&self) -> Option<f64> {
        self.timing_info.as_ref().and_then(|ti| ti.frame_rate_fps())
    }

    /// Check if video uses full range
    pub fn is_full_range(&self) -> bool {
        self.video_signal_type.as_ref().is_some_and(|vs| vs.is_full_range())
    }

    /// Get colour description if available
    pub fn colour_description(&self) -> Option<&ColourDescription> {
        self.video_signal_type.as_ref().and_then(|vs| vs.colour_description())
    }

    /// Check if NAL HRD parameters are present
    #[inline]
    pub fn has_nal_hrd(&self) -> bool {
        self.nal_hrd_parameters.is_some()
    }

    /// Check if VCL HRD parameters are present
    #[inline]
    pub fn has_vcl_hrd(&self) -> bool {
        self.vcl_hrd_parameters.is_some()
    }

    /// Get maximum number of reorder frames
    pub fn max_num_reorder_frames(&self) -> Option<u32> {
        self.bitstream_restriction.as_ref().map(|br| br.max_num_reorder_frames)
    }

    /// Get DPB size (max_dec_frame_buffering)
    pub fn dpb_size(&self) -> Option<u32> {
        self.bitstream_restriction.as_ref().map(|br| br.max_dec_frame_buffering)
    }
}

/// Sequence Parameter Set (SPS)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sps {
    /// Profile IDC
    pub profile_idc: u8,
    /// Constraint set flags (0-5)
    pub constraint_set_flags: ConstraintSetFlags,
    /// Level IDC
    pub level_idc: u8,
    /// Sequence parameter set ID (0-31)
    pub seq_parameter_set_id: u8,
    /// Chroma format
    pub chroma_format: ChromaFormat,
    /// Separate colour plane flag
    pub separate_colour_plane_flag: bool,
    /// Bit depth luma (8-14)
    pub bit_depth_luma: u8,
    /// Bit depth chroma (8-14)
    pub bit_depth_chroma: u8,
    /// QP prime Y zero transform bypass flag
    pub qpprime_y_zero_transform_bypass_flag: bool,
    /// Scaling matrix (if seq_scaling_matrix_present_flag is true)
    pub scaling_matrix: Option<ScalingMatrix>,
    /// Log2 of max frame number (4-16)
    pub log2_max_frame_num: u32,
    /// Picture order count type (0-2)
    pub pic_order_cnt_type: u32,
    /// Log2 of max picture order count LSB (4-16, valid when pic_order_cnt_type
    /// == 0)
    pub log2_max_pic_order_cnt_lsb: u32,
    /// Delta picture order always zero flag (for pic_order_cnt_type == 1)
    pub delta_pic_order_always_zero_flag: bool,
    /// Offset for non-reference picture (for pic_order_cnt_type == 1)
    pub offset_for_non_ref_pic: i32,
    /// Offset for top to bottom field (for pic_order_cnt_type == 1)
    pub offset_for_top_to_bottom_field: i32,
    /// Number of reference frames in pic order count cycle
    pub num_ref_frames_in_pic_order_cnt_cycle: u32,
    /// Offset for reference frames
    pub offset_for_ref_frame: SmallVec<[i32; 256]>,
    /// Maximum number of reference frames
    pub max_num_ref_frames: u32,
    /// Gaps in frame num value allowed flag
    pub gaps_in_frame_num_value_allowed_flag: bool,
    /// Picture width in macroblocks
    pub pic_width_in_mbs: u32,
    /// Picture height in map units
    pub pic_height_in_map_units: u32,
    /// Frame MBs only flag (1=frames only, 0=field/frame adaptive)
    pub frame_mbs_only_flag: bool,
    /// MB adaptive frame field flag
    pub mb_adaptive_frame_field_flag: bool,
    /// Direct 8x8 inference flag
    pub direct_8x8_inference_flag: bool,
    /// Frame cropping flag
    pub frame_cropping_flag: bool,
    /// Frame crop left offset
    pub frame_crop_left_offset: u32,
    /// Frame crop right offset
    pub frame_crop_right_offset: u32,
    /// Frame crop top offset
    pub frame_crop_top_offset: u32,
    /// Frame crop bottom offset
    pub frame_crop_bottom_offset: u32,
    /// VUI parameters
    pub vui_parameters: Option<VuiParameters>,
}

impl Sps {
    /// Parse SPS from raw NAL unit RBSP data (with EPB already removed)
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader)
    }

    /// Parse SPS from a BitReader
    pub fn parse_from_bit_reader<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        // Read profile_idc
        let profile_idc = reader.read::<8, u8>()?;

        // Read constraint set flags (6 flags + 2 reserved bits) as a single byte
        // The flags are stored in bitstream order: constraint_set0 at MSB (bit 7)
        let constraint_flags_byte = reader.read::<8, u8>()?;
        let constraint_set_flags = ConstraintSetFlags::from_raw(constraint_flags_byte);

        // Read level_idc
        let level_idc = reader.read::<8, u8>()?;

        // Read seq_parameter_set_id
        let seq_parameter_set_id = reader.read_ue()?;
        if seq_parameter_set_id as usize >= MAX_SPS_COUNT {
            return Err(invalid_data_error!("sps_id", seq_parameter_set_id));
        }
        let seq_parameter_set_id = seq_parameter_set_id as u8;

        // Initialize defaults
        let mut chroma_format = ChromaFormat::YUV420;
        let mut separate_colour_plane_flag = false;
        let mut bit_depth_luma = 8u8;
        let mut bit_depth_chroma = 8u8;
        let mut qpprime_y_zero_transform_bypass_flag = false;
        let mut scaling_matrix = None;

        let profile = Profile::from_raw(profile_idc, constraint_set_flags)?;

        // High profile extensions
        if profile.has_chroma_format_extension() {
            let chroma_format_idc = reader.read_ue()?;
            chroma_format = ChromaFormat::from(chroma_format_idc as u8);

            if chroma_format_idc == 3 {
                separate_colour_plane_flag = reader.read_bit()?;
            }

            bit_depth_luma = (reader.read_ue()? as u8) + 8;

            bit_depth_chroma = (reader.read_ue()? as u8) + 8;

            qpprime_y_zero_transform_bypass_flag = reader.read_bit()?;
            let seq_scaling_matrix_present_flag = reader.read_bit()?;

            if seq_scaling_matrix_present_flag {
                scaling_matrix = Some(ScalingMatrix::parse(reader, chroma_format_idc)?);
            }
        }

        // Read log2_max_frame_num and convert
        let log2_max_frame_num = reader.read_ue()? + 4;

        // Read pic_order_cnt_type
        let pic_order_cnt_type = reader.read_ue()?;

        let mut log2_max_pic_order_cnt_lsb = 4u32;
        let mut delta_pic_order_always_zero_flag = false;
        let mut offset_for_non_ref_pic = 0i32;
        let mut offset_for_top_to_bottom_field = 0i32;
        let mut num_ref_frames_in_pic_order_cnt_cycle = 0u32;
        let mut offset_for_ref_frame = SmallVec::new();

        if pic_order_cnt_type == 0 {
            log2_max_pic_order_cnt_lsb = reader.read_ue()? + 4;
        } else if pic_order_cnt_type == 1 {
            delta_pic_order_always_zero_flag = reader.read_bit()?;
            offset_for_non_ref_pic = reader.read_se()?;
            offset_for_top_to_bottom_field = reader.read_se()?;
            num_ref_frames_in_pic_order_cnt_cycle = reader.read_ue()?;

            offset_for_ref_frame.reserve(num_ref_frames_in_pic_order_cnt_cycle as usize);
            for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                offset_for_ref_frame.push(reader.read_se()?);
            }
        }

        // Read max_num_ref_frames
        let max_num_ref_frames = reader.read_ue()?;

        // Read gaps_in_frame_num_value_allowed_flag
        let gaps_in_frame_num_value_allowed_flag = reader.read_bit()?;

        // Read pic_width_in_mbs and convert
        let pic_width_in_mbs = reader.read_ue()? + 1;

        // Read pic_height_in_map_units and convert
        let pic_height_in_map_units = reader.read_ue()? + 1;

        // Read frame_mbs_only_flag
        let frame_mbs_only_flag = reader.read_bit()?;

        let mut mb_adaptive_frame_field_flag = false;
        if !frame_mbs_only_flag {
            mb_adaptive_frame_field_flag = reader.read_bit()?;
        }

        // Read direct_8x8_inference_flag
        let direct_8x8_inference_flag = reader.read_bit()?;

        // Read frame cropping
        let frame_cropping_flag = reader.read_bit()?;
        let mut frame_crop_left_offset = 0u32;
        let mut frame_crop_right_offset = 0u32;
        let mut frame_crop_top_offset = 0u32;
        let mut frame_crop_bottom_offset = 0u32;

        if frame_cropping_flag {
            frame_crop_left_offset = reader.read_ue()?;
            frame_crop_right_offset = reader.read_ue()?;
            frame_crop_top_offset = reader.read_ue()?;
            frame_crop_bottom_offset = reader.read_ue()?;
        }

        // Read VUI parameters
        let vui_parameters_present_flag = reader.read_bit()?;
        let vui_parameters = if vui_parameters_present_flag {
            Some(VuiParameters::parse(reader)?)
        } else {
            None
        };

        Ok(Self {
            profile_idc,
            constraint_set_flags,
            level_idc,
            seq_parameter_set_id,
            chroma_format,
            separate_colour_plane_flag,
            bit_depth_luma,
            bit_depth_chroma,
            qpprime_y_zero_transform_bypass_flag,
            scaling_matrix,
            log2_max_frame_num,
            pic_order_cnt_type,
            log2_max_pic_order_cnt_lsb,
            delta_pic_order_always_zero_flag,
            offset_for_non_ref_pic,
            offset_for_top_to_bottom_field,
            num_ref_frames_in_pic_order_cnt_cycle,
            offset_for_ref_frame,
            max_num_ref_frames,
            gaps_in_frame_num_value_allowed_flag,
            pic_width_in_mbs,
            pic_height_in_map_units,
            frame_mbs_only_flag,
            mb_adaptive_frame_field_flag,
            direct_8x8_inference_flag,
            frame_cropping_flag,
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
            vui_parameters,
        })
    }

    /// Get MaxFrameNum derived from log2_max_frame_num
    #[inline]
    pub fn max_frame_num(&self) -> u32 {
        1 << self.log2_max_frame_num
    }

    /// Get MaxPicOrderCntLsb derived from log2_max_pic_order_cnt_lsb
    #[inline]
    pub fn max_pic_order_cnt_lsb(&self) -> u32 {
        1 << self.log2_max_pic_order_cnt_lsb
    }

    /// Get the actual frame width in pixels
    pub fn width(&self) -> u32 {
        let width = self.pic_width_in_mbs * 16;
        let crop_x = self.crop_unit_x();
        width - (self.frame_crop_left_offset + self.frame_crop_right_offset) * crop_x
    }

    /// Get the actual frame height in pixels
    pub fn height(&self) -> u32 {
        let height = self.pic_height_in_map_units * 16 * (2 - self.frame_mbs_only_flag as u32);
        let crop_y = self.crop_unit_y();
        height - (self.frame_crop_top_offset + self.frame_crop_bottom_offset) * crop_y
    }

    /// Get the crop unit X based on chroma format
    fn crop_unit_x(&self) -> u32 {
        match self.chroma_format {
            ChromaFormat::Monochrome => 1,
            ChromaFormat::YUV420 => 2,
            ChromaFormat::YUV422 => 2,
            ChromaFormat::YUV444 => 1,
        }
    }

    /// Get the crop unit Y based on chroma format and frame_mbs_only_flag
    fn crop_unit_y(&self) -> u32 {
        let sub_height_c = match self.chroma_format {
            ChromaFormat::Monochrome => 1,
            ChromaFormat::YUV420 => 2,
            ChromaFormat::YUV422 => 1,
            ChromaFormat::YUV444 => 1,
        };
        sub_height_c * (2 - self.frame_mbs_only_flag as u32)
    }

    /// Get frame rate as (numerator, denominator) if available
    pub fn frame_rate(&self) -> Option<(u32, u32)> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.frame_rate())
    }

    /// Get frame rate as floating point
    pub fn frame_rate_fps(&self) -> Option<f64> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.frame_rate_fps())
    }

    /// Get sample aspect ratio (SAR) if available
    pub fn sample_aspect_ratio(&self) -> Option<(u16, u16)> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.sample_aspect_ratio())
    }

    /// Get colour description if available
    pub fn colour_description(&self) -> Option<&ColourDescription> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.colour_description())
    }

    /// Check if video uses full range
    pub fn is_full_range(&self) -> bool {
        self.vui_parameters.as_ref().is_some_and(|vui_params| vui_params.is_full_range())
    }

    /// Get timing info if available
    pub fn timing_info(&self) -> Option<&TimingInfo> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.timing_info.as_ref())
    }

    /// Get NAL HRD parameters if available
    pub fn nal_hrd_parameters(&self) -> Option<&HrdParameters> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.nal_hrd_parameters.as_ref())
    }

    /// Get VCL HRD parameters if available
    pub fn vcl_hrd_parameters(&self) -> Option<&HrdParameters> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.vcl_hrd_parameters.as_ref())
    }

    /// Get bitstream restriction if available
    pub fn bitstream_restriction(&self) -> Option<&BitstreamRestriction> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.bitstream_restriction.as_ref())
    }
}
