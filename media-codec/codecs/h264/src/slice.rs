//! H.264/AVC Slice Header parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, not_found_error, Result};
use smallvec::SmallVec;

use crate::{
    constants::{MAX_PPS_COUNT, MAX_REFS},
    nal::NalUnitType,
    pps::Pps,
    ps::ParameterSets,
};

/// Slice type as defined in ITU-T H.264 Table 7-6
///
/// Values 0-4 are the base types, values 5-9 indicate that all slices
/// in the picture are of the same type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum SliceType {
    /// P slice (predictive)
    P  = 0,
    /// B slice (bi-predictive)
    B  = 1,
    /// I slice (intra)
    #[default]
    I  = 2,
    /// SP slice (switching P)
    SP = 3,
    /// SI slice (switching I)
    SI = 4,
}

impl SliceType {
    /// Create from raw slice type value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value % 5 {
            0 => Some(Self::P),
            1 => Some(Self::B),
            2 => Some(Self::I),
            3 => Some(Self::SP),
            4 => Some(Self::SI),
            _ => None,
        }
    }

    /// Check if the slice type indicates all slices in picture have same type
    #[inline]
    pub fn is_all_same_type(value: u32) -> bool {
        value >= 5
    }

    /// Check if this is an intra slice (I or SI)
    #[inline]
    pub fn is_intra(&self) -> bool {
        matches!(self, Self::I | Self::SI)
    }

    /// Check if this is an inter slice (P, B, or SP)
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

    /// Check if this is an SP slice
    #[inline]
    pub fn is_sp(&self) -> bool {
        matches!(self, Self::SP)
    }

    /// Check if this is an SI slice
    #[inline]
    pub fn is_si(&self) -> bool {
        matches!(self, Self::SI)
    }

    /// Check if this is a switching slice (SP or SI)
    #[inline]
    pub fn is_switching(&self) -> bool {
        matches!(self, Self::SP | Self::SI)
    }
}

/// Picture structure
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PictureStructure {
    #[default]
    Frame,
    TopField,
    BottomField,
}

/// Reference picture list modification operation
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefPicListModOp {
    /// Subtract from picture number (modification_of_pic_nums_idc == 0)
    SubtractFromPicNum(u32),
    /// Add to picture number (modification_of_pic_nums_idc == 1)
    AddToPicNum(u32),
    /// Long-term picture index (modification_of_pic_nums_idc == 2)
    LongTermPicNum(u32),
    /// End of modification (modification_of_pic_nums_idc == 3)
    End,
}

/// Reference picture list modification
///
/// Note: SmallVec's inline capacity must be a 2 ^ N. Since up to `MAX_REFS + 1`
/// entries are needed, `MAX_REFS << 1` is used as the next power of 2.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RefPicListModification {
    /// Modification operations for list 0
    pub modification_l0: SmallVec<[RefPicListModOp; MAX_REFS << 1]>,
    /// Modification operations for list 1
    pub modification_l1: SmallVec<[RefPicListModOp; MAX_REFS << 1]>,
}

impl RefPicListModification {
    /// Parse reference picture list modification from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, slice_type: SliceType) -> Result<Self> {
        let mut rplm = Self::default();

        // ref_pic_list_modification for list 0 (P, B, SP slices)
        if !slice_type.is_intra() {
            let ref_pic_list_modification_flag_l0 = reader.read_bit()?;
            if ref_pic_list_modification_flag_l0 {
                rplm.modification_l0.reserve(MAX_REFS);
                loop {
                    let modification_of_pic_nums_idc = reader.read_ue()?;
                    let op = match modification_of_pic_nums_idc {
                        0 => {
                            let abs_diff_pic_num = reader.read_ue()? + 1;
                            RefPicListModOp::SubtractFromPicNum(abs_diff_pic_num)
                        }
                        1 => {
                            let abs_diff_pic_num = reader.read_ue()? + 1;
                            RefPicListModOp::AddToPicNum(abs_diff_pic_num)
                        }
                        2 => {
                            let long_term_pic_num = reader.read_ue()?;
                            RefPicListModOp::LongTermPicNum(long_term_pic_num)
                        }
                        3 => RefPicListModOp::End,
                        _ => return Err(invalid_data_error!("modification_of_pic_nums_idc", modification_of_pic_nums_idc)),
                    };
                    rplm.modification_l0.push(op);
                    if matches!(op, RefPicListModOp::End) {
                        break;
                    }
                }
            }
        }

        // ref_pic_list_modification for list 1 (B slices only)
        if slice_type.is_b() {
            let ref_pic_list_modification_flag_l1 = reader.read_bit()?;
            if ref_pic_list_modification_flag_l1 {
                rplm.modification_l1.reserve(MAX_REFS);
                loop {
                    let modification_of_pic_nums_idc = reader.read_ue()?;
                    let op = match modification_of_pic_nums_idc {
                        0 => {
                            let abs_diff_pic_num = reader.read_ue()? + 1;
                            RefPicListModOp::SubtractFromPicNum(abs_diff_pic_num)
                        }
                        1 => {
                            let abs_diff_pic_num = reader.read_ue()? + 1;
                            RefPicListModOp::AddToPicNum(abs_diff_pic_num)
                        }
                        2 => {
                            let long_term_pic_num = reader.read_ue()?;
                            RefPicListModOp::LongTermPicNum(long_term_pic_num)
                        }
                        3 => RefPicListModOp::End,
                        _ => return Err(invalid_data_error!("modification_of_pic_nums_idc", modification_of_pic_nums_idc)),
                    };
                    rplm.modification_l1.push(op);
                    if matches!(op, RefPicListModOp::End) {
                        break;
                    }
                }
            }
        }

        Ok(rplm)
    }
}

/// Prediction weight table
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PredWeightTable {
    /// Luma log2 weight denom
    pub luma_log2_weight_denom: u32,
    /// Chroma log2 weight denom
    pub chroma_log2_weight_denom: u32,
    /// Luma weight L0
    pub luma_weight_l0: SmallVec<[i32; MAX_REFS]>,
    /// Luma offset L0
    pub luma_offset_l0: SmallVec<[i32; MAX_REFS]>,
    /// Chroma weight L0 [Cb, Cr]
    pub chroma_weight_l0: SmallVec<[[i32; 2]; MAX_REFS]>,
    /// Chroma offset L0 [Cb, Cr]
    pub chroma_offset_l0: SmallVec<[[i32; 2]; MAX_REFS]>,
    /// Luma weight L1
    pub luma_weight_l1: SmallVec<[i32; MAX_REFS]>,
    /// Luma offset L1
    pub luma_offset_l1: SmallVec<[i32; MAX_REFS]>,
    /// Chroma weight L1 [Cb, Cr]
    pub chroma_weight_l1: SmallVec<[[i32; 2]; MAX_REFS]>,
    /// Chroma offset L1 [Cb, Cr]
    pub chroma_offset_l1: SmallVec<[[i32; 2]; MAX_REFS]>,
}

impl PredWeightTable {
    /// Parse prediction weight table from a BitReader
    pub fn parse<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        slice_type: SliceType,
        num_ref_idx_l0_active: u32,
        num_ref_idx_l1_active: u32,
        chroma_array_type: u8,
    ) -> Result<Self> {
        let mut pwt = Self {
            // Read luma_log2_weight_denom
            luma_log2_weight_denom: reader.read_ue()?,
            ..Default::default()
        };

        // Read chroma_log2_weight_denom
        if chroma_array_type != 0 {
            pwt.chroma_log2_weight_denom = reader.read_ue()?;
        }

        let default_luma_weight = 1i32 << pwt.luma_log2_weight_denom;
        let default_chroma_weight = 1i32 << pwt.chroma_log2_weight_denom;

        let num_l0 = num_ref_idx_l0_active as usize;

        // Initialize L0 vectors
        pwt.luma_weight_l0 = SmallVec::with_capacity(num_l0);
        pwt.luma_offset_l0 = SmallVec::with_capacity(num_l0);
        pwt.chroma_weight_l0 = SmallVec::with_capacity(num_l0);
        pwt.chroma_offset_l0 = SmallVec::with_capacity(num_l0);

        // L0 weights
        for _ in 0..num_l0 {
            let luma_weight_l0_flag = reader.read_bit()?;
            if luma_weight_l0_flag {
                pwt.luma_weight_l0.push(reader.read_se()?);
                pwt.luma_offset_l0.push(reader.read_se()?);
            } else {
                pwt.luma_weight_l0.push(default_luma_weight);
                pwt.luma_offset_l0.push(0);
            }

            if chroma_array_type != 0 {
                let chroma_weight_l0_flag = reader.read_bit()?;
                if chroma_weight_l0_flag {
                    let w0 = reader.read_se()?;
                    let o0 = reader.read_se()?;
                    let w1 = reader.read_se()?;
                    let o1 = reader.read_se()?;
                    pwt.chroma_weight_l0.push([w0, w1]);
                    pwt.chroma_offset_l0.push([o0, o1]);
                } else {
                    pwt.chroma_weight_l0.push([default_chroma_weight, default_chroma_weight]);
                    pwt.chroma_offset_l0.push([0, 0]);
                }
            }
        }

        // L1 weights (for B slices)
        if slice_type.is_b() {
            let num_l1 = num_ref_idx_l1_active as usize;

            pwt.luma_weight_l1 = SmallVec::with_capacity(num_l1);
            pwt.luma_offset_l1 = SmallVec::with_capacity(num_l1);
            pwt.chroma_weight_l1 = SmallVec::with_capacity(num_l1);
            pwt.chroma_offset_l1 = SmallVec::with_capacity(num_l1);

            for _ in 0..num_l1 {
                let luma_weight_l1_flag = reader.read_bit()?;
                if luma_weight_l1_flag {
                    pwt.luma_weight_l1.push(reader.read_se()?);
                    pwt.luma_offset_l1.push(reader.read_se()?);
                } else {
                    pwt.luma_weight_l1.push(default_luma_weight);
                    pwt.luma_offset_l1.push(0);
                }

                if chroma_array_type != 0 {
                    let chroma_weight_l1_flag = reader.read_bit()?;
                    if chroma_weight_l1_flag {
                        let w0 = reader.read_se()?;
                        let o0 = reader.read_se()?;
                        let w1 = reader.read_se()?;
                        let o1 = reader.read_se()?;
                        pwt.chroma_weight_l1.push([w0, w1]);
                        pwt.chroma_offset_l1.push([o0, o1]);
                    } else {
                        pwt.chroma_weight_l1.push([default_chroma_weight, default_chroma_weight]);
                        pwt.chroma_offset_l1.push([0, 0]);
                    }
                }
            }
        }

        Ok(pwt)
    }
}

/// Memory management control operation
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryManagementControlOp {
    /// End of operations (memory_management_control_operation == 0)
    End,
    /// Mark short-term picture as "unused for reference" (op == 1)
    ShortTermUnused { difference_of_pic_nums: u32 },
    /// Mark long-term picture as "unused for reference" (op == 2)
    LongTermUnused { long_term_pic_num: u32 },
    /// Assign long-term frame index to short-term picture (op == 3)
    ShortTermToLongTerm { difference_of_pic_nums: u32, long_term_frame_idx: u32 },
    /// Specify max long-term frame index (op == 4)
    MaxLongTermFrameIdx { max_long_term_frame_idx: i32 },
    /// Mark all reference pictures as "unused for reference" (op == 5)
    ClearAll,
    /// Assign long-term frame index to current picture (op == 6)
    CurrentToLongTerm { long_term_frame_idx: u32 },
}

/// Decoded reference picture marking
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DecRefPicMarking {
    /// No output of prior pictures flag (for IDR)
    pub no_output_of_prior_pics_flag: bool,
    /// Long-term reference flag (for IDR)
    pub long_term_reference_flag: bool,
    /// Adaptive reference picture marking mode flag (for non-IDR)
    pub adaptive_ref_pic_marking_mode_flag: bool,
    /// Memory management control operations
    /// Note: SmallVec's inline capacity must be a 2 ^ N. Since up to `MAX_REFS
    /// << 1 + 3` entries are needed, `MAX_REFS << 2` is used as the next
    /// power of 2.
    pub memory_management_control_operations: SmallVec<[MemoryManagementControlOp; MAX_REFS << 2]>,
}

impl DecRefPicMarking {
    /// Parse decoded reference picture marking from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, is_idr: bool) -> Result<Self> {
        let mut drpm = Self::default();

        if is_idr {
            drpm.no_output_of_prior_pics_flag = reader.read_bit()?;
            drpm.long_term_reference_flag = reader.read_bit()?;
        } else {
            drpm.adaptive_ref_pic_marking_mode_flag = reader.read_bit()?;
            if drpm.adaptive_ref_pic_marking_mode_flag {
                loop {
                    let memory_management_control_operation = reader.read_ue()?;
                    let op = match memory_management_control_operation {
                        0 => MemoryManagementControlOp::End,
                        1 => {
                            let difference_of_pic_nums = reader.read_ue()? + 1;
                            MemoryManagementControlOp::ShortTermUnused {
                                difference_of_pic_nums,
                            }
                        }
                        2 => {
                            let long_term_pic_num = reader.read_ue()?;
                            MemoryManagementControlOp::LongTermUnused {
                                long_term_pic_num,
                            }
                        }
                        3 => {
                            let difference_of_pic_nums = reader.read_ue()? + 1;
                            let long_term_frame_idx = reader.read_ue()?;
                            MemoryManagementControlOp::ShortTermToLongTerm {
                                difference_of_pic_nums,
                                long_term_frame_idx,
                            }
                        }
                        4 => {
                            let max_long_term_frame_idx = reader.read_ue()? as i32 - 1;
                            MemoryManagementControlOp::MaxLongTermFrameIdx {
                                max_long_term_frame_idx,
                            }
                        }
                        5 => MemoryManagementControlOp::ClearAll,
                        6 => {
                            let long_term_frame_idx = reader.read_ue()?;
                            MemoryManagementControlOp::CurrentToLongTerm {
                                long_term_frame_idx,
                            }
                        }
                        _ => return Err(invalid_data_error!("memory_management_control_operation", memory_management_control_operation)),
                    };
                    drpm.memory_management_control_operations.push(op);
                    if matches!(op, MemoryManagementControlOp::End) {
                        break;
                    }
                }
            }
        }

        Ok(drpm)
    }
}

/// Slice Header
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SliceHeader {
    /// First macroblock in slice
    pub first_mb_in_slice: u32,
    /// Slice type
    pub slice_type: SliceType,
    /// PPS ID (0-255)
    pub pic_parameter_set_id: u8,
    /// Colour plane ID (for separate colour plane, 0-2)
    pub colour_plane_id: u8,
    /// Frame number
    pub frame_num: u32,
    /// Field picture flag
    pub field_pic_flag: bool,
    /// Bottom field flag
    pub bottom_field_flag: bool,
    /// Picture
    pub picture_structure: PictureStructure,
    /// IDR picture ID (for IDR slices)
    pub idr_pic_id: u32,
    /// Picture order count LSB (for pic_order_cnt_type == 0)
    pub pic_order_cnt_lsb: u32,
    /// Delta picture order count bottom (for pic_order_cnt_type == 0 and bottom
    /// field)
    pub delta_pic_order_cnt_bottom: i32,
    /// Delta picture order count [0] (for pic_order_cnt_type == 1)
    pub delta_pic_order_cnt_0: i32,
    /// Delta picture order count [1] (for pic_order_cnt_type == 1)
    pub delta_pic_order_cnt_1: i32,
    /// Redundant picture count
    pub redundant_pic_cnt: u32,
    /// Direct spatial MV prediction flag (for B slices)
    pub direct_spatial_mv_pred_flag: bool,
    /// Number of reference pictures in list 0 override flag
    pub num_ref_idx_active_override_flag: bool,
    /// Number of reference pictures in list 0
    pub num_ref_idx_l0_active: u32,
    /// Number of reference pictures in list 1
    pub num_ref_idx_l1_active: u32,
    /// Reference picture list modification
    pub ref_pic_list_modification: Option<RefPicListModification>,
    /// Prediction weight table
    pub pred_weight_table: Option<PredWeightTable>,
    /// Decoded reference picture marking
    pub dec_ref_pic_marking: Option<DecRefPicMarking>,
    /// CABAC init IDC (0-2, for CABAC slices)
    pub cabac_init_idc: u32,
    /// Slice QP delta
    pub slice_qp_delta: i32,
    /// SP for switch flag (for SP/SI slices)
    pub sp_for_switch_flag: bool,
    /// Slice QS delta (for SP/SI slices)
    pub slice_qs_delta: i32,
    /// Slice QS
    pub slice_qs: u32,
    /// Chroma QP for Cb
    pub chroma_qp_cb: u8,
    /// Chroma QP for Cr
    pub chroma_qp_cr: u8,
    /// Disable deblocking filter IDC (0-2)
    pub disable_deblocking_filter_idc: u32,
    /// Slice alpha C0 offset
    pub slice_alpha_c0_offset: i32,
    /// Slice beta offset
    pub slice_beta_offset: i32,
    /// Slice group change cycle (for slice groups)
    pub slice_group_change_cycle: u32,
}

impl SliceHeader {
    /// Parse slice header from raw NAL unit RBSP data
    pub fn parse(data: &[u8], nal_unit_type: NalUnitType, nal_ref_idc: u8, param_sets: &ParameterSets) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, nal_unit_type, nal_ref_idc, param_sets)
    }

    /// Parse slice header from a BitReader
    pub fn parse_from_bit_reader<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        nal_unit_type: NalUnitType,
        nal_ref_idc: u8,
        param_sets: &ParameterSets,
    ) -> Result<Self> {
        let mut header = Self::default();

        let is_idr = matches!(nal_unit_type, NalUnitType::SliceIdr);

        // Read first_mb_in_slice
        header.first_mb_in_slice = reader.read_ue()?;

        // Read slice_type
        let slice_type_raw = reader.read_ue()? as u8;
        header.slice_type = SliceType::from_u8(slice_type_raw).ok_or_else(|| invalid_data_error!("slice_type", slice_type_raw))?;

        // Read pic_parameter_set_id
        let pic_parameter_set_id = reader.read_ue()?;
        if pic_parameter_set_id as usize >= MAX_PPS_COUNT {
            return Err(invalid_data_error!("pps_id", pic_parameter_set_id));
        }
        header.pic_parameter_set_id = pic_parameter_set_id as u8;

        let pps = param_sets.get_pps(pic_parameter_set_id).ok_or_else(|| not_found_error!("pps_id", pic_parameter_set_id))?;
        let sps = param_sets.get_sps(pps.seq_parameter_set_id as u32).ok_or_else(|| not_found_error!("sps_id", pps.seq_parameter_set_id))?;

        // Read colour_plane_id (for separate colour plane)
        if sps.separate_colour_plane_flag {
            header.colour_plane_id = reader.read::<2, u8>()?;
        }

        // Read frame_num
        let frame_num_bits = sps.log2_max_frame_num;
        header.frame_num = reader.read_var(frame_num_bits)?;

        // Read field_pic_flag, bottom_field_flag and set picture_structure
        header.picture_structure = if !sps.frame_mbs_only_flag && reader.read_bit()? {
            header.field_pic_flag = true;
            if reader.read_bit()? {
                header.bottom_field_flag = true;
                PictureStructure::BottomField
            } else {
                PictureStructure::TopField
            }
        } else {
            PictureStructure::Frame
        };

        // Read idr_pic_id (for IDR slices)
        if is_idr {
            header.idr_pic_id = reader.read_ue()?;
        }

        // Read Picture order count
        if sps.pic_order_cnt_type == 0 {
            let poc_lsb_bits = sps.log2_max_pic_order_cnt_lsb;
            header.pic_order_cnt_lsb = reader.read_var(poc_lsb_bits)?;

            if pps.bottom_field_pic_order_in_frame_present_flag && !header.field_pic_flag {
                header.delta_pic_order_cnt_bottom = reader.read_se()?;
            }
        }

        if sps.pic_order_cnt_type == 1 && !sps.delta_pic_order_always_zero_flag {
            header.delta_pic_order_cnt_0 = reader.read_se()?;
            if pps.bottom_field_pic_order_in_frame_present_flag && !header.field_pic_flag {
                header.delta_pic_order_cnt_1 = reader.read_se()?;
            }
        }

        // Read redundant_pic_cnt
        if pps.redundant_pic_cnt_present_flag {
            header.redundant_pic_cnt = reader.read_ue()?;
        }

        // Read direct_spatial_mv_pred_flag (for B slices)
        if header.slice_type.is_b() {
            header.direct_spatial_mv_pred_flag = reader.read_bit()?;
        }

        // Read num_ref_idx_active_override_flag and ref_idx counts
        if header.slice_type.is_p() || header.slice_type.is_sp() || header.slice_type.is_b() {
            header.num_ref_idx_active_override_flag = reader.read_bit()?;
            if header.num_ref_idx_active_override_flag {
                let num_ref_idx_l0_active_minus1 = reader.read_ue()?;
                if num_ref_idx_l0_active_minus1 as usize > MAX_REFS {
                    return Err(invalid_data_error!("num_ref_idx_l0_active_minus1", num_ref_idx_l0_active_minus1));
                }
                header.num_ref_idx_l0_active = num_ref_idx_l0_active_minus1 + 1;
                if header.slice_type.is_b() {
                    let num_ref_idx_l1_active_minus1 = reader.read_ue()?;
                    if num_ref_idx_l1_active_minus1 as usize > MAX_REFS {
                        return Err(invalid_data_error!("num_ref_idx_l1_active_minus1", num_ref_idx_l1_active_minus1));
                    }
                    header.num_ref_idx_l1_active = num_ref_idx_l1_active_minus1 + 1;
                }
            } else {
                header.num_ref_idx_l0_active = pps.num_ref_idx_l0_default_active;
                header.num_ref_idx_l1_active = pps.num_ref_idx_l1_default_active;
            }
        }

        // Read ref_pic_list_modification
        if !header.slice_type.is_intra() {
            header.ref_pic_list_modification = Some(RefPicListModification::parse(reader, header.slice_type)?);
        }

        // Read pred_weight_table
        let chroma_array_type = if sps.separate_colour_plane_flag {
            0
        } else {
            sps.chroma_format as u8
        };

        if (pps.weighted_pred_flag && (header.slice_type.is_p() || header.slice_type.is_sp())) ||
            (pps.weighted_bipred_idc == 1 && header.slice_type.is_b())
        {
            header.pred_weight_table = Some(PredWeightTable::parse(
                reader,
                header.slice_type,
                header.num_ref_idx_l0_active,
                header.num_ref_idx_l1_active,
                chroma_array_type,
            )?);
        }

        // Read dec_ref_pic_marking
        if nal_ref_idc != 0 {
            header.dec_ref_pic_marking = Some(DecRefPicMarking::parse(reader, is_idr)?);
        }

        // Read cabac_init_idc
        if pps.entropy_coding_mode_flag && !header.slice_type.is_intra() {
            header.cabac_init_idc = reader.read_ue()?;
            if header.cabac_init_idc > 2 {
                return Err(invalid_data_error!("cabac_init_idc", header.cabac_init_idc));
            }
        }

        // Read slice_qp_delta and validate slice QP
        header.slice_qp_delta = reader.read_se()?;
        let qp_offset = 6 * (sps.bit_depth_luma as i32 - 8);
        let slice_qp = header.slice_qp(pps);
        if slice_qp < 0 || slice_qp > 51 + qp_offset {
            return Err(invalid_data_error!("slice_qp_delta", header.slice_qp_delta));
        }

        // SP/SI specific parameters
        if header.slice_type.is_sp() || header.slice_type.is_si() {
            if header.slice_type.is_sp() {
                header.sp_for_switch_flag = reader.read_bit()?;
            }
            header.slice_qs_delta = reader.read_se()?;
            let slice_qs = header.slice_qs(pps);
            if !(0..=51).contains(&slice_qs) {
                return Err(invalid_data_error!("slice_qs_delta", header.slice_qs_delta));
            }

            header.slice_qs = slice_qs as u32;
        }

        header.chroma_qp_cb = pps.chroma_qp_tables.get_cb(slice_qp);
        header.chroma_qp_cr = pps.chroma_qp_tables.get_cr(slice_qp);

        // Deblocking filter control
        if pps.deblocking_filter_control_present_flag {
            header.disable_deblocking_filter_idc = reader.read_ue()?;
            if header.disable_deblocking_filter_idc > 2 {
                return Err(invalid_data_error!("disable_deblocking_filter_idc", header.disable_deblocking_filter_idc));
            }
            if header.disable_deblocking_filter_idc != 1 {
                let slice_alpha_c0_offset_div2: i32 = reader.read_se()?;
                let slice_beta_offset_div2: i32 = reader.read_se()?;

                if !(-6..=6).contains(&slice_alpha_c0_offset_div2) {
                    return Err(invalid_data_error!("slice_alpha_c0_offset_div2", slice_alpha_c0_offset_div2));
                }

                if !(-6..=6).contains(&slice_beta_offset_div2) {
                    return Err(invalid_data_error!("slice_beta_offset_div2", slice_beta_offset_div2));
                }

                header.slice_alpha_c0_offset = slice_alpha_c0_offset_div2 * 2;
                header.slice_beta_offset = slice_beta_offset_div2 * 2;
            }
        }

        // Read slice_group_change_cycle
        if pps.num_slice_groups > 1 {
            if let Some(ref sg_params) = pps.slice_group_params {
                let map_type = sg_params.slice_group_map_type as u8;
                if (3..=5).contains(&map_type) {
                    let pic_size_in_map_units = sps.pic_width_in_mbs * sps.pic_height_in_map_units;
                    let slice_group_change_rate = sg_params.slice_group_change_rate;
                    let bits_needed = (32 - ((pic_size_in_map_units - 1) / slice_group_change_rate + 1).leading_zeros()).max(1);
                    header.slice_group_change_cycle = reader.read_var(bits_needed)?;
                }
            }
        }

        Ok(header)
    }

    /// Get slice QP
    #[inline]
    pub fn slice_qp(&self, pps: &Pps) -> i32 {
        pps.pic_init_qp + self.slice_qp_delta
    }

    /// Get slice QS
    #[inline]
    pub fn slice_qs(&self, pps: &Pps) -> i32 {
        pps.pic_init_qs + self.slice_qs_delta
    }

    /// Check if this is the first macroblock in the picture
    #[inline]
    pub fn is_first_mb_in_pic(&self) -> bool {
        self.first_mb_in_slice == 0
    }

    /// Check if this is a field picture
    #[inline]
    pub fn is_field_pic(&self) -> bool {
        self.field_pic_flag
    }

    /// Check if this is a bottom field
    #[inline]
    pub fn is_bottom_field(&self) -> bool {
        self.field_pic_flag && self.bottom_field_flag
    }

    /// Check if this is a top field
    #[inline]
    pub fn is_top_field(&self) -> bool {
        self.field_pic_flag && !self.bottom_field_flag
    }

    /// Check if this is a frame (not field) picture
    #[inline]
    pub fn is_frame(&self) -> bool {
        !self.field_pic_flag
    }

    /// Get slice alpha offset
    #[inline]
    pub fn alpha_c0_offset(&self) -> i32 {
        self.slice_alpha_c0_offset
    }

    /// Get slice beta offset
    #[inline]
    pub fn beta_offset(&self) -> i32 {
        self.slice_beta_offset
    }

    /// Check if deblocking filter is disabled for this slice
    #[inline]
    pub fn is_deblocking_filter_disabled(&self) -> bool {
        self.disable_deblocking_filter_idc == 1
    }

    /// Check if deblocking filter crosses slice boundaries
    #[inline]
    pub fn deblocking_filter_crosses_slices(&self) -> bool {
        self.disable_deblocking_filter_idc != 2
    }
}
