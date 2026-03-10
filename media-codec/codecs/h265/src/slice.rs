//! H.265/HEVC Slice Segment Header parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, not_found_error, Result};
use smallvec::SmallVec;

use crate::{
    constants::{MAX_PPS_COUNT, MAX_REFS},
    nal::NalUnitType,
    pps::Pps,
    ps::ParameterSets,
    sps::{ShortTermRefPicSet, Sps},
};

/// Slice type as defined in ITU-T H.265 Table 7-7
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum SliceType {
    /// B slice (bi-predictive)
    B = 0,
    /// P slice (predictive)
    P = 1,
    /// I slice (intra)
    #[default]
    I = 2,
}

impl SliceType {
    /// Create from raw slice type value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::B),
            1 => Some(Self::P),
            2 => Some(Self::I),
            _ => None,
        }
    }

    /// Check if this is an intra slice
    #[inline]
    pub fn is_intra(&self) -> bool {
        matches!(self, Self::I)
    }

    /// Check if this is an inter slice (P or B)
    #[inline]
    pub fn is_inter(&self) -> bool {
        !self.is_intra()
    }

    /// Check if this is a B slice
    #[inline]
    pub fn is_b(&self) -> bool {
        matches!(self, Self::B)
    }

    /// Check if this is a P slice
    #[inline]
    pub fn is_p(&self) -> bool {
        matches!(self, Self::P)
    }
}

/// Reference picture list modification
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RefPicListModification {
    /// Reference picture list modification flag for L0
    pub ref_pic_list_modification_flag_l0: bool,
    /// List entry for L0
    pub list_entry_l0: SmallVec<[u32; MAX_REFS]>,
    /// Reference picture list modification flag for L1
    pub ref_pic_list_modification_flag_l1: bool,
    /// List entry for L1
    pub list_entry_l1: SmallVec<[u32; MAX_REFS]>,
}

impl RefPicListModification {
    /// Parse reference picture list modification from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, header: &SliceSegmentHeader) -> Result<Option<Self>> {
        let mut rplm = Self::default();

        // Calculate NumPocTotalCurr (simplified - just use ref_idx counts)
        let num_poc_total_curr = header.num_ref_idx_l0_active +
            if header.slice_type.is_b() {
                header.num_ref_idx_l1_active
            } else {
                0
            };

        if num_poc_total_curr > 1 {
            let bits_needed = (32 - num_poc_total_curr.leading_zeros()).max(1);

            rplm.ref_pic_list_modification_flag_l0 = reader.read_bit()?;
            if rplm.ref_pic_list_modification_flag_l0 {
                rplm.list_entry_l0 = SmallVec::with_capacity(header.num_ref_idx_l0_active as usize);
                for _ in 0..header.num_ref_idx_l0_active {
                    rplm.list_entry_l0.push(reader.read_var(bits_needed)?);
                }
            }

            if header.slice_type.is_b() {
                rplm.ref_pic_list_modification_flag_l1 = reader.read_bit()?;
                if rplm.ref_pic_list_modification_flag_l1 {
                    rplm.list_entry_l1 = SmallVec::with_capacity(header.num_ref_idx_l1_active as usize);
                    for _ in 0..header.num_ref_idx_l1_active {
                        rplm.list_entry_l1.push(reader.read_var(bits_needed)?);
                    }
                }
            }
        }

        if rplm.ref_pic_list_modification_flag_l0 || rplm.ref_pic_list_modification_flag_l1 {
            Ok(Some(rplm))
        } else {
            Ok(None)
        }
    }
}

/// Prediction weight table
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PredWeightTable {
    /// Luma log2 weight denom
    pub luma_log2_weight_denom: u32,
    /// Delta chroma log2 weight denom
    pub delta_chroma_log2_weight_denom: i32,
    /// Luma weight L0
    pub luma_weight_l0: SmallVec<[i32; MAX_REFS]>,
    /// Luma offset L0
    pub luma_offset_l0: SmallVec<[i32; MAX_REFS]>,
    /// Chroma weight L0
    pub chroma_weight_l0: SmallVec<[[i32; 2]; MAX_REFS]>,
    /// Chroma offset L0
    pub chroma_offset_l0: SmallVec<[[i32; 2]; MAX_REFS]>,
    /// Luma weight L1
    pub luma_weight_l1: SmallVec<[i32; MAX_REFS]>,
    /// Luma offset L1
    pub luma_offset_l1: SmallVec<[i32; MAX_REFS]>,
    /// Chroma weight L1
    pub chroma_weight_l1: SmallVec<[[i32; 2]; MAX_REFS]>,
    /// Chroma offset L1
    pub chroma_offset_l1: SmallVec<[[i32; 2]; MAX_REFS]>,
}

impl PredWeightTable {
    /// Parse prediction weight table from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, header: &SliceSegmentHeader, sps: &Sps) -> Result<Self> {
        let mut pwt = Self {
            // luma_log2_weight_denom
            luma_log2_weight_denom: reader.read_ue()?,
            ..Default::default()
        };

        // delta_chroma_log2_weight_denom
        let chroma_array_type = if sps.separate_colour_plane_flag {
            0
        } else {
            sps.chroma_format as u8
        };
        if chroma_array_type != 0 {
            pwt.delta_chroma_log2_weight_denom = reader.read_se()?;
        }

        let num_l0 = header.num_ref_idx_l0_active as usize;

        // L0 weights
        let mut luma_weight_l0_flag = SmallVec::<[bool; MAX_REFS]>::with_capacity(num_l0);
        for _ in 0..num_l0 {
            luma_weight_l0_flag.push(reader.read_bit()?);
        }

        let mut chroma_weight_l0_flag = SmallVec::<[bool; MAX_REFS]>::with_capacity(num_l0);
        if chroma_array_type != 0 {
            for _ in 0..num_l0 {
                chroma_weight_l0_flag.push(reader.read_bit()?);
            }
        }

        pwt.luma_weight_l0 = SmallVec::with_capacity(num_l0);
        pwt.luma_offset_l0 = SmallVec::with_capacity(num_l0);
        pwt.chroma_weight_l0 = SmallVec::with_capacity(num_l0);
        pwt.chroma_offset_l0 = SmallVec::with_capacity(num_l0);

        let default_luma_weight = 1i32 << pwt.luma_log2_weight_denom;

        for i in 0..num_l0 {
            if luma_weight_l0_flag[i] {
                pwt.luma_weight_l0.push(reader.read_se()? + default_luma_weight);
                pwt.luma_offset_l0.push(reader.read_se()?);
            } else {
                pwt.luma_weight_l0.push(default_luma_weight);
                pwt.luma_offset_l0.push(0);
            }

            if chroma_array_type != 0 {
                if i < chroma_weight_l0_flag.len() && chroma_weight_l0_flag[i] {
                    let w0 = reader.read_se()?;
                    let o0 = reader.read_se()?;
                    let w1 = reader.read_se()?;
                    let o1 = reader.read_se()?;
                    pwt.chroma_weight_l0.push([w0, w1]);
                    pwt.chroma_offset_l0.push([o0, o1]);
                } else {
                    pwt.chroma_weight_l0.push([0, 0]);
                    pwt.chroma_offset_l0.push([0, 0]);
                }
            }
        }

        // L1 weights (for B slices)
        if header.slice_type.is_b() {
            let num_l1 = header.num_ref_idx_l1_active as usize;

            let mut luma_weight_l1_flag = SmallVec::<[bool; MAX_REFS]>::with_capacity(num_l1);
            for _ in 0..num_l1 {
                luma_weight_l1_flag.push(reader.read_bit()?);
            }

            let mut chroma_weight_l1_flag = SmallVec::<[bool; MAX_REFS]>::with_capacity(num_l1);
            if chroma_array_type != 0 {
                for _ in 0..num_l1 {
                    chroma_weight_l1_flag.push(reader.read_bit()?);
                }
            }

            pwt.luma_weight_l1 = SmallVec::with_capacity(num_l1);
            pwt.luma_offset_l1 = SmallVec::with_capacity(num_l1);
            pwt.chroma_weight_l1 = SmallVec::with_capacity(num_l1);
            pwt.chroma_offset_l1 = SmallVec::with_capacity(num_l1);

            for i in 0..num_l1 {
                if luma_weight_l1_flag[i] {
                    pwt.luma_weight_l1.push(reader.read_se()? + default_luma_weight);
                    pwt.luma_offset_l1.push(reader.read_se()?);
                } else {
                    pwt.luma_weight_l1.push(default_luma_weight);
                    pwt.luma_offset_l1.push(0);
                }

                if chroma_array_type != 0 {
                    if i < chroma_weight_l1_flag.len() && chroma_weight_l1_flag[i] {
                        let w0 = reader.read_se()?;
                        let o0 = reader.read_se()?;
                        let w1 = reader.read_se()?;
                        let o1 = reader.read_se()?;
                        pwt.chroma_weight_l1.push([w0, w1]);
                        pwt.chroma_offset_l1.push([o0, o1]);
                    } else {
                        pwt.chroma_weight_l1.push([0, 0]);
                        pwt.chroma_offset_l1.push([0, 0]);
                    }
                }
            }
        }

        Ok(pwt)
    }
}

/// Slice Segment Header
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SliceSegmentHeader {
    /// First slice segment in picture flag
    pub first_slice_segment_in_pic_flag: bool,
    /// No output of prior pics flag (only for IRAP)
    pub no_output_of_prior_pics_flag: bool,
    /// PPS ID (0-63)
    pub pic_parameter_set_id: u8,
    /// Dependent slice segment flag
    pub dependent_slice_segment_flag: bool,
    /// Slice segment address
    pub slice_segment_address: u32,
    /// Slice type
    pub slice_type: SliceType,
    /// Picture output flag
    pub pic_output_flag: bool,
    /// Colour plane ID (for separate colour plane)
    pub colour_plane_id: u8,
    /// Slice picture order count LSB
    pub slice_pic_order_cnt_lsb: u32,
    /// Short-term ref pic set SPS flag
    pub short_term_ref_pic_set_sps_flag: bool,
    /// Short-term ref pic set index
    pub short_term_ref_pic_set_idx: u32,
    /// Short-term ref pic set (if not from SPS)
    pub short_term_ref_pic_set: Option<ShortTermRefPicSet>,
    /// Number of long-term SPS
    pub num_long_term_sps: u32,
    /// Number of long-term pics
    pub num_long_term_pics: u32,
    /// Slice temporal MVP enabled flag
    pub slice_temporal_mvp_enabled_flag: bool,
    /// Slice SAO luma flag
    pub slice_sao_luma_flag: bool,
    /// Slice SAO chroma flag
    pub slice_sao_chroma_flag: bool,
    /// Number of reference pictures in list 0
    pub num_ref_idx_l0_active: u32,
    /// Number of reference pictures in list 1
    pub num_ref_idx_l1_active: u32,
    /// Reference picture list modification
    pub ref_pic_list_modification: Option<RefPicListModification>,
    /// MVP L0 flag
    pub mvd_l1_zero_flag: bool,
    /// CABAC init flag
    pub cabac_init_flag: bool,
    /// Collocated from L0 flag
    pub collocated_from_l0_flag: bool,
    /// Collocated reference index
    pub collocated_ref_idx: u32,
    /// Prediction weight table
    pub pred_weight_table: Option<PredWeightTable>,
    /// Max number of merge candidates
    pub max_num_merge_cand: u32,
    /// Use integer MV flag (SCC extension)
    pub use_integer_mv_flag: bool,
    /// Slice QP delta
    pub slice_qp_delta: i32,
    /// Slice Cb QP offset
    pub slice_cb_qp_offset: i32,
    /// Slice Cr QP offset
    pub slice_cr_qp_offset: i32,
    /// Slice act Y QP offset (SCC extension)
    pub slice_act_y_qp_offset: i32,
    /// Slice act Cb QP offset (SCC extension)
    pub slice_act_cb_qp_offset: i32,
    /// Slice act Cr QP offset (SCC extension)
    pub slice_act_cr_qp_offset: i32,
    /// CU chroma QP offset enabled flag
    pub cu_chroma_qp_offset_enabled_flag: bool,
    /// Deblocking filter override flag
    pub deblocking_filter_override_flag: bool,
    /// Slice deblocking filter disabled flag
    pub slice_deblocking_filter_disabled_flag: bool,
    /// Slice beta offset
    pub slice_beta_offset: i32,
    /// Slice tc offset
    pub slice_tc_offset: i32,
    /// Slice loop filter across slices enabled flag
    pub slice_loop_filter_across_slices_enabled_flag: bool,
    /// Number of entry point offsets
    pub num_entry_point_offsets: u32,
    /// Entry point offset
    pub entry_point_offset: Vec<u32>,
}

impl SliceSegmentHeader {
    /// Parse slice segment header from raw NAL unit RBSP data
    pub fn parse(data: &[u8], nal_unit_type: NalUnitType, param_sets: &ParameterSets) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, nal_unit_type, param_sets)
    }

    /// Parse slice segment header from a BitReader
    pub fn parse_from_bit_reader<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        nal_unit_type: NalUnitType,
        param_sets: &ParameterSets,
    ) -> Result<Self> {
        let mut header = Self {
            // first_slice_segment_in_pic_flag
            first_slice_segment_in_pic_flag: reader.read_bit()?,
            ..Default::default()
        };

        // no_output_of_prior_pics_flag (only for BLA/IDR/CRA)
        if nal_unit_type.is_irap() {
            header.no_output_of_prior_pics_flag = reader.read_bit()?;
        }

        // Read pic_parameter_set_id
        let pic_parameter_set_id = reader.read_ue()?;
        if pic_parameter_set_id as usize >= MAX_PPS_COUNT {
            return Err(invalid_data_error!("pps_id", pic_parameter_set_id));
        }
        header.pic_parameter_set_id = pic_parameter_set_id as u8;

        let pps = param_sets.get_pps(header.pic_parameter_set_id as u32).ok_or_else(|| not_found_error!("pps_id", header.pic_parameter_set_id))?;
        let sps = param_sets.get_sps(pps.seq_parameter_set_id as u32).ok_or_else(|| not_found_error!("sps_id", pps.seq_parameter_set_id))?;

        // Read dependent_slice_segment_flag
        if !header.first_slice_segment_in_pic_flag {
            if pps.dependent_slice_segments_enabled_flag {
                header.dependent_slice_segment_flag = reader.read_bit()?;
            }

            // Read slice_segment_address
            let pic_size_in_ctbs_y = sps.pic_size_in_ctbs();
            let bits_needed = (32 - pic_size_in_ctbs_y.leading_zeros()).max(1);
            header.slice_segment_address = reader.read_var(bits_needed)?;
        }

        // If dependent slice, we're done with the header
        if header.dependent_slice_segment_flag {
            return Ok(header);
        }

        // Read slice_reserved_flags
        for _ in 0..pps.num_extra_slice_header_bits {
            let _ = reader.read_bit()?; // slice_reserved_flag[i]
        }

        // Read slice_type
        let slice_type_raw = reader.read_ue()? as u8;
        header.slice_type = SliceType::from_u8(slice_type_raw).ok_or_else(|| invalid_data_error!("slice_type", slice_type_raw))?;

        // Read pic_output_flag
        if pps.output_flag_present_flag {
            header.pic_output_flag = reader.read_bit()?;
        } else {
            header.pic_output_flag = true;
        }

        // Read colour_plane_id
        if sps.separate_colour_plane_flag {
            header.colour_plane_id = reader.read::<2, u8>()?;
        }

        // For non-IDR slices
        if !nal_unit_type.is_idr() {
            // Read slice_pic_order_cnt_lsb
            let poc_lsb_bits = sps.log2_max_pic_order_cnt_lsb as u32;
            header.slice_pic_order_cnt_lsb = reader.read_var(poc_lsb_bits)?;

            // Read short_term_ref_pic_set_sps_flag
            header.short_term_ref_pic_set_sps_flag = reader.read_bit()?;

            if !header.short_term_ref_pic_set_sps_flag {
                // Parse short_term_ref_pic_set
                header.short_term_ref_pic_set = Some(ShortTermRefPicSet::parse(
                    reader,
                    sps.num_short_term_ref_pic_sets as usize,
                    sps.num_short_term_ref_pic_sets as usize,
                    &sps.short_term_ref_pic_sets,
                )?);
            } else if sps.num_short_term_ref_pic_sets > 1 {
                // Read short_term_ref_pic_set_idx
                let bits_needed = (32 - sps.num_short_term_ref_pic_sets.leading_zeros()).max(1);
                header.short_term_ref_pic_set_idx = reader.read_var(bits_needed)?;
            }

            // Long-term reference pictures
            if sps.long_term_ref_pics_present_flag {
                if sps.num_long_term_ref_pics_sps > 0 {
                    header.num_long_term_sps = reader.read_ue()?;
                }
                header.num_long_term_pics = reader.read_ue()?;

                // Skip long-term ref pic info for now
                let total_lt = header.num_long_term_sps + header.num_long_term_pics;
                for i in 0..total_lt {
                    if i < header.num_long_term_sps {
                        if sps.num_long_term_ref_pics_sps > 1 {
                            let bits = (32 - sps.num_long_term_ref_pics_sps.leading_zeros()).max(1);
                            // lt_idx_sps
                            let _: u32 = reader.read_var(bits)?;
                        }
                    } else {
                        let poc_bits = sps.log2_max_pic_order_cnt_lsb as u32;
                        // poc_lsb_lt
                        let _: u32 = reader.read_var(poc_bits)?;
                        // used_by_curr_pic_lt_flag
                        let _ = reader.read_bit()?;
                    }
                    // delta_poc_msb_present_flag
                    // Note: delta_poc_msb_cycle_lt parsing omitted for simplicity
                    let _ = reader.read_bit()?;
                }
            }

            // Read slice_temporal_mvp_enabled_flag
            if sps.sps_temporal_mvp_enabled_flag {
                header.slice_temporal_mvp_enabled_flag = reader.read_bit()?;
            }
        }

        // Read slice_sao_luma_flag and slice_sao_chroma_flag
        if sps.sample_adaptive_offset_enabled_flag {
            header.slice_sao_luma_flag = reader.read_bit()?;
            let chroma_array_type = if sps.separate_colour_plane_flag {
                0
            } else {
                sps.chroma_format as u8
            };
            if chroma_array_type != 0 {
                header.slice_sao_chroma_flag = reader.read_bit()?;
            }
        }

        // Reference picture list related
        if header.slice_type.is_inter() {
            // Read num_ref_idx_active_override_flag
            let num_ref_idx_active_override_flag = reader.read_bit()?;
            if num_ref_idx_active_override_flag {
                header.num_ref_idx_l0_active = reader.read_ue()? + 1;
                if header.slice_type.is_b() {
                    header.num_ref_idx_l1_active = reader.read_ue()? + 1;
                }
            } else {
                header.num_ref_idx_l0_active = pps.num_ref_idx_l0_default_active;
                header.num_ref_idx_l1_active = pps.num_ref_idx_l1_default_active;
            }

            // Read ref_pic_lists_modification
            if pps.lists_modification_present_flag {
                header.ref_pic_list_modification = RefPicListModification::parse(reader, &header)?;
            }

            // Read mvd_l1_zero_flag
            if header.slice_type.is_b() {
                header.mvd_l1_zero_flag = reader.read_bit()?;
            }

            // Read cabac_init_flag
            if pps.cabac_init_present_flag {
                header.cabac_init_flag = reader.read_bit()?;
            }

            // Read collocated_from_l0_flag and collocated_ref_idx
            if header.slice_temporal_mvp_enabled_flag {
                if header.slice_type.is_b() {
                    header.collocated_from_l0_flag = reader.read_bit()?;
                } else {
                    header.collocated_from_l0_flag = true;
                }

                let max_ref_idx = if header.collocated_from_l0_flag {
                    header.num_ref_idx_l0_active
                } else {
                    header.num_ref_idx_l1_active
                };

                if max_ref_idx > 1 {
                    header.collocated_ref_idx = reader.read_ue()?;
                }
            }

            // Read pred_weight_table
            if (pps.weighted_pred_flag && header.slice_type.is_p()) || (pps.weighted_bipred_flag && header.slice_type.is_b()) {
                header.pred_weight_table = Some(PredWeightTable::parse(reader, &header, sps)?);
            }

            // Read max_num_merge_cand
            let five_minus_max_num_merge_cand = reader.read_ue()?;
            header.max_num_merge_cand = 5 - five_minus_max_num_merge_cand;

            // use_integer_mv_flag (SCC extension) - skip for now
        }

        // Read slice_qp_delta
        header.slice_qp_delta = reader.read_se()?;

        // Read slice_cb_qp_offset and slice_cr_qp_offset
        if pps.pps_slice_chroma_qp_offsets_present_flag {
            header.slice_cb_qp_offset = reader.read_se()?;
            header.slice_cr_qp_offset = reader.read_se()?;
        }

        // cu_chroma_qp_offset_enabled_flag (range extension) - skip for now

        // Read deblocking_filter_override_flag
        if pps.deblocking_filter_control_present_flag {
            if let Some(ref dbf_params) = pps.deblocking_filter_params {
                if dbf_params.deblocking_filter_override_enabled_flag {
                    header.deblocking_filter_override_flag = reader.read_bit()?;
                }
            }

            if header.deblocking_filter_override_flag {
                header.slice_deblocking_filter_disabled_flag = reader.read_bit()?;
                if !header.slice_deblocking_filter_disabled_flag {
                    header.slice_beta_offset = reader.read_se()? * 2;
                    header.slice_tc_offset = reader.read_se()? * 2;
                }
            } else if let Some(ref dbf_params) = pps.deblocking_filter_params {
                header.slice_deblocking_filter_disabled_flag = dbf_params.pps_deblocking_filter_disabled_flag;
                header.slice_beta_offset = dbf_params.pps_beta_offset;
                header.slice_tc_offset = dbf_params.pps_tc_offset;
            }
        }

        // Read slice_loop_filter_across_slices_enabled_flag
        let has_tiles_or_entropy_sync = pps.tiles_enabled_flag || pps.entropy_coding_sync_enabled_flag;
        if has_tiles_or_entropy_sync {
            header.slice_loop_filter_across_slices_enabled_flag = reader.read_bit()?;
        } else {
            header.slice_loop_filter_across_slices_enabled_flag = pps.pps_loop_filter_across_slices_enabled_flag;
        }

        // Entry points
        if pps.tiles_enabled_flag || pps.entropy_coding_sync_enabled_flag {
            header.num_entry_point_offsets = reader.read_ue()?;
            if header.num_entry_point_offsets > 0 {
                let offset_len = reader.read_ue()? + 1;
                let offset_bits = offset_len;
                header.entry_point_offset = Vec::with_capacity(header.num_entry_point_offsets as usize);
                for _ in 0..header.num_entry_point_offsets {
                    header.entry_point_offset.push(reader.read_var::<u32>(offset_bits)? + 1);
                }
            }
        }

        Ok(header)
    }

    /// Get number of reference pictures in list 0
    #[inline]
    pub fn num_ref_idx_l0_active(&self) -> u32 {
        self.num_ref_idx_l0_active
    }

    /// Get number of reference pictures in list 1
    #[inline]
    pub fn num_ref_idx_l1_active(&self) -> u32 {
        self.num_ref_idx_l1_active
    }

    /// Get slice QP
    #[inline]
    pub fn slice_qp(&self, pps: &Pps) -> i32 {
        pps.init_qp + self.slice_qp_delta
    }

    /// Get max number of merge candidates
    #[inline]
    pub fn max_num_merge_cand(&self) -> u32 {
        self.max_num_merge_cand
    }

    /// Check if this is the first slice segment in the picture
    #[inline]
    pub fn is_first_slice(&self) -> bool {
        self.first_slice_segment_in_pic_flag
    }

    /// Check if this is a dependent slice segment
    #[inline]
    pub fn is_dependent(&self) -> bool {
        self.dependent_slice_segment_flag
    }

    /// Get slice beta offset (actual value)
    #[inline]
    pub fn beta_offset(&self) -> i32 {
        self.slice_beta_offset
    }

    /// Get slice tc offset (actual value)
    #[inline]
    pub fn tc_offset(&self) -> i32 {
        self.slice_tc_offset
    }
}
