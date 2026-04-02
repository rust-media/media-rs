//! H.265/HEVC Sequence Parameter Set (SPS) parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, not_found_error, Result};
use smallvec::{smallvec, SmallVec};

use crate::{
    constants::{MAX_LONG_TERM_REF_PICS, MAX_REFS, MAX_SPS_COUNT, MAX_SUB_LAYERS, MAX_VPS_COUNT},
    ps::ParameterSets,
    scaling_list::ScalingListData,
    vps::{HrdParameters, ProfileTierLevel, Vps},
};

/// H.265/HEVC Profile
///
/// Defined in ITU-T H.265 Annex A
///
/// Note: Profile IDC 10 has two definitions depending on compatibility flags:
/// - Multiview Format Range Extensions (when
///   general_profile_compatibility_flag[6] is set)
/// - Scalable Format Range Extensions (when
///   general_profile_compatibility_flag[7] is set)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Profile {
    /// Main Profile (general_profile_idc = 1)
    /// - 8-bit 4:2:0 video
    /// - Suitable for most consumer applications
    Main                     = 1,
    /// Main 10 Profile (general_profile_idc = 2)
    /// - 10-bit 4:2:0 video
    /// - Commonly used for HDR content
    Main10                   = 2,
    /// Main Still Picture Profile (general_profile_idc = 3)
    /// - For still images encoded as single-frame video
    MainStillPicture         = 3,
    /// Range Extensions Profile (general_profile_idc = 4)
    /// - Extended bit depths (up to 16-bit) and chroma formats (4:2:2, 4:4:4)
    RangeExtensions          = 4,
    /// High Throughput Profile (general_profile_idc = 5)
    /// - Optimized for high-throughput applications
    HighThroughput           = 5,
    /// Multiview Main Profile (general_profile_idc = 6)
    /// - For 3D/multiview video
    MultiviewMain            = 6,
    /// Scalable Main Profile (general_profile_idc = 7)
    /// - For scalable video coding (SHVC)
    ScalableMain             = 7,
    /// 3D Main Profile (general_profile_idc = 8)
    /// - For 3D video with depth
    ThreeDMain               = 8,
    /// Screen Content Coding Profile (general_profile_idc = 9)
    /// - Optimized for screen content (text, graphics)
    ScreenContentCoding      = 9,
    /// Multiview Format Range Extensions Profile (general_profile_idc = 10)
    /// - Multiview video with extended bit depths/chroma formats
    /// - Identified when general_profile_compatibility_flag[6] is set
    MultiviewRangeExtensions = 10,
    /// Scalable Format Range Extensions Profile (general_profile_idc = 10)
    /// - Scalable video with extended bit depths/chroma formats
    /// - Identified when general_profile_compatibility_flag[7] is set
    ScalableRangeExtensions  = 1 << 4 | 10,
    /// High Throughput Screen Content Coding Extensions Profile
    /// (general_profile_idc = 11) - High throughput for screen content
    HighThroughputScreenContentCodingExtensions = 11,
}

impl Profile {
    /// Create a Profile from general_profile_idc value
    pub fn from_profile_idc(profile_idc: u8) -> Option<Self> {
        match profile_idc {
            1 => Some(Profile::Main),
            2 => Some(Profile::Main10),
            3 => Some(Profile::MainStillPicture),
            4 => Some(Profile::RangeExtensions),
            5 => Some(Profile::HighThroughput),
            6 => Some(Profile::MultiviewMain),
            7 => Some(Profile::ScalableMain),
            8 => Some(Profile::ThreeDMain),
            9 => Some(Profile::ScreenContentCoding),
            // Profile IDC 10 defaults to MultiviewRangeExtensions
            // Use from_profile_tier_level for accurate detection
            10 => Some(Profile::MultiviewRangeExtensions),
            11 => Some(Profile::HighThroughputScreenContentCodingExtensions),
            _ => None,
        }
    }

    /// Create a Profile from ProfileTierLevel
    pub fn from_profile_tier_level(ptl: &ProfileTierLevel) -> Option<Self> {
        let profile_idc = ptl.general_profile_idc;

        // Handle profile_idc = 10 specially: check compatibility flags to distinguish
        // between Multiview Format Range Extensions and Scalable Format Range
        // Extensions
        if profile_idc == 10 {
            // Check general_profile_compatibility_flag[6] for Multiview Format Range
            // Extensions
            if ptl.is_profile_compatible(6) {
                return Some(Profile::MultiviewRangeExtensions);
            }
            // Check general_profile_compatibility_flag[7] for Scalable Format Range
            // Extensions
            if ptl.is_profile_compatible(7) {
                return Some(Profile::ScalableRangeExtensions);
            }
            // Default to MultiviewRangeExtensions if neither flag is set
            return Some(Profile::MultiviewRangeExtensions);
        }

        // First try direct profile_idc match for other profiles
        if let Some(profile) = Self::from_profile_idc(profile_idc) {
            return Some(profile);
        }

        None
    }

    /// Get the profile_idc value
    #[inline]
    pub fn idc(&self) -> u8 {
        *self as u8 | 0x0F
    }

    /// Check if this profile supports 10-bit or higher bit depth
    pub fn supports_high_bit_depth(&self) -> bool {
        matches!(
            self,
            Profile::Main10 |
                Profile::RangeExtensions |
                Profile::HighThroughput |
                Profile::MultiviewRangeExtensions |
                Profile::ScalableRangeExtensions |
                Profile::HighThroughputScreenContentCodingExtensions
        )
    }

    /// Check if this profile supports 4:2:2 or 4:4:4 chroma formats
    pub fn supports_extended_chroma(&self) -> bool {
        matches!(
            self,
            Profile::RangeExtensions |
                Profile::HighThroughput |
                Profile::MultiviewRangeExtensions |
                Profile::ScalableRangeExtensions |
                Profile::HighThroughputScreenContentCodingExtensions
        )
    }

    /// Check if this is a scalable profile (SHVC)
    pub fn is_scalable(&self) -> bool {
        matches!(self, Profile::ScalableMain | Profile::ScalableRangeExtensions)
    }

    /// Check if this is a multiview/3D profile
    pub fn is_multiview(&self) -> bool {
        matches!(self, Profile::MultiviewMain | Profile::MultiviewRangeExtensions | Profile::ThreeDMain)
    }

    /// Check if this profile is optimized for screen content
    pub fn is_screen_content(&self) -> bool {
        matches!(self, Profile::ScreenContentCoding | Profile::HighThroughputScreenContentCodingExtensions)
    }
}

/// H.265/HEVC Tier
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum Tier {
    /// Main Tier (general_tier_flag = 0)
    #[default]
    Main = 0,
    /// High Tier (general_tier_flag = 1)
    High = 1,
}

impl From<bool> for Tier {
    fn from(tier_flag: bool) -> Self {
        if tier_flag {
            Tier::High
        } else {
            Tier::Main
        }
    }
}

impl Tier {
    /// Get the tier name as a string
    pub fn name(&self) -> &'static str {
        match self {
            Tier::Main => "Main",
            Tier::High => "High",
        }
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
/// Defined in ITU-T H.265 Table E-1.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum AspectRatioInfo {
    #[default]
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

    /// Check if aspect ratio is extended
    #[inline]
    pub const fn is_extended(&self) -> bool {
        matches!(self, Self::Extended(_, _))
    }

    /// Check if aspect ratio is specified
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
    pub vui_num_units_in_tick: u32,
    /// Time scale
    pub vui_time_scale: u32,
    /// POC proportional to timing flag
    pub vui_poc_proportional_to_timing_flag: bool,
    /// Number of ticks POC diff one
    pub vui_num_ticks_poc_diff_one: u32,
}

impl TimingInfo {
    /// Parse TimingInfo from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        let vui_num_units_in_tick = reader.read::<32, u32>()?;
        let vui_time_scale = reader.read::<32, u32>()?;
        let vui_poc_proportional_to_timing_flag = reader.read_bit()?;
        let vui_num_ticks_poc_diff_one = if vui_poc_proportional_to_timing_flag {
            reader.read_ue()? + 1
        } else {
            0
        };

        Ok(Self {
            vui_num_units_in_tick,
            vui_time_scale,
            vui_poc_proportional_to_timing_flag,
            vui_num_ticks_poc_diff_one,
        })
    }

    /// Get frame rate as (numerator, denominator)
    /// Returns None if num_units_in_tick is 0
    pub fn frame_rate(&self) -> Option<(u32, u32)> {
        if self.vui_num_units_in_tick > 0 {
            Some((self.vui_time_scale, self.vui_num_units_in_tick))
        } else {
            None
        }
    }

    /// Get frame rate as floating point
    pub fn frame_rate_fps(&self) -> Option<f64> {
        self.frame_rate().map(|(num, den)| num as f64 / den as f64)
    }
}

/// Bitstream Restriction Information
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BitstreamRestriction {
    /// Tiles fixed structure flag
    pub tiles_fixed_structure_flag: bool,
    /// Motion vectors over picture boundaries flag
    pub motion_vectors_over_pic_boundaries_flag: bool,
    /// Restricted ref pic lists flag
    pub restricted_ref_pic_lists_flag: bool,
    /// Min spatial segmentation IDC
    pub min_spatial_segmentation_idc: u32,
    /// Max bytes per pic denom
    pub max_bytes_per_pic_denom: u32,
    /// Max bits per min CU denom
    pub max_bits_per_min_cu_denom: u32,
    /// Log2 max mv length horizontal
    pub log2_max_mv_length_horizontal: u32,
    /// Log2 max mv length vertical
    pub log2_max_mv_length_vertical: u32,
}

impl BitstreamRestriction {
    /// Parse BitstreamRestriction from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        Ok(Self {
            tiles_fixed_structure_flag: reader.read_bit()?,
            motion_vectors_over_pic_boundaries_flag: reader.read_bit()?,
            restricted_ref_pic_lists_flag: reader.read_bit()?,
            min_spatial_segmentation_idc: reader.read_ue()?,
            max_bytes_per_pic_denom: reader.read_ue()?,
            max_bits_per_min_cu_denom: reader.read_ue()?,
            log2_max_mv_length_horizontal: reader.read_ue()?,
            log2_max_mv_length_vertical: reader.read_ue()?,
        })
    }

    /// Check if motion vectors can cross picture boundaries
    #[inline]
    pub fn allows_mv_over_pic_boundaries(&self) -> bool {
        self.motion_vectors_over_pic_boundaries_flag
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
    /// Neutral chroma indication flag
    pub neutral_chroma_indication_flag: bool,
    /// Field sequence flag
    pub field_seq_flag: bool,
    /// Frame field info present flag
    pub frame_field_info_present_flag: bool,
    /// Default display window flag
    pub default_display_window_flag: bool,
    /// Default display window left offset
    pub def_disp_win_left_offset: u32,
    /// Default display window right offset
    pub def_disp_win_right_offset: u32,
    /// Default display window top offset
    pub def_disp_win_top_offset: u32,
    /// Default display window bottom offset
    pub def_disp_win_bottom_offset: u32,
    /// Timing info (if present)
    pub timing_info: Option<TimingInfo>,
    /// HRD parameters present flag
    pub vui_hrd_parameters_present_flag: bool,
    /// HRD parameters (if present)
    pub hrd_parameters: Option<HrdParameters>,
    /// Bitstream restriction (if present)
    pub bitstream_restriction: Option<BitstreamRestriction>,
}

impl VuiParameters {
    /// Parse VUI parameters from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, max_sub_layers: u8) -> Result<Self> {
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

        // Additional VUI flags
        let neutral_chroma_indication_flag = reader.read_bit()?;
        let field_seq_flag = reader.read_bit()?;
        let frame_field_info_present_flag = reader.read_bit()?;

        // Default display window
        let default_display_window_flag = reader.read_bit()?;
        let (def_disp_win_left_offset, def_disp_win_right_offset, def_disp_win_top_offset, def_disp_win_bottom_offset) =
            if default_display_window_flag {
                (reader.read_ue()?, reader.read_ue()?, reader.read_ue()?, reader.read_ue()?)
            } else {
                (0, 0, 0, 0)
            };

        // Timing info
        let vui_timing_info_present_flag = reader.read_bit()?;
        let (timing_info, vui_hrd_parameters_present_flag, hrd_parameters) = if vui_timing_info_present_flag {
            let timing = TimingInfo::parse(reader)?;
            let hrd_present = reader.read_bit()?;
            let hrd = if hrd_present {
                Some(HrdParameters::parse(reader, true, max_sub_layers)?)
            } else {
                None
            };
            (Some(timing), hrd_present, hrd)
        } else {
            (None, false, None)
        };

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
            neutral_chroma_indication_flag,
            field_seq_flag,
            frame_field_info_present_flag,
            default_display_window_flag,
            def_disp_win_left_offset,
            def_disp_win_right_offset,
            def_disp_win_top_offset,
            def_disp_win_bottom_offset,
            timing_info,
            vui_hrd_parameters_present_flag,
            hrd_parameters,
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
}

/// Short-term reference picture set
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShortTermRefPicSet {
    /// Inter ref pic set prediction flag
    pub inter_ref_pic_set_prediction_flag: bool,
    /// Number of negative pictures
    pub num_negative_pics: u32,
    /// Number of positive pictures
    pub num_positive_pics: u32,
    /// Delta POC for negative pictures
    pub delta_poc_s0: SmallVec<[u32; MAX_REFS]>,
    /// Used by curr pic for negative pictures
    pub used_by_curr_pic_s0_flag: SmallVec<[bool; MAX_REFS]>,
    /// Delta POC for positive pictures
    pub delta_poc_s1: SmallVec<[u32; MAX_REFS]>,
    /// Used by curr pic for positive pictures
    pub used_by_curr_pic_s1_flag: SmallVec<[bool; MAX_REFS]>,
}

impl ShortTermRefPicSet {
    /// Parse ShortTermRefPicSet from a BitReader
    pub fn parse<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        st_rps_idx: usize,
        num_short_term_ref_pic_sets: usize,
        previous_sets: &[ShortTermRefPicSet],
    ) -> Result<Self> {
        let mut rps = Self::default();

        // Read inter_ref_pic_set_prediction_flag
        if st_rps_idx != 0 {
            rps.inter_ref_pic_set_prediction_flag = reader.read_bit()?;
        }

        if rps.inter_ref_pic_set_prediction_flag {
            // Prediction from another set
            let delta_idx = if st_rps_idx == num_short_term_ref_pic_sets {
                reader.read_ue()? as usize + 1
            } else {
                1
            };

            let _delta_rps_sign = reader.read_bit()?;
            let _abs_delta_rps = reader.read_ue()? + 1;

            let ref_rps_idx = st_rps_idx - delta_idx;
            if ref_rps_idx >= previous_sets.len() {
                return Err(invalid_data_error!("ref_rps_idx", ref_rps_idx));
            }

            let ref_rps = &previous_sets[ref_rps_idx];
            let num_delta_pocs = ref_rps.num_negative_pics + ref_rps.num_positive_pics;

            for _ in 0..=num_delta_pocs {
                let used_by_curr_pic_flag = reader.read_bit()?;
                if !used_by_curr_pic_flag {
                    let _use_delta_flag = reader.read_bit()?;
                }
            }

            // For simplicity, copy from reference (actual impl would compute)
            rps.num_negative_pics = ref_rps.num_negative_pics;
            rps.num_positive_pics = ref_rps.num_positive_pics;
        } else {
            // Explicit specification
            rps.num_negative_pics = reader.read_ue()?;
            rps.num_positive_pics = reader.read_ue()?;

            rps.delta_poc_s0.reserve(rps.num_negative_pics as usize);
            rps.used_by_curr_pic_s0_flag.reserve(rps.num_negative_pics as usize);
            for _ in 0..rps.num_negative_pics {
                rps.delta_poc_s0.push(reader.read_ue()? + 1);
                rps.used_by_curr_pic_s0_flag.push(reader.read_bit()?);
            }

            rps.delta_poc_s1.reserve(rps.num_positive_pics as usize);
            rps.used_by_curr_pic_s1_flag.reserve(rps.num_positive_pics as usize);
            for _ in 0..rps.num_positive_pics {
                rps.delta_poc_s1.push(reader.read_ue()? + 1);
                rps.used_by_curr_pic_s1_flag.push(reader.read_bit()?);
            }
        }

        Ok(rps)
    }

    /// Get total number of pictures in this set
    #[inline]
    pub fn num_pics(&self) -> u32 {
        self.num_negative_pics + self.num_positive_pics
    }
}

/// Sequence Parameter Set (SPS)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sps {
    /// VPS ID that this SPS refers to (0-15)
    pub video_parameter_set_id: u8,
    /// Maximum number of sub-layers (0-6)
    pub sps_max_sub_layers: u8,
    /// Temporal ID nesting flag
    pub sps_temporal_id_nesting_flag: bool,
    /// Profile tier level
    pub profile_tier_level: ProfileTierLevel,
    /// SPS ID (0-15)
    pub seq_parameter_set_id: u8,
    /// Chroma format
    pub chroma_format: ChromaFormat,
    /// Separate colour plane flag (for chroma_format_idc == 3)
    pub separate_colour_plane_flag: bool,
    /// Picture width in luma samples
    pub pic_width_in_luma_samples: u32,
    /// Picture height in luma samples
    pub pic_height_in_luma_samples: u32,
    /// Conformance window flag
    pub conformance_window_flag: bool,
    /// Conformance window left offset
    pub conf_win_left_offset: u32,
    /// Conformance window right offset
    pub conf_win_right_offset: u32,
    /// Conformance window top offset
    pub conf_win_top_offset: u32,
    /// Conformance window bottom offset
    pub conf_win_bottom_offset: u32,
    /// Bit depth luma
    pub bit_depth_luma: u8,
    /// Bit depth chroma
    pub bit_depth_chroma: u8,
    /// Log2 max POC LSB
    pub log2_max_pic_order_cnt_lsb: u8,
    /// SPS sub-layer ordering info present flag
    pub sps_sub_layer_ordering_info_present_flag: bool,
    /// Maximum decoder buffer size per sub-layer
    pub sps_max_dec_pic_buffering: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Maximum number of reorder pictures per sub-layer
    pub sps_max_num_reorder_pics: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Maximum latency increase plus 1 per sub-layer
    pub sps_max_latency_increase_plus1: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Log2 minimum luma coding block size
    pub log2_min_luma_coding_block_size: u32,
    /// Log2 differential max min luma coding block size
    pub log2_diff_max_min_luma_coding_block_size: u32,
    /// Log2 minimum luma transform block size
    pub log2_min_luma_transform_block_size: u32,
    /// Log2 differential max min luma transform block size
    pub log2_diff_max_min_luma_transform_block_size: u32,
    /// Maximum transform hierarchy depth for inter slices
    pub max_transform_hierarchy_depth_inter: u32,
    /// Maximum transform hierarchy depth for intra slices
    pub max_transform_hierarchy_depth_intra: u32,
    /// Scaling list enabled flag
    pub scaling_list_enabled_flag: bool,
    /// SPS scaling list data present flag
    pub sps_scaling_list_data_present_flag: bool,
    /// Scaling list data (if sps_scaling_list_data_present_flag)
    pub scaling_list_data: Option<ScalingListData>,
    /// Asymmetric motion partitions enabled flag
    pub amp_enabled_flag: bool,
    /// Sample adaptive offset enabled flag
    pub sample_adaptive_offset_enabled_flag: bool,
    /// PCM enabled flag
    pub pcm_enabled_flag: bool,
    /// PCM sample bit depth luma
    pub pcm_sample_bit_depth_luma: u8,
    /// PCM sample bit depth chroma
    pub pcm_sample_bit_depth_chroma: u8,
    /// Log2 minimum PCM luma coding block size
    pub log2_min_pcm_luma_coding_block_size: u32,
    /// Log2 differential max min PCM luma coding block size
    pub log2_diff_max_min_pcm_luma_coding_block_size: u32,
    /// PCM loop filter disabled flag
    pub pcm_loop_filter_disabled_flag: bool,
    /// Number of short-term reference picture sets
    pub num_short_term_ref_pic_sets: u32,
    /// Short-term reference picture sets
    pub short_term_ref_pic_sets: Vec<ShortTermRefPicSet>,
    /// Long-term reference pictures present in SPS flag
    pub long_term_ref_pics_present_flag: bool,
    /// Number of long-term reference pictures in SPS
    pub num_long_term_ref_pics_sps: u32,
    /// LT ref pic POC LSB SPS
    pub lt_ref_pic_poc_lsb_sps: SmallVec<[u32; MAX_LONG_TERM_REF_PICS]>,
    /// Used by curr pic LT SPS flag
    pub used_by_curr_pic_lt_sps_flag: SmallVec<[bool; MAX_LONG_TERM_REF_PICS]>,
    /// SPS temporal MVP enabled flag
    pub sps_temporal_mvp_enabled_flag: bool,
    /// Strong intra smoothing enabled flag
    pub strong_intra_smoothing_enabled_flag: bool,
    /// VUI parameters
    pub vui_parameters: Option<VuiParameters>,
    /// SPS extension present flag
    pub sps_extension_present_flag: bool,
    /// SPS range extension flag
    pub sps_range_extension_flag: bool,
    /// SPS multilayer extension flag
    pub sps_multilayer_extension_flag: bool,
    /// SPS 3D extension flag
    pub sps_3d_extension_flag: bool,
    /// SPS SCC extension flag
    pub sps_scc_extension_flag: bool,
    /// SPS extension 4 bits
    pub sps_extension_4bits: u8,
}

impl Sps {
    pub fn parse_vps_id(data: &[u8]) -> Result<u8> {
        let mut reader = BitReader::new(data);
        Self::parse_vps_id_from_bit_reader(&mut reader)
    }

    pub fn parse_with_vps(data: &[u8], vps: &Vps) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, Some(vps), None)
    }

    pub fn parse_with_param_sets(data: &[u8], param_sets: &ParameterSets) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, None, Some(param_sets))
    }

    pub fn parse_vps_id_from_bit_reader<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<u8> {
        // Read video_parameter_set_id
        let video_parameter_set_id = reader.read::<4, u8>()?;
        if video_parameter_set_id as usize >= MAX_VPS_COUNT {
            return Err(invalid_data_error!("vps_id", video_parameter_set_id));
        }

        Ok(video_parameter_set_id)
    }

    /// Parse SPS from a BitReader
    pub fn parse_from_bit_reader<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        vps: Option<&Vps>,
        param_sets: Option<&ParameterSets>,
    ) -> Result<Self> {
        let video_parameter_set_id = Self::parse_vps_id_from_bit_reader(reader)?;

        let vps = if let Some(vps) = vps {
            if video_parameter_set_id != vps.video_parameter_set_id {
                return Err(invalid_data_error!("vps_id", video_parameter_set_id));
            }

            Some(vps)
        } else if let Some(param_sets) = param_sets {
            Some(param_sets.get_vps(video_parameter_set_id as u32).ok_or_else(|| not_found_error!("vps_id", video_parameter_set_id))?)
        } else {
            None
        };

        // Read sps_max_sub_layers
        let sps_max_sub_layers = reader.read::<3, u8>()? + 1;
        if sps_max_sub_layers as usize > MAX_SUB_LAYERS {
            return Err(invalid_data_error!("sps_max_sub_layers", sps_max_sub_layers));
        }

        if let Some(vps) = vps {
            if sps_max_sub_layers > vps.vps_max_sub_layers {
                return Err(invalid_data_error!("sps_max_sub_layers", sps_max_sub_layers));
            }
        }

        // Read sps_temporal_id_nesting_flag
        let sps_temporal_id_nesting_flag = reader.read_bit()?;

        // Parse profile_tier_level
        let profile_tier_level = ProfileTierLevel::parse(reader, true, sps_max_sub_layers)?;

        // Read seq_parameter_set_id
        let seq_parameter_set_id = reader.read_ue()?;
        if seq_parameter_set_id as usize >= MAX_SPS_COUNT {
            return Err(invalid_data_error!("sps_id", seq_parameter_set_id));
        }

        // Read chroma_format_idc
        let chroma_format_idc = reader.read_ue()?;
        let chroma_format = ChromaFormat::from(chroma_format_idc as u8);

        // Read separate_colour_plane_flag (only if chroma_format_idc == 3)
        let separate_colour_plane_flag = if chroma_format_idc == 3 {
            reader.read_bit()?
        } else {
            false
        };

        // Read pic_width_in_luma_samples
        let pic_width_in_luma_samples = reader.read_ue()?;

        // Read pic_height_in_luma_samples
        let pic_height_in_luma_samples = reader.read_ue()?;

        // Read conformance_window_flag and conformance window offsets
        let conformance_window_flag = reader.read_bit()?;
        let (conf_win_left_offset, conf_win_right_offset, conf_win_top_offset, conf_win_bottom_offset) = if conformance_window_flag {
            (reader.read_ue()?, reader.read_ue()?, reader.read_ue()?, reader.read_ue()?)
        } else {
            (0, 0, 0, 0)
        };

        // Read bit_depth_luma
        let bit_depth_luma = reader.read_ue()? as u8 + 8;

        // Read bit_depth_chroma
        let bit_depth_chroma = reader.read_ue()? as u8 + 8;

        // Read log2_max_pic_order_cnt_lsb
        let log2_max_pic_order_cnt_lsb = reader.read_ue()? as u8 + 4;

        // Read sps_sub_layer_ordering_info_present_flag
        let sps_sub_layer_ordering_info_present_flag = reader.read_bit()?;

        // Parse sub-layer ordering info
        let num_sub_layers = sps_max_sub_layers as usize;
        let start_idx = if sps_sub_layer_ordering_info_present_flag {
            0
        } else {
            num_sub_layers - 1
        };

        let mut sps_max_dec_pic_buffering = smallvec![0u32; num_sub_layers];
        let mut sps_max_num_reorder_pics = smallvec![0u32; num_sub_layers];
        let mut sps_max_latency_increase_plus1 = smallvec![0u32; num_sub_layers];

        for i in start_idx..num_sub_layers {
            sps_max_dec_pic_buffering[i] = reader.read_ue()? + 1;
            sps_max_num_reorder_pics[i] = reader.read_ue()?;
            sps_max_latency_increase_plus1[i] = reader.read_ue()?;
        }

        // Fill in lower sub-layers if not present
        if !sps_sub_layer_ordering_info_present_flag {
            for i in 0..start_idx {
                sps_max_dec_pic_buffering[i] = sps_max_dec_pic_buffering[start_idx];
                sps_max_num_reorder_pics[i] = sps_max_num_reorder_pics[start_idx];
                sps_max_latency_increase_plus1[i] = sps_max_latency_increase_plus1[start_idx];
            }
        }

        // Read log2_min_luma_coding_block_size
        let log2_min_luma_coding_block_size = reader.read_ue()? + 3;

        // Read log2_diff_max_min_luma_coding_block_size
        let log2_diff_max_min_luma_coding_block_size = reader.read_ue()?;

        // Read log2_min_luma_transform_block_size
        let log2_min_luma_transform_block_size = reader.read_ue()? + 2;

        // Read log2_diff_max_min_luma_transform_block_size
        let log2_diff_max_min_luma_transform_block_size = reader.read_ue()?;

        // Read max_transform_hierarchy_depth_inter
        let max_transform_hierarchy_depth_inter = reader.read_ue()?;

        // Read max_transform_hierarchy_depth_intra
        let max_transform_hierarchy_depth_intra = reader.read_ue()?;

        // Read scaling_list_enabled_flag and parse scaling list data
        let scaling_list_enabled_flag = reader.read_bit()?;
        let (sps_scaling_list_data_present_flag, scaling_list_data) = if scaling_list_enabled_flag {
            let present = reader.read_bit()?;
            let data = if present {
                let is_444 = chroma_format_idc == 3;
                Some(ScalingListData::parse(reader, is_444)?)
            } else {
                None
            };
            (present, data)
        } else {
            (false, None)
        };

        // Read amp_enabled_flag
        let amp_enabled_flag = reader.read_bit()?;

        // Read sample_adaptive_offset_enabled_flag
        let sample_adaptive_offset_enabled_flag = reader.read_bit()?;

        // Read pcm_enabled_flag and PCM parameters
        let pcm_enabled_flag = reader.read_bit()?;
        let (
            pcm_sample_bit_depth_luma,
            pcm_sample_bit_depth_chroma,
            log2_min_pcm_luma_coding_block_size,
            log2_diff_max_min_pcm_luma_coding_block_size,
            pcm_loop_filter_disabled_flag,
        ) = if pcm_enabled_flag {
            // Read pcm_sample_bit_depth_luma
            let luma_bits = reader.read::<4, u8>()? + 1;
            // Read pcm_sample_bit_depth_chroma
            let chroma_bits = reader.read::<4, u8>()? + 1;
            // Read log2_min_pcm_luma_coding_block_size
            let min_size = reader.read_ue()? + 3;
            let diff_size = reader.read_ue()?;
            let loop_filter = reader.read_bit()?;
            (luma_bits, chroma_bits, min_size, diff_size, loop_filter)
        } else {
            (0, 0, 0, 0, false)
        };

        // Read num_short_term_ref_pic_sets
        let num_short_term_ref_pic_sets = reader.read_ue()?;

        // Parse short_term_ref_pic_set for each set
        let mut short_term_ref_pic_sets = Vec::with_capacity(num_short_term_ref_pic_sets as usize);
        for i in 0..num_short_term_ref_pic_sets as usize {
            let rps = ShortTermRefPicSet::parse(reader, i, num_short_term_ref_pic_sets as usize, &short_term_ref_pic_sets)?;
            short_term_ref_pic_sets.push(rps);
        }

        // Read long_term_ref_pics_present_flag and parse long-term reference pictures
        let long_term_ref_pics_present_flag = reader.read_bit()?;
        let (num_long_term_ref_pics_sps, lt_ref_pic_poc_lsb_sps, used_by_curr_pic_lt_sps_flag) = if long_term_ref_pics_present_flag {
            let num_lt = reader.read_ue()?;
            if num_lt as usize > MAX_LONG_TERM_REF_PICS {
                return Err(invalid_data_error!("num_long_term_ref_pics_sps", num_lt));
            }
            let mut poc_lsb = SmallVec::with_capacity(num_lt as usize);
            let mut used_flags = SmallVec::with_capacity(num_lt as usize);
            let log2_max_poc_lsb = log2_max_pic_order_cnt_lsb as u32;
            for _ in 0..num_lt {
                poc_lsb.push(reader.read_var(log2_max_poc_lsb)?);
                used_flags.push(reader.read_bit()?);
            }
            (num_lt, poc_lsb, used_flags)
        } else {
            (0, SmallVec::new(), SmallVec::new())
        };

        // Read sps_temporal_mvp_enabled_flag
        let sps_temporal_mvp_enabled_flag = reader.read_bit()?;

        // Read strong_intra_smoothing_enabled_flag
        let strong_intra_smoothing_enabled_flag = reader.read_bit()?;

        // Read vui_parameters_present_flag and parse VUI parameters
        let vui_parameters_present_flag = reader.read_bit()?;
        let vui_parameters = if vui_parameters_present_flag {
            Some(VuiParameters::parse(reader, sps_max_sub_layers)?)
        } else {
            None
        };

        // Read sps_extension_present_flag and extension flags
        let sps_extension_present_flag = reader.read_bit()?;
        let (sps_range_extension_flag, sps_multilayer_extension_flag, sps_3d_extension_flag, sps_scc_extension_flag, sps_extension_4bits) =
            if sps_extension_present_flag {
                (reader.read_bit()?, reader.read_bit()?, reader.read_bit()?, reader.read_bit()?, reader.read::<4, u8>()?)
            } else {
                (false, false, false, false, 0)
            };

        Ok(Self {
            video_parameter_set_id,
            sps_max_sub_layers,
            sps_temporal_id_nesting_flag,
            profile_tier_level,
            seq_parameter_set_id: seq_parameter_set_id as u8,
            chroma_format,
            separate_colour_plane_flag,
            pic_width_in_luma_samples,
            pic_height_in_luma_samples,
            conformance_window_flag,
            conf_win_left_offset,
            conf_win_right_offset,
            conf_win_top_offset,
            conf_win_bottom_offset,
            bit_depth_luma,
            bit_depth_chroma,
            log2_max_pic_order_cnt_lsb,
            sps_sub_layer_ordering_info_present_flag,
            sps_max_dec_pic_buffering,
            sps_max_num_reorder_pics,
            sps_max_latency_increase_plus1,
            log2_min_luma_coding_block_size,
            log2_diff_max_min_luma_coding_block_size,
            log2_min_luma_transform_block_size,
            log2_diff_max_min_luma_transform_block_size,
            max_transform_hierarchy_depth_inter,
            max_transform_hierarchy_depth_intra,
            scaling_list_enabled_flag,
            sps_scaling_list_data_present_flag,
            scaling_list_data,
            amp_enabled_flag,
            sample_adaptive_offset_enabled_flag,
            pcm_enabled_flag,
            pcm_sample_bit_depth_luma,
            pcm_sample_bit_depth_chroma,
            log2_min_pcm_luma_coding_block_size,
            log2_diff_max_min_pcm_luma_coding_block_size,
            pcm_loop_filter_disabled_flag,
            num_short_term_ref_pic_sets,
            short_term_ref_pic_sets,
            long_term_ref_pics_present_flag,
            num_long_term_ref_pics_sps,
            lt_ref_pic_poc_lsb_sps,
            used_by_curr_pic_lt_sps_flag,
            sps_temporal_mvp_enabled_flag,
            strong_intra_smoothing_enabled_flag,
            vui_parameters,
            sps_extension_present_flag,
            sps_range_extension_flag,
            sps_multilayer_extension_flag,
            sps_3d_extension_flag,
            sps_scc_extension_flag,
            sps_extension_4bits,
        })
    }

    /// Get actual bit depth for luma samples (8-16)
    #[inline]
    pub fn bit_depth_luma(&self) -> u8 {
        self.bit_depth_luma
    }

    /// Get actual bit depth for chroma samples (8-16)
    #[inline]
    pub fn bit_depth_chroma(&self) -> u8 {
        self.bit_depth_chroma
    }

    /// Get actual log2_max_pic_order_cnt_lsb (4-16)
    #[inline]
    pub fn log2_max_pic_order_cnt_lsb(&self) -> u8 {
        self.log2_max_pic_order_cnt_lsb
    }

    /// Get MaxPicOrderCntLsb
    #[inline]
    pub fn max_pic_order_cnt_lsb(&self) -> u32 {
        1 << self.log2_max_pic_order_cnt_lsb
    }

    /// Get maximum number of sub-layers
    #[inline]
    pub fn max_sub_layers(&self) -> u8 {
        self.sps_max_sub_layers
    }

    /// Get log2 minimum coding block size
    #[inline]
    pub fn log2_min_cb_size(&self) -> u32 {
        self.log2_min_luma_coding_block_size
    }

    /// Get log2 CTB size (Coding Tree Block)
    #[inline]
    pub fn log2_ctb_size(&self) -> u32 {
        self.log2_min_luma_coding_block_size + self.log2_diff_max_min_luma_coding_block_size
    }

    /// Get minimum coding block size in pixels
    #[inline]
    pub fn min_cb_size(&self) -> u32 {
        1 << self.log2_min_cb_size()
    }

    /// Get CTB (Coding Tree Block) size in pixels
    #[inline]
    pub fn ctb_size(&self) -> u32 {
        1 << self.log2_ctb_size()
    }

    /// Get picture width in CTBs
    #[inline]
    pub fn pic_width_in_ctbs(&self) -> u32 {
        self.pic_width_in_luma_samples.div_ceil(self.ctb_size())
    }

    /// Get picture height in CTBs
    #[inline]
    pub fn pic_height_in_ctbs(&self) -> u32 {
        self.pic_height_in_luma_samples.div_ceil(self.ctb_size())
    }

    /// Get total picture size in CTBs (width * height)
    #[inline]
    pub fn pic_size_in_ctbs(&self) -> u32 {
        self.pic_width_in_ctbs() * self.pic_height_in_ctbs()
    }

    /// Get the actual frame width in pixels (after conformance window cropping)
    pub fn width(&self) -> u32 {
        let sub_width_c = self.sub_width_c();
        self.pic_width_in_luma_samples - sub_width_c * (self.conf_win_left_offset + self.conf_win_right_offset)
    }

    /// Get the actual frame height in pixels (after conformance window
    /// cropping)
    pub fn height(&self) -> u32 {
        let sub_height_c = self.sub_height_c();
        self.pic_height_in_luma_samples - sub_height_c * (self.conf_win_top_offset + self.conf_win_bottom_offset)
    }

    /// Get SubWidthC based on chroma format
    fn sub_width_c(&self) -> u32 {
        if self.separate_colour_plane_flag {
            return 1;
        }
        match self.chroma_format {
            ChromaFormat::Monochrome => 1,
            ChromaFormat::YUV420 => 2,
            ChromaFormat::YUV422 => 2,
            ChromaFormat::YUV444 => 1,
        }
    }

    /// Get SubHeightC based on chroma format
    fn sub_height_c(&self) -> u32 {
        if self.separate_colour_plane_flag {
            return 1;
        }
        match self.chroma_format {
            ChromaFormat::Monochrome => 1,
            ChromaFormat::YUV420 => 2,
            ChromaFormat::YUV422 => 1,
            ChromaFormat::YUV444 => 1,
        }
    }

    /// Get sample aspect ratio (SAR) if available
    pub fn sample_aspect_ratio(&self) -> Option<(u16, u16)> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.sample_aspect_ratio())
    }

    /// Get frame rate as (numerator, denominator) if available
    pub fn frame_rate(&self) -> Option<(u32, u32)> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.frame_rate())
    }

    /// Get frame rate as floating point
    pub fn frame_rate_fps(&self) -> Option<f64> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.frame_rate_fps())
    }

    /// Check if video uses full range
    pub fn is_full_range(&self) -> bool {
        self.vui_parameters.as_ref().is_some_and(|vui_params| vui_params.is_full_range())
    }

    /// Get colour description if available
    pub fn colour_description(&self) -> Option<&ColourDescription> {
        self.vui_parameters.as_ref().and_then(|vui_params| vui_params.colour_description())
    }

    /// Get the Profile from ProfileTierLevel
    pub fn profile(&self) -> Option<Profile> {
        Profile::from_profile_tier_level(&self.profile_tier_level)
    }

    /// Get the Tier (Main or High)
    pub fn tier(&self) -> Tier {
        Tier::from(self.profile_tier_level.general_tier_flag)
    }

    /// Get the Level as a floating point value (e.g., 5.1)
    pub fn level(&self) -> f32 {
        self.profile_tier_level.level()
    }

    /// Get the general_level_idc value
    pub fn level_idc(&self) -> u8 {
        self.profile_tier_level.general_level_idc
    }
}
