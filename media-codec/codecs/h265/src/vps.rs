//! H.265/HEVC Video Parameter Set (VPS) parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, Result};
use smallvec::SmallVec;

/// Maximum number of sub-layers
const MAX_SUB_LAYERS: usize = 7;
/// Maximum CPB count
const MAX_CPB_CNT: usize = 32;

/// Profile Tier Level information
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProfileTierLevel {
    /// General profile space (2 bits)
    pub general_profile_space: u8,
    /// General tier flag (0=Main tier, 1=High tier)
    pub general_tier_flag: bool,
    /// General profile IDC
    pub general_profile_idc: u8,
    /// General profile compatibility flags (32 flags)
    pub general_profile_compatibility_flags: u32,
    /// General progressive source flag
    pub general_progressive_source_flag: bool,
    /// General interlaced source flag
    pub general_interlaced_source_flag: bool,
    /// General non-packed constraint flag
    pub general_non_packed_constraint_flag: bool,
    /// General frame only constraint flag
    pub general_frame_only_constraint_flag: bool,
    /// General constraint indicator flags (44 bits stored as upper 44 bits)
    pub general_constraint_indicator_flags: u64,
    /// General level IDC
    pub general_level_idc: u8,
    /// Sub-layer profile present flags
    pub sub_layer_profile_present_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer level present flags
    pub sub_layer_level_present_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer profile space
    pub sub_layer_profile_space: SmallVec<[u8; MAX_SUB_LAYERS]>,
    /// Sub-layer tier flag
    pub sub_layer_tier_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer profile IDC
    pub sub_layer_profile_idc: SmallVec<[u8; MAX_SUB_LAYERS]>,
    /// Sub-layer profile compatibility flags
    pub sub_layer_profile_compatibility_flags: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Sub-layer progressive source flag
    pub sub_layer_progressive_source_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer interlaced source flag
    pub sub_layer_interlaced_source_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer non-packed constraint flag
    pub sub_layer_non_packed_constraint_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer frame only constraint flag
    pub sub_layer_frame_only_constraint_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Sub-layer constraint indicator flags
    pub sub_layer_constraint_indicator_flags: SmallVec<[u64; MAX_SUB_LAYERS]>,
    /// Sub-layer level IDC
    pub sub_layer_level_idc: SmallVec<[u8; MAX_SUB_LAYERS]>,
}

impl ProfileTierLevel {
    /// Parse ProfileTierLevel from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, profile_present_flag: bool, max_num_sub_layers_minus1: u8) -> Result<Self> {
        let mut ptl = Self::default();

        if profile_present_flag {
            // general_profile_space (2 bits)
            ptl.general_profile_space = reader.read::<2, u8>()?;
            // general_tier_flag (1 bit)
            ptl.general_tier_flag = reader.read_bit()?;
            // general_profile_idc (5 bits)
            ptl.general_profile_idc = reader.read::<5, u8>()?;
            // general_profile_compatibility_flag[32] (32 bits)
            ptl.general_profile_compatibility_flags = reader.read::<32, u32>()?;
            // general_progressive_source_flag
            ptl.general_progressive_source_flag = reader.read_bit()?;
            // general_interlaced_source_flag
            ptl.general_interlaced_source_flag = reader.read_bit()?;
            // general_non_packed_constraint_flag
            ptl.general_non_packed_constraint_flag = reader.read_bit()?;
            // general_frame_only_constraint_flag
            ptl.general_frame_only_constraint_flag = reader.read_bit()?;
            // general_reserved_zero_44bits (44 bits - constraint indicator flags)
            let high = reader.read::<32, u64>()?;
            let low = reader.read::<12, u64>()?;
            ptl.general_constraint_indicator_flags = (high << 12) | low;
        }

        // general_level_idc (8 bits)
        ptl.general_level_idc = reader.read::<8, u8>()?;

        // Sub-layer flags
        let num_sub_layers = max_num_sub_layers_minus1 as usize;
        ptl.sub_layer_profile_present_flag.reserve(num_sub_layers);
        ptl.sub_layer_level_present_flag.reserve(num_sub_layers);

        for _ in 0..num_sub_layers {
            ptl.sub_layer_profile_present_flag.push(reader.read_bit()?);
            ptl.sub_layer_level_present_flag.push(reader.read_bit()?);
        }

        // Reserved bits for alignment if max_num_sub_layers_minus1 > 0
        if max_num_sub_layers_minus1 > 0 {
            for _ in num_sub_layers..8 {
                let _ = reader.read::<2, u8>()?; // reserved_zero_2bits
            }
        }

        // Sub-layer profile and level data
        ptl.sub_layer_profile_space = smallvec::smallvec![0; num_sub_layers];
        ptl.sub_layer_tier_flag = smallvec::smallvec![false; num_sub_layers];
        ptl.sub_layer_profile_idc = smallvec::smallvec![0; num_sub_layers];
        ptl.sub_layer_profile_compatibility_flags = smallvec::smallvec![0; num_sub_layers];
        ptl.sub_layer_progressive_source_flag = smallvec::smallvec![false; num_sub_layers];
        ptl.sub_layer_interlaced_source_flag = smallvec::smallvec![false; num_sub_layers];
        ptl.sub_layer_non_packed_constraint_flag = smallvec::smallvec![false; num_sub_layers];
        ptl.sub_layer_frame_only_constraint_flag = smallvec::smallvec![false; num_sub_layers];
        ptl.sub_layer_constraint_indicator_flags = smallvec::smallvec![0; num_sub_layers];
        ptl.sub_layer_level_idc = smallvec::smallvec![0; num_sub_layers];

        for i in 0..num_sub_layers {
            if ptl.sub_layer_profile_present_flag[i] {
                ptl.sub_layer_profile_space[i] = reader.read::<2, u8>()?;
                ptl.sub_layer_tier_flag[i] = reader.read_bit()?;
                ptl.sub_layer_profile_idc[i] = reader.read::<5, u8>()?;
                ptl.sub_layer_profile_compatibility_flags[i] = reader.read::<32, u32>()?;
                ptl.sub_layer_progressive_source_flag[i] = reader.read_bit()?;
                ptl.sub_layer_interlaced_source_flag[i] = reader.read_bit()?;
                ptl.sub_layer_non_packed_constraint_flag[i] = reader.read_bit()?;
                ptl.sub_layer_frame_only_constraint_flag[i] = reader.read_bit()?;
                let high = reader.read::<32, u64>()?;
                let low = reader.read::<12, u64>()?;
                ptl.sub_layer_constraint_indicator_flags[i] = (high << 12) | low;
            }
            if ptl.sub_layer_level_present_flag[i] {
                ptl.sub_layer_level_idc[i] = reader.read::<8, u8>()?;
            }
        }

        Ok(ptl)
    }

    /// Check if general profile compatibility flag is set
    #[inline]
    pub fn is_profile_compatible(&self, profile_idc: u8) -> bool {
        if profile_idc < 32 {
            (self.general_profile_compatibility_flags >> (31 - profile_idc)) & 1 != 0
        } else {
            false
        }
    }

    /// Check if this is Main profile
    #[inline]
    pub fn is_main_profile(&self) -> bool {
        self.general_profile_idc == 1 || self.is_profile_compatible(1)
    }

    /// Check if this is Main 10 profile
    #[inline]
    pub fn is_main_10_profile(&self) -> bool {
        self.general_profile_idc == 2 || self.is_profile_compatible(2)
    }

    /// Check if this is Main Still Picture profile
    #[inline]
    pub fn is_main_still_picture_profile(&self) -> bool {
        self.general_profile_idc == 3 || self.is_profile_compatible(3)
    }

    /// Check if this is Range Extensions profile
    #[inline]
    pub fn is_range_extensions_profile(&self) -> bool {
        self.general_profile_idc == 4 || self.is_profile_compatible(4)
    }

    /// Get level as floating point (e.g., 5.1 = 153 / 30 = 5.1)
    #[inline]
    pub fn level(&self) -> f32 {
        self.general_level_idc as f32 / 30.0
    }

    /// Check if high tier
    #[inline]
    pub fn is_high_tier(&self) -> bool {
        self.general_tier_flag
    }
}

/// Sub-layer HRD parameters
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubLayerHrdParameters {
    /// Bit rate values
    pub bit_rate_value: SmallVec<[u32; MAX_CPB_CNT]>,
    /// CPB size values
    pub cpb_size_value: SmallVec<[u32; MAX_CPB_CNT]>,
    /// CPB size DU values (if sub_pic_hrd_params_present_flag)
    pub cpb_size_du_value: SmallVec<[u32; MAX_CPB_CNT]>,
    /// Bit rate DU values (if sub_pic_hrd_params_present_flag)
    pub bit_rate_du_value: SmallVec<[u32; MAX_CPB_CNT]>,
    /// CBR flags
    pub cbr_flag: SmallVec<[bool; MAX_CPB_CNT]>,
}

impl SubLayerHrdParameters {
    /// Parse SubLayerHrdParameters from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, cpb_cnt: usize, sub_pic_hrd_params_present_flag: bool) -> Result<Self> {
        let mut params = Self::default();
        params.bit_rate_value.reserve(cpb_cnt);
        params.cpb_size_value.reserve(cpb_cnt);
        params.cpb_size_du_value.reserve(cpb_cnt);
        params.bit_rate_du_value.reserve(cpb_cnt);
        params.cbr_flag.reserve(cpb_cnt);

        for _ in 0..cpb_cnt {
            params.bit_rate_value.push(reader.read_ue()? + 1);
            params.cpb_size_value.push(reader.read_ue()? + 1);
            if sub_pic_hrd_params_present_flag {
                params.cpb_size_du_value.push(reader.read_ue()? + 1);
                params.bit_rate_du_value.push(reader.read_ue()? + 1);
            }
            params.cbr_flag.push(reader.read_bit()?);
        }

        Ok(params)
    }
}

/// HRD (Hypothetical Reference Decoder) Parameters
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HrdParameters {
    /// NAL HRD parameters present flag
    pub nal_hrd_parameters_present_flag: bool,
    /// VCL HRD parameters present flag
    pub vcl_hrd_parameters_present_flag: bool,
    /// Sub-picture HRD parameters present flag
    pub sub_pic_hrd_params_present_flag: bool,
    /// Tick divisor
    pub tick_divisor: u8,
    /// DU CPB removal delay increment length
    pub du_cpb_removal_delay_increment_length: u8,
    /// Sub-picture CPB params in pic timing SEI flag
    pub sub_pic_cpb_params_in_pic_timing_sei_flag: bool,
    /// DPB output delay DU length
    pub dpb_output_delay_du_length: u8,
    /// Bit rate scale
    pub bit_rate_scale: u8,
    /// CPB size scale
    pub cpb_size_scale: u8,
    /// CPB size DU scale
    pub cpb_size_du_scale: u8,
    /// Initial CPB removal delay length
    pub initial_cpb_removal_delay_length: u8,
    /// AU CPB removal delay length
    pub au_cpb_removal_delay_length: u8,
    /// DPB output delay length
    pub dpb_output_delay_length: u8,
    /// Fixed picture rate general flags per sub-layer
    pub fixed_pic_rate_general_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Fixed picture rate within CVS flags per sub-layer
    pub fixed_pic_rate_within_cvs_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// Elemental duration in TC per sub-layer
    pub elemental_duration_in_tc: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Low delay HRD flags per sub-layer
    pub low_delay_hrd_flag: SmallVec<[bool; MAX_SUB_LAYERS]>,
    /// CPB count per sub-layer
    pub cpb_cnt: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// NAL sub-layer HRD parameters
    pub nal_sub_layer_hrd_parameters: SmallVec<[SubLayerHrdParameters; MAX_SUB_LAYERS]>,
    /// VCL sub-layer HRD parameters
    pub vcl_sub_layer_hrd_parameters: SmallVec<[SubLayerHrdParameters; MAX_SUB_LAYERS]>,
}

impl HrdParameters {
    /// Parse HrdParameters from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, common_inf_present_flag: bool, max_num_sub_layers_minus1: u8) -> Result<Self> {
        let mut hrd = Self::default();

        if common_inf_present_flag {
            hrd.nal_hrd_parameters_present_flag = reader.read_bit()?;
            hrd.vcl_hrd_parameters_present_flag = reader.read_bit()?;

            if hrd.nal_hrd_parameters_present_flag || hrd.vcl_hrd_parameters_present_flag {
                hrd.sub_pic_hrd_params_present_flag = reader.read_bit()?;

                if hrd.sub_pic_hrd_params_present_flag {
                    hrd.tick_divisor = reader.read::<8, u8>()? + 2;
                    hrd.du_cpb_removal_delay_increment_length = reader.read::<5, u8>()? + 1;
                    hrd.sub_pic_cpb_params_in_pic_timing_sei_flag = reader.read_bit()?;
                    hrd.dpb_output_delay_du_length = reader.read::<5, u8>()? + 1;
                }

                hrd.bit_rate_scale = reader.read::<4, u8>()?;
                hrd.cpb_size_scale = reader.read::<4, u8>()?;

                if hrd.sub_pic_hrd_params_present_flag {
                    hrd.cpb_size_du_scale = reader.read::<4, u8>()?;
                }

                hrd.initial_cpb_removal_delay_length = reader.read::<5, u8>()? + 1;
                hrd.au_cpb_removal_delay_length = reader.read::<5, u8>()? + 1;
                hrd.dpb_output_delay_length = reader.read::<5, u8>()? + 1;
            }
        }

        let num_sub_layers = (max_num_sub_layers_minus1 + 1) as usize;
        hrd.fixed_pic_rate_general_flag = smallvec::smallvec![false; num_sub_layers];
        hrd.fixed_pic_rate_within_cvs_flag = smallvec::smallvec![false; num_sub_layers];
        hrd.elemental_duration_in_tc = smallvec::smallvec![0; num_sub_layers];
        hrd.low_delay_hrd_flag = smallvec::smallvec![false; num_sub_layers];
        hrd.cpb_cnt = smallvec::smallvec![0; num_sub_layers];
        hrd.nal_sub_layer_hrd_parameters.reserve(num_sub_layers);
        hrd.vcl_sub_layer_hrd_parameters.reserve(num_sub_layers);

        for i in 0..num_sub_layers {
            hrd.fixed_pic_rate_general_flag[i] = reader.read_bit()?;

            if !hrd.fixed_pic_rate_general_flag[i] {
                hrd.fixed_pic_rate_within_cvs_flag[i] = reader.read_bit()?;
            } else {
                hrd.fixed_pic_rate_within_cvs_flag[i] = true;
            }

            if hrd.fixed_pic_rate_within_cvs_flag[i] {
                hrd.elemental_duration_in_tc[i] = reader.read_ue()? + 1;
            } else {
                hrd.low_delay_hrd_flag[i] = reader.read_bit()?;
            }

            if !hrd.low_delay_hrd_flag[i] {
                hrd.cpb_cnt[i] = reader.read_ue()? + 1;
            }

            let cpb_cnt = hrd.cpb_cnt[i] as usize;

            if hrd.nal_hrd_parameters_present_flag {
                hrd.nal_sub_layer_hrd_parameters.push(SubLayerHrdParameters::parse(reader, cpb_cnt, hrd.sub_pic_hrd_params_present_flag)?);
            }

            if hrd.vcl_hrd_parameters_present_flag {
                hrd.vcl_sub_layer_hrd_parameters.push(SubLayerHrdParameters::parse(reader, cpb_cnt, hrd.sub_pic_hrd_params_present_flag)?);
            }
        }

        Ok(hrd)
    }

    /// Get initial CPB removal delay length in bits
    #[inline]
    pub fn initial_cpb_removal_delay_length(&self) -> u8 {
        self.initial_cpb_removal_delay_length
    }

    /// Get AU CPB removal delay length in bits
    #[inline]
    pub fn au_cpb_removal_delay_length(&self) -> u8 {
        self.au_cpb_removal_delay_length
    }

    /// Get DPB output delay length in bits
    #[inline]
    pub fn dpb_output_delay_length(&self) -> u8 {
        self.dpb_output_delay_length
    }
}

/// Video Parameter Set (VPS)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Vps {
    /// VPS ID (0-15)
    pub video_parameter_set_id: u8,
    /// VPS base layer internal flag
    pub vps_base_layer_internal_flag: bool,
    /// VPS base layer available flag
    pub vps_base_layer_available_flag: bool,
    /// Maximum number of layers (0-62)
    pub vps_max_layers: u8,
    /// Maximum number of sub-layers (0-6)
    pub vps_max_sub_layers: u8,
    /// Temporal ID nesting flag
    pub vps_temporal_id_nesting_flag: bool,
    /// Profile tier level
    pub profile_tier_level: ProfileTierLevel,
    /// VPS sub-layer ordering info present flag
    pub vps_sub_layer_ordering_info_present_flag: bool,
    /// Maximum decoder buffer size per sub-layer
    pub vps_max_dec_pic_buffering: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Maximum number of reorder pictures per sub-layer
    pub vps_max_num_reorder_pics: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Maximum latency increase plus 1 per sub-layer
    pub vps_max_latency_increase_plus1: SmallVec<[u32; MAX_SUB_LAYERS]>,
    /// Maximum layer ID
    pub vps_max_layer_id: u8,
    /// Number of layer sets
    pub vps_num_layer_sets: u32,
    /// Layer ID included flags - kept as Vec<Vec> since outer size is variable
    pub layer_id_included_flag: Vec<Vec<bool>>,
    /// VPS timing info present flag
    pub vps_timing_info_present_flag: bool,
    /// VPS number of units in tick
    pub vps_num_units_in_tick: u32,
    /// VPS time scale
    pub vps_time_scale: u32,
    /// VPS POC proportional to timing flag
    pub vps_poc_proportional_to_timing_flag: bool,
    /// VPS number of ticks POC diff one
    pub vps_num_ticks_poc_diff_one: u32,
    /// VPS number of HRD parameters
    pub vps_num_hrd_parameters: u32,
    /// HRD layer set indices - kept as Vec since count is variable
    pub hrd_layer_set_idx: Vec<u32>,
    /// CPRMS present flags - kept as Vec since count is variable
    pub cprms_present_flag: Vec<bool>,
    /// HRD parameters - kept as Vec since count is variable
    pub hrd_parameters: Vec<HrdParameters>,
    /// VPS extension flag
    pub vps_extension_flag: bool,
}

impl Vps {
    /// Parse VPS from raw NAL unit RBSP data (with EPB already removed)
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader)
    }

    /// Parse VPS from a BitReader
    pub fn parse_from_bit_reader<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        // vps_video_parameter_set_id (4 bits)
        let vps_video_parameter_set_id = reader.read::<4, u8>()?;
        if vps_video_parameter_set_id > 15 {
            return Err(invalid_data_error!("vps_video_parameter_set_id", vps_video_parameter_set_id));
        }

        // vps_base_layer_internal_flag (1 bit)
        let vps_base_layer_internal_flag = reader.read_bit()?;
        // vps_base_layer_available_flag (1 bit)
        let vps_base_layer_available_flag = reader.read_bit()?;
        // vps_max_layers (6 bits, stored as minus1)
        let vps_max_layers = reader.read::<6, u8>()? + 1;
        // vps_max_sub_layers (3 bits, stored as minus1)
        let vps_max_sub_layers_minus1 = reader.read::<3, u8>()?;
        if vps_max_sub_layers_minus1 > 6 {
            return Err(invalid_data_error!("vps_max_sub_layers", vps_max_sub_layers_minus1 + 1));
        }
        let vps_max_sub_layers = vps_max_sub_layers_minus1 + 1;
        // vps_temporal_id_nesting_flag (1 bit)
        let vps_temporal_id_nesting_flag = reader.read_bit()?;

        // vps_reserved_0xffff_16bits (16 bits) - must be 0xFFFF
        let reserved = reader.read::<16, u16>()?;
        if reserved != 0xFFFF {
            return Err(invalid_data_error!("vps_reserved_0xffff_16bits", reserved));
        }

        // profile_tier_level
        let profile_tier_level = ProfileTierLevel::parse(reader, true, vps_max_sub_layers_minus1)?;

        // vps_sub_layer_ordering_info_present_flag
        let vps_sub_layer_ordering_info_present_flag = reader.read_bit()?;

        // Sub-layer ordering info
        let start_idx = if vps_sub_layer_ordering_info_present_flag {
            0
        } else {
            vps_max_sub_layers_minus1 as usize
        };
        let num_sub_layers = vps_max_sub_layers as usize;

        let mut vps_max_dec_pic_buffering = smallvec::smallvec![0u32; num_sub_layers];
        let mut vps_max_num_reorder_pics = smallvec::smallvec![0u32; num_sub_layers];
        let mut vps_max_latency_increase_plus1 = smallvec::smallvec![0u32; num_sub_layers];

        for i in start_idx..num_sub_layers {
            vps_max_dec_pic_buffering[i] = reader.read_ue()? + 1;
            vps_max_num_reorder_pics[i] = reader.read_ue()?;
            vps_max_latency_increase_plus1[i] = reader.read_ue()?;
        }

        // Fill in lower sub-layers if not present
        if !vps_sub_layer_ordering_info_present_flag {
            for i in 0..start_idx {
                vps_max_dec_pic_buffering[i] = vps_max_dec_pic_buffering[start_idx];
                vps_max_num_reorder_pics[i] = vps_max_num_reorder_pics[start_idx];
                vps_max_latency_increase_plus1[i] = vps_max_latency_increase_plus1[start_idx];
            }
        }

        // vps_max_layer_id (6 bits)
        let vps_max_layer_id = reader.read::<6, u8>()?;
        // vps_num_layer_sets
        let vps_num_layer_sets = reader.read_ue()? + 1;

        // layer_id_included_flag
        let mut layer_id_included_flag = Vec::with_capacity(vps_num_layer_sets as usize);
        // Layer set 0 is implicit
        layer_id_included_flag.push(vec![true]); // layer 0 in set 0

        for _ in 1..vps_num_layer_sets {
            let mut layer_flags = Vec::with_capacity((vps_max_layer_id + 1) as usize);
            for _ in 0..=vps_max_layer_id {
                layer_flags.push(reader.read_bit()?);
            }
            layer_id_included_flag.push(layer_flags);
        }

        // vps_timing_info_present_flag
        let vps_timing_info_present_flag = reader.read_bit()?;

        let mut vps_num_units_in_tick = 0;
        let mut vps_time_scale = 0;
        let mut vps_poc_proportional_to_timing_flag = false;
        let mut vps_num_ticks_poc_diff_one = 0;
        let mut vps_num_hrd_parameters = 0;
        let mut hrd_layer_set_idx = Vec::new();
        let mut cprms_present_flag = Vec::new();
        let mut hrd_parameters = Vec::new();

        if vps_timing_info_present_flag {
            vps_num_units_in_tick = reader.read::<32, u32>()?;
            vps_time_scale = reader.read::<32, u32>()?;
            vps_poc_proportional_to_timing_flag = reader.read_bit()?;

            if vps_poc_proportional_to_timing_flag {
                vps_num_ticks_poc_diff_one = reader.read_ue()? + 1;
            }

            vps_num_hrd_parameters = reader.read_ue()?;

            for i in 0..vps_num_hrd_parameters as usize {
                hrd_layer_set_idx.push(reader.read_ue()?);

                let cprms_flag = if i > 0 {
                    reader.read_bit()?
                } else {
                    true
                };
                cprms_present_flag.push(cprms_flag);

                hrd_parameters.push(HrdParameters::parse(reader, cprms_flag, vps_max_sub_layers_minus1)?);
            }
        }

        // vps_extension_flag
        let vps_extension_flag = reader.read_bit()?;

        Ok(Self {
            video_parameter_set_id: vps_video_parameter_set_id,
            vps_base_layer_internal_flag,
            vps_base_layer_available_flag,
            vps_max_layers,
            vps_max_sub_layers,
            vps_temporal_id_nesting_flag,
            profile_tier_level,
            vps_sub_layer_ordering_info_present_flag,
            vps_max_dec_pic_buffering,
            vps_max_num_reorder_pics,
            vps_max_latency_increase_plus1,
            vps_max_layer_id,
            vps_num_layer_sets,
            layer_id_included_flag,
            vps_timing_info_present_flag,
            vps_num_units_in_tick,
            vps_time_scale,
            vps_poc_proportional_to_timing_flag,
            vps_num_ticks_poc_diff_one,
            vps_num_hrd_parameters,
            hrd_layer_set_idx,
            cprms_present_flag,
            hrd_parameters,
            vps_extension_flag,
        })
    }

    /// Get maximum number of layers
    #[inline]
    pub fn max_layers(&self) -> u8 {
        self.vps_max_layers
    }

    /// Get maximum number of sub-layers
    #[inline]
    pub fn max_sub_layers(&self) -> u8 {
        self.vps_max_sub_layers
    }

    /// Get maximum decoded picture buffering for a sub-layer
    #[inline]
    pub fn max_dec_pic_buffering(&self, sub_layer: usize) -> Option<u32> {
        self.vps_max_dec_pic_buffering.get(sub_layer).copied()
    }

    /// Get maximum number of reorder pictures for a sub-layer
    #[inline]
    pub fn max_num_reorder_pics(&self, sub_layer: usize) -> Option<u32> {
        self.vps_max_num_reorder_pics.get(sub_layer).copied()
    }

    /// Get frame rate as (numerator, denominator) if timing info is present
    pub fn frame_rate(&self) -> Option<(u32, u32)> {
        if self.vps_timing_info_present_flag && self.vps_num_units_in_tick > 0 {
            Some((self.vps_time_scale, self.vps_num_units_in_tick))
        } else {
            None
        }
    }

    /// Get frame rate as floating point
    pub fn frame_rate_fps(&self) -> Option<f64> {
        self.frame_rate().map(|(num, den)| num as f64 / den as f64)
    }

    /// Get the number of layer sets
    #[inline]
    pub fn num_layer_sets(&self) -> u32 {
        self.vps_num_layer_sets
    }
}
