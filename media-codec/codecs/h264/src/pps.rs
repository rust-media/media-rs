//! H.264/AVC Picture Parameter Set (PPS) parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, Result};
use smallvec::SmallVec;

use crate::scaling_list::{ScalingList4x4, ScalingList8x8};

/// Maximum number of slice groups
const MAX_SLICE_GROUPS: usize = 8;

/// Slice group map type
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum SliceGroupMapType {
    /// Interleaved slice groups
    #[default]
    Interleaved        = 0,
    /// Dispersed slice group mapping
    Dispersed          = 1,
    /// Foreground with left-over slice group
    ForegroundLeftover = 2,
    /// Changing slice groups (box-out)
    BoxOut             = 3,
    /// Changing slice groups (raster scan)
    RasterScan         = 4,
    /// Changing slice groups (wipe)
    Wipe               = 5,
    /// Explicit slice group map
    Explicit           = 6,
}

impl From<u32> for SliceGroupMapType {
    fn from(value: u32) -> Self {
        match value {
            0 => SliceGroupMapType::Interleaved,
            1 => SliceGroupMapType::Dispersed,
            2 => SliceGroupMapType::ForegroundLeftover,
            3 => SliceGroupMapType::BoxOut,
            4 => SliceGroupMapType::RasterScan,
            5 => SliceGroupMapType::Wipe,
            6 => SliceGroupMapType::Explicit,
            _ => SliceGroupMapType::Interleaved,
        }
    }
}

/// Slice group parameters
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SliceGroupParams {
    /// Slice group map type
    pub slice_group_map_type: SliceGroupMapType,
    /// Run length for each slice group (for type 0)
    pub run_length: SmallVec<[u32; MAX_SLICE_GROUPS]>,
    /// Top left macroblock address for each slice group (for type 2)
    pub top_left: SmallVec<[u32; MAX_SLICE_GROUPS]>,
    /// Bottom right macroblock address for each slice group (for type 2)
    pub bottom_right: SmallVec<[u32; MAX_SLICE_GROUPS]>,
    /// Slice group change direction flag (for types 3, 4, 5)
    pub slice_group_change_direction_flag: bool,
    /// Slice group change rate (for types 3, 4, 5)
    pub slice_group_change_rate: u32,
    /// Picture size in map units (for type 6)
    pub pic_size_in_map_units: u32,
    /// Slice group ID for each map unit (for type 6)
    pub slice_group_id: Vec<u32>,
}

impl SliceGroupParams {
    /// Parse slice group parameters from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, num_slice_groups: u32) -> Result<Self> {
        let slice_group_map_type_val = reader.read_ue()?;
        let slice_group_map_type = SliceGroupMapType::from(slice_group_map_type_val);

        let mut params = Self {
            slice_group_map_type,
            ..Default::default()
        };

        match slice_group_map_type {
            SliceGroupMapType::Interleaved => {
                // Type 0: Interleaved slice groups
                let num_groups = num_slice_groups as usize;
                params.run_length.reserve(num_groups);
                for _ in 0..num_groups {
                    params.run_length.push(reader.read_ue()? + 1);
                }
            }
            SliceGroupMapType::Dispersed => {
                // Type 1: No additional parameters
            }
            SliceGroupMapType::ForegroundLeftover => {
                // Type 2: Foreground with left-over
                let num_groups = (num_slice_groups - 1) as usize;
                params.top_left.reserve(num_groups);
                params.bottom_right.reserve(num_groups);
                for _ in 0..num_groups {
                    params.top_left.push(reader.read_ue()?);
                    params.bottom_right.push(reader.read_ue()?);
                }
            }
            SliceGroupMapType::BoxOut | SliceGroupMapType::RasterScan | SliceGroupMapType::Wipe => {
                // Types 3, 4, 5: Changing slice groups
                params.slice_group_change_direction_flag = reader.read_bit()?;
                params.slice_group_change_rate = reader.read_ue()? + 1;
            }
            SliceGroupMapType::Explicit => {
                // Type 6: Explicit slice group map
                params.pic_size_in_map_units = reader.read_ue()? + 1;
                let num_map_units = params.pic_size_in_map_units as usize;
                // Calculate bits needed for slice_group_id
                let bits_needed = (32 - (num_slice_groups - 1).leading_zeros()).max(1);
                params.slice_group_id = Vec::with_capacity(num_map_units);
                for _ in 0..num_map_units {
                    params.slice_group_id.push(reader.read_var(bits_needed)?);
                }
            }
        }

        Ok(params)
    }
}

/// Picture Parameter Set (PPS)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Pps {
    /// Picture parameter set ID (0-255)
    pub pic_parameter_set_id: u8,
    /// Sequence parameter set ID this PPS refers to (0-31)
    pub seq_parameter_set_id: u8,
    /// Entropy coding mode flag (0=CAVLC, 1=CABAC)
    pub entropy_coding_mode_flag: bool,
    /// Bottom field picture order in frame present flag
    pub bottom_field_pic_order_in_frame_present_flag: bool,
    /// Number of slice groups
    pub num_slice_groups: u32,
    /// Slice group parameters (if num_slice_groups > 1)
    pub slice_group_params: Option<SliceGroupParams>,
    /// Number of reference pictures in list 0 (0-32)
    pub num_ref_idx_l0_default_active: u32,
    /// Number of reference pictures in list 1 (0-32)
    pub num_ref_idx_l1_default_active: u32,
    /// Weighted prediction flag for P and SP slices
    pub weighted_pred_flag: bool,
    /// Weighted biprediction IDC for B slices (0, 1, or 2)
    pub weighted_bipred_idc: u8,
    /// Initial QP for slices
    pub pic_init_qp: i32,
    /// Initial QP for SP/SI slices
    pub pic_init_qs: i32,
    /// Chroma QP index offset (-12 to 12)
    pub chroma_qp_index_offset: i32,
    /// Deblocking filter control present flag
    pub deblocking_filter_control_present_flag: bool,
    /// Constrained intra prediction flag
    pub constrained_intra_pred_flag: bool,
    /// Redundant picture count present flag
    pub redundant_pic_cnt_present_flag: bool,
    /// Transform 8x8 mode flag (for High Profile and above)
    pub transform_8x8_mode_flag: bool,
    /// PPS scaling matrix (if pic_scaling_matrix_present_flag is true)
    pub scaling_matrix: Option<PpsScalingMatrix>,
    /// Second chroma QP index offset (for High Profile and above)
    pub second_chroma_qp_index_offset: i32,
}

/// PPS Scaling Matrix
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PpsScalingMatrix {
    /// 4x4 scaling lists (always 6)
    pub scaling_list_4x4: [ScalingList4x4; 6],
    /// 8x8 scaling lists (6 for chroma_format_idc == 3, otherwise 2)
    pub scaling_list_8x8: SmallVec<[ScalingList8x8; 6]>,
    /// Flags indicating which 4x4 lists are present in the bitstream
    pub scaling_list_4x4_present: [bool; 6],
    /// Flags indicating which 8x8 lists are present in the bitstream
    pub scaling_list_8x8_present: SmallVec<[bool; 6]>,
}

impl Default for PpsScalingMatrix {
    fn default() -> Self {
        Self {
            scaling_list_4x4: [ScalingList4x4::default(); 6],
            scaling_list_8x8: smallvec::smallvec![ScalingList8x8::default(); 2],
            scaling_list_4x4_present: [false; 6],
            scaling_list_8x8_present: smallvec::smallvec![false, false],
        }
    }
}

impl PpsScalingMatrix {
    /// Create a new PPS scaling matrix with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create PPS scaling matrix with the specified number of 8x8 lists
    pub fn with_8x8_count(count: usize) -> Self {
        Self {
            scaling_list_4x4: [ScalingList4x4::default(); 6],
            scaling_list_8x8: smallvec::smallvec![ScalingList8x8::default(); count],
            scaling_list_4x4_present: [false; 6],
            scaling_list_8x8_present: smallvec::smallvec![false; count],
        }
    }

    /// Parse PPS scaling matrix
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>, transform_8x8_mode_flag: bool, chroma_format_idc: u32) -> Result<Self> {
        let num_8x8_lists = if !transform_8x8_mode_flag {
            0
        } else if chroma_format_idc == 3 {
            6
        } else {
            2
        };

        let total_lists = 6 + num_8x8_lists;
        let mut matrix = Self::with_8x8_count(num_8x8_lists);

        // Parse 4x4 scaling lists (always 6)
        for i in 0..6 {
            let present = reader.read_bit()?;
            matrix.scaling_list_4x4_present[i] = present;

            if present {
                matrix.scaling_list_4x4[i] = ScalingList4x4::parse(reader)?;
            } else {
                // PPS fallback rule: use SPS scaling list (indicated by Fallback)
                matrix.scaling_list_4x4[i] = ScalingList4x4::fallback();
            }
        }

        // Parse 8x8 scaling lists (if transform_8x8_mode_flag)
        for i in 0..num_8x8_lists {
            if 6 + i < total_lists {
                let present = reader.read_bit()?;
                matrix.scaling_list_8x8_present[i] = present;

                if present {
                    matrix.scaling_list_8x8[i] = ScalingList8x8::parse(reader)?;
                } else {
                    // PPS fallback rule: use SPS scaling list (indicated by Fallback)
                    matrix.scaling_list_8x8[i] = ScalingList8x8::fallback();
                }
            }
        }

        Ok(matrix)
    }
}

impl Pps {
    /// Parse PPS from raw NAL unit RBSP data (with EPB already removed)
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::parse_with_sps_info(data, 1, false)
    }

    /// Parse PPS from raw NAL unit RBSP data with SPS information
    pub fn parse_with_sps_info(data: &[u8], chroma_format_idc: u32, has_separate_colour_plane: bool) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, chroma_format_idc, has_separate_colour_plane)
    }

    /// Parse PPS from a BitReader
    pub fn parse_from_bit_reader<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        chroma_format_idc: u32,
        _has_separate_colour_plane: bool,
    ) -> Result<Self> {
        // Read pic_parameter_set_id
        let pic_parameter_set_id = reader.read_ue()?;
        if pic_parameter_set_id > 255 {
            return Err(invalid_data_error!("pic_parameter_set_id", pic_parameter_set_id));
        }
        let pic_parameter_set_id = pic_parameter_set_id as u8;

        // Read seq_parameter_set_id
        let seq_parameter_set_id = reader.read_ue()?;
        if seq_parameter_set_id > 31 {
            return Err(invalid_data_error!("seq_parameter_set_id", seq_parameter_set_id));
        }
        let seq_parameter_set_id = seq_parameter_set_id as u8;

        // Read entropy_coding_mode_flag
        let entropy_coding_mode_flag = reader.read_bit()?;

        // Read bottom_field_pic_order_in_frame_present_flag
        let bottom_field_pic_order_in_frame_present_flag = reader.read_bit()?;

        // Read num_slice_groups and convert
        let num_slice_groups = reader.read_ue()? + 1;

        // Read slice group parameters if more than one slice group
        let slice_group_params = if num_slice_groups > 1 {
            Some(SliceGroupParams::parse(reader, num_slice_groups)?)
        } else {
            None
        };

        // Read num_ref_idx_l0_default_active and convert
        let num_ref_idx_l0_default_active = reader.read_ue()? + 1;
        if num_ref_idx_l0_default_active > 32 {
            return Err(invalid_data_error!("num_ref_idx_l0_default_active", num_ref_idx_l0_default_active));
        }

        // Read num_ref_idx_l1_default_active and convert
        let num_ref_idx_l1_default_active = reader.read_ue()? + 1;
        if num_ref_idx_l1_default_active > 32 {
            return Err(invalid_data_error!("num_ref_idx_l1_default_active", num_ref_idx_l1_default_active));
        }

        // Read weighted_pred_flag
        let weighted_pred_flag = reader.read_bit()?;

        // Read weighted_bipred_idc
        let weighted_bipred_idc = reader.read::<2, u8>()?;

        // Read pic_init_qp and convert
        let pic_init_qp = reader.read_se()? + 26;

        // Read pic_init_qs and convert
        let pic_init_qs = reader.read_se()? + 26;

        // Read chroma_qp_index_offset
        let chroma_qp_index_offset = reader.read_se()?;
        if !(-12..=12).contains(&chroma_qp_index_offset) {
            return Err(invalid_data_error!("chroma_qp_index_offset", chroma_qp_index_offset));
        }

        // Read deblocking_filter_control_present_flag
        let deblocking_filter_control_present_flag = reader.read_bit()?;

        // Read constrained_intra_pred_flag
        let constrained_intra_pred_flag = reader.read_bit()?;

        // Read redundant_pic_cnt_present_flag
        let redundant_pic_cnt_present_flag = reader.read_bit()?;

        // Initialize defaults for optional fields
        let mut transform_8x8_mode_flag = false;
        let mut scaling_matrix = None;
        let mut second_chroma_qp_index_offset = chroma_qp_index_offset;

        // Try to parse High Profile extensions (transform_8x8_mode_flag, etc.)
        // If any read fails or values are invalid, assume it was rbsp_trailing_bits
        if let Some((flag, second_offset, matrix)) = (|| -> Option<_> {
            let flag = reader.read_bit().ok()?;
            let scaling_present = reader.read_bit().ok()?;
            let second_offset = reader.read_se().ok().filter(|v| (-12..=12).contains(v))?;
            let matrix = scaling_present.then(|| PpsScalingMatrix::parse(reader, flag, chroma_format_idc).ok()).flatten();
            Some((flag, second_offset, matrix))
        })() {
            transform_8x8_mode_flag = flag;
            second_chroma_qp_index_offset = second_offset;
            scaling_matrix = matrix;
        }

        Ok(Self {
            pic_parameter_set_id,
            seq_parameter_set_id,
            entropy_coding_mode_flag,
            bottom_field_pic_order_in_frame_present_flag,
            num_slice_groups,
            slice_group_params,
            num_ref_idx_l0_default_active,
            num_ref_idx_l1_default_active,
            weighted_pred_flag,
            weighted_bipred_idc,
            pic_init_qp,
            pic_init_qs,
            chroma_qp_index_offset,
            deblocking_filter_control_present_flag,
            constrained_intra_pred_flag,
            redundant_pic_cnt_present_flag,
            transform_8x8_mode_flag,
            scaling_matrix,
            second_chroma_qp_index_offset,
        })
    }

    /// Check if CABAC entropy coding is used
    #[inline]
    pub fn is_cabac(&self) -> bool {
        self.entropy_coding_mode_flag
    }

    /// Check if CAVLC entropy coding is used
    #[inline]
    pub fn is_cavlc(&self) -> bool {
        !self.entropy_coding_mode_flag
    }

    /// Check if weighted prediction is enabled for P/SP slices
    #[inline]
    pub fn has_weighted_prediction(&self) -> bool {
        self.weighted_pred_flag
    }

    /// Check if weighted bi-prediction is enabled for B slices
    #[inline]
    pub fn has_weighted_bi_prediction(&self) -> bool {
        self.weighted_bipred_idc != 0
    }

    /// Get weighted bi-prediction mode
    /// 0 = Default, 1 = Explicit, 2 = Implicit
    #[inline]
    pub fn weighted_bi_prediction_mode(&self) -> u8 {
        self.weighted_bipred_idc
    }

    /// Check if 8x8 transform is enabled
    #[inline]
    pub fn has_8x8_transform(&self) -> bool {
        self.transform_8x8_mode_flag
    }

    /// Check if deblocking filter control is present in slice headers
    #[inline]
    pub fn has_deblocking_filter_control(&self) -> bool {
        self.deblocking_filter_control_present_flag
    }

    /// Check if constrained intra prediction is used
    #[inline]
    pub fn is_constrained_intra_pred(&self) -> bool {
        self.constrained_intra_pred_flag
    }

    /// Check if redundant pictures may be present
    #[inline]
    pub fn has_redundant_pic_cnt(&self) -> bool {
        self.redundant_pic_cnt_present_flag
    }

    /// Get the Cb QP offset
    #[inline]
    pub fn cb_qp_offset(&self) -> i32 {
        self.chroma_qp_index_offset
    }

    /// Get the Cr QP offset
    #[inline]
    pub fn cr_qp_offset(&self) -> i32 {
        self.second_chroma_qp_index_offset
    }
}
