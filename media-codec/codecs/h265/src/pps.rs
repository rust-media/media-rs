//! H.265/HEVC Picture Parameter Set (PPS) parser

use std::io::Read;

use media_codec_bitstream::{BigEndian, BitReader};
use media_core::{invalid_data_error, none_param_error, not_found_error, Result};
use smallvec::SmallVec;

use crate::{
    constants::{MAX_PPS_COUNT, MAX_QP_OFFSET, MAX_REFS, MAX_SPS_COUNT, MAX_TILE_COLUMNS, MAX_TILE_ROWS, MIN_QP_OFFSET},
    ps::ParameterSets,
    scaling_list::ScalingListData,
    sps::{ChromaFormat, Sps},
};

/// Tile information in PPS
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TileInfo {
    /// Number of tile columns
    pub num_tile_columns: u32,
    /// Number of tile rows
    pub num_tile_rows: u32,
    /// Uniform spacing flag
    pub uniform_spacing_flag: bool,
    /// Column widths (if not uniform spacing)
    pub column_width: SmallVec<[u32; MAX_TILE_COLUMNS]>,
    /// Row heights (if not uniform spacing)
    pub row_height: SmallVec<[u32; MAX_TILE_ROWS]>,
    /// Loop filter across tiles enabled flag
    pub loop_filter_across_tiles_enabled_flag: bool,
}

impl TileInfo {
    /// Parse TileInfo from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        let num_tile_columns = reader.read_ue()? + 1;
        let num_tile_rows = reader.read_ue()? + 1;
        let uniform_spacing_flag = reader.read_bit()?;

        let mut column_width = SmallVec::new();
        let mut row_height = SmallVec::new();

        if !uniform_spacing_flag {
            column_width.reserve((num_tile_columns - 1) as usize);
            for _ in 0..(num_tile_columns - 1) {
                column_width.push(reader.read_ue()? + 1);
            }

            row_height.reserve((num_tile_rows - 1) as usize);
            for _ in 0..(num_tile_rows - 1) {
                row_height.push(reader.read_ue()? + 1);
            }
        }

        let loop_filter_across_tiles_enabled_flag = reader.read_bit()?;

        Ok(Self {
            num_tile_columns,
            num_tile_rows,
            uniform_spacing_flag,
            column_width,
            row_height,
            loop_filter_across_tiles_enabled_flag,
        })
    }

    /// Get number of tile columns
    #[inline]
    pub fn num_tile_columns(&self) -> u32 {
        self.num_tile_columns
    }

    /// Get number of tile rows
    #[inline]
    pub fn num_tile_rows(&self) -> u32 {
        self.num_tile_rows
    }

    /// Get total number of tiles
    #[inline]
    pub fn num_tiles(&self) -> u32 {
        self.num_tile_columns() * self.num_tile_rows()
    }
}

/// Deblocking filter override parameters
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeblockingFilterParams {
    /// Deblocking filter override enabled flag
    pub deblocking_filter_override_enabled_flag: bool,
    /// PPS deblocking filter disabled flag
    pub pps_deblocking_filter_disabled_flag: bool,
    /// PPS beta offset
    pub pps_beta_offset: i32,
    /// PPS tc offset
    pub pps_tc_offset: i32,
}

impl DeblockingFilterParams {
    /// Parse DeblockingFilterParams from a BitReader
    pub fn parse<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<Self> {
        let deblocking_filter_override_enabled_flag = reader.read_bit()?;
        let pps_deblocking_filter_disabled_flag = reader.read_bit()?;

        let (pps_beta_offset, pps_tc_offset) = if !pps_deblocking_filter_disabled_flag {
            (reader.read_se()? * 2, reader.read_se()? * 2)
        } else {
            (0, 0)
        };

        Ok(Self {
            deblocking_filter_override_enabled_flag,
            pps_deblocking_filter_disabled_flag,
            pps_beta_offset,
            pps_tc_offset,
        })
    }
}

/// Picture Parameter Set (PPS)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Pps {
    /// PPS ID (0-63)
    pub pic_parameter_set_id: u8,
    /// SPS ID that this PPS refers to (0-15)
    pub seq_parameter_set_id: u8,
    /// Dependent slice segments enabled flag
    pub dependent_slice_segments_enabled_flag: bool,
    /// Output flag present flag
    pub output_flag_present_flag: bool,
    /// Number of extra slice header bits
    pub num_extra_slice_header_bits: u8,
    /// Sign data hiding enabled flag
    pub sign_data_hiding_enabled_flag: bool,
    /// CABAC init present flag
    pub cabac_init_present_flag: bool,
    /// Number of reference pictures in list 0
    pub num_ref_idx_l0_default_active: u32,
    /// Number of reference pictures in list 1
    pub num_ref_idx_l1_default_active: u32,
    /// Initial QP
    pub init_qp: i32,
    /// Constrained intra prediction flag
    pub constrained_intra_pred_flag: bool,
    /// Transform skip enabled flag
    pub transform_skip_enabled_flag: bool,
    /// CU QP delta enabled flag
    pub cu_qp_delta_enabled_flag: bool,
    /// Diff CU QP delta depth
    pub diff_cu_qp_delta_depth: u32,
    /// Cb QP offset
    pub pps_cb_qp_offset: i32,
    /// Cr QP offset
    pub pps_cr_qp_offset: i32,
    /// PPS slice chroma QP offsets present flag
    pub pps_slice_chroma_qp_offsets_present_flag: bool,
    /// Weighted prediction flag
    pub weighted_pred_flag: bool,
    /// Weighted biprediction flag
    pub weighted_bipred_flag: bool,
    /// Transquant bypass enabled flag
    pub transquant_bypass_enabled_flag: bool,
    /// Tiles enabled flag
    pub tiles_enabled_flag: bool,
    /// Entropy coding sync enabled flag
    pub entropy_coding_sync_enabled_flag: bool,
    /// Tile info (if tiles_enabled_flag)
    pub tile_info: Option<TileInfo>,
    /// PPS loop filter across slices enabled flag
    pub pps_loop_filter_across_slices_enabled_flag: bool,
    /// Deblocking filter control present flag
    pub deblocking_filter_control_present_flag: bool,
    /// Deblocking filter params (if deblocking_filter_control_present_flag)
    pub deblocking_filter_params: Option<DeblockingFilterParams>,
    /// PPS scaling list data present flag
    pub pps_scaling_list_data_present_flag: bool,
    /// Scaling list data (if pps_scaling_list_data_present_flag)
    pub scaling_list_data: Option<ScalingListData>,
    /// Lists modification present flag
    pub lists_modification_present_flag: bool,
    /// Log2 parallel merge level
    pub log2_parallel_merge_level: u32,
    /// Slice segment header extension present flag
    pub slice_segment_header_extension_present_flag: bool,
    /// PPS extension present flag
    pub pps_extension_present_flag: bool,
    /// PPS range extension flag
    pub pps_range_extension_flag: bool,
    /// PPS multilayer extension flag
    pub pps_multilayer_extension_flag: bool,
    /// PPS 3D extension flag
    pub pps_3d_extension_flag: bool,
    /// PPS SCC extension flag
    pub pps_scc_extension_flag: bool,
    /// PPS extension 4 bits
    pub pps_extension_4bits: u8,
}

impl Pps {
    pub fn parse_ids(data: &[u8]) -> Result<(u8, u8)> {
        let mut reader = BitReader::new(data);
        Self::parse_ids_from_bit_reader(&mut reader)
    }

    pub fn parse_with_sps(data: &[u8], sps: &Sps) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, Some(sps), None)
    }

    pub fn parse_with_param_sets(data: &[u8], param_sets: &ParameterSets) -> Result<Self> {
        let mut reader = BitReader::new(data);
        Self::parse_from_bit_reader(&mut reader, None, Some(param_sets))
    }

    pub fn parse_ids_from_bit_reader<R: Read>(reader: &mut BitReader<R, BigEndian>) -> Result<(u8, u8)> {
        // Read pic_parameter_set_id
        let pic_parameter_set_id = reader.read_ue()?;
        if pic_parameter_set_id as usize >= MAX_PPS_COUNT {
            return Err(invalid_data_error!("pps_id", pic_parameter_set_id));
        }
        let pic_parameter_set_id = pic_parameter_set_id as u8;

        // Read seq_parameter_set_id
        let seq_parameter_set_id = reader.read_ue()?;
        if seq_parameter_set_id as usize >= MAX_SPS_COUNT {
            return Err(invalid_data_error!("sps_id", seq_parameter_set_id));
        }
        let seq_parameter_set_id = seq_parameter_set_id as u8;

        Ok((pic_parameter_set_id, seq_parameter_set_id))
    }

    /// Parse PPS from a BitReader
    pub fn parse_from_bit_reader<R: Read>(
        reader: &mut BitReader<R, BigEndian>,
        sps: Option<&Sps>,
        param_sets: Option<&ParameterSets>,
    ) -> Result<Self> {
        let (pic_parameter_set_id, seq_parameter_set_id) = Self::parse_ids_from_bit_reader(reader)?;

        let sps = if let Some(sps) = sps {
            if seq_parameter_set_id != sps.seq_parameter_set_id {
                return Err(invalid_data_error!("sps_id", seq_parameter_set_id));
            }

            sps
        } else if let Some(param_sets) = param_sets {
            param_sets.get_sps(seq_parameter_set_id as u32).ok_or_else(|| not_found_error!("sps_id", seq_parameter_set_id))?
        } else {
            return Err(none_param_error!("sps or param_sets"));
        };

        // Read dependent_slice_segments_enabled_flag
        let dependent_slice_segments_enabled_flag = reader.read_bit()?;

        // Read output_flag_present_flag
        let output_flag_present_flag = reader.read_bit()?;

        // Read num_extra_slice_header_bits
        let num_extra_slice_header_bits = reader.read::<3, u8>()?;

        // Read sign_data_hiding_enabled_flag
        let sign_data_hiding_enabled_flag = reader.read_bit()?;

        // Read cabac_init_present_flag
        let cabac_init_present_flag = reader.read_bit()?;

        // Read num_ref_idx_l0_default_active
        let num_ref_idx_l0_default_active = reader.read_ue()? + 1;
        if num_ref_idx_l0_default_active as usize > MAX_REFS {
            return Err(invalid_data_error!("num_ref_idx_l0_default_active", num_ref_idx_l0_default_active));
        }

        // Read num_ref_idx_l1_default_active
        let num_ref_idx_l1_default_active = reader.read_ue()? + 1;
        if num_ref_idx_l1_default_active as usize > MAX_REFS {
            return Err(invalid_data_error!("num_ref_idx_l1_default_active", num_ref_idx_l1_default_active));
        }

        // Read init_qp
        let init_qp = reader.read_se()? + 26;

        // Read constrained_intra_pred_flag
        let constrained_intra_pred_flag = reader.read_bit()?;

        // Read transform_skip_enabled_flag
        let transform_skip_enabled_flag = reader.read_bit()?;

        // Read cu_qp_delta_enabled_flag and diff_cu_qp_delta_depth
        let cu_qp_delta_enabled_flag = reader.read_bit()?;
        let diff_cu_qp_delta_depth = if cu_qp_delta_enabled_flag {
            reader.read_ue()?
        } else {
            0
        };

        // Read pps_cb_qp_offset (MIN_QP_OFFSET..MAX_QP_OFFSET)
        let pps_cb_qp_offset = reader.read_se()?;
        if !(MIN_QP_OFFSET..=MAX_QP_OFFSET).contains(&pps_cb_qp_offset) {
            return Err(invalid_data_error!("pps_cb_qp_offset", pps_cb_qp_offset));
        }

        // Read pps_cr_qp_offset (MIN_QP_OFFSET..MAX_QP_OFFSET)
        let pps_cr_qp_offset = reader.read_se()?;
        if !(MIN_QP_OFFSET..=MAX_QP_OFFSET).contains(&pps_cr_qp_offset) {
            return Err(invalid_data_error!("pps_cr_qp_offset", pps_cr_qp_offset));
        }

        // Read pps_slice_chroma_qp_offsets_present_flag
        let pps_slice_chroma_qp_offsets_present_flag = reader.read_bit()?;

        // Read weighted_pred_flag
        let weighted_pred_flag = reader.read_bit()?;

        // Read weighted_bipred_flag
        let weighted_bipred_flag = reader.read_bit()?;

        // Read transquant_bypass_enabled_flag
        let transquant_bypass_enabled_flag = reader.read_bit()?;

        // Read tiles_enabled_flag
        let tiles_enabled_flag = reader.read_bit()?;

        // Read entropy_coding_sync_enabled_flag
        let entropy_coding_sync_enabled_flag = reader.read_bit()?;

        // Parse tile info (if tiles_enabled_flag)
        let tile_info = if tiles_enabled_flag {
            Some(TileInfo::parse(reader)?)
        } else {
            None
        };

        // Read pps_loop_filter_across_slices_enabled_flag
        let pps_loop_filter_across_slices_enabled_flag = reader.read_bit()?;

        // Read deblocking_filter_control_present_flag and parse deblocking filter
        // params
        let deblocking_filter_control_present_flag = reader.read_bit()?;
        let deblocking_filter_params = if deblocking_filter_control_present_flag {
            Some(DeblockingFilterParams::parse(reader)?)
        } else {
            None
        };

        // Read pps_scaling_list_data_present_flag and parse scaling list data
        let pps_scaling_list_data_present_flag = reader.read_bit()?;
        let scaling_list_data = if pps_scaling_list_data_present_flag {
            let is_444 = sps.chroma_format == ChromaFormat::YUV444;
            Some(ScalingListData::parse(reader, is_444)?)
        } else {
            None
        };

        // Read lists_modification_present_flag
        let lists_modification_present_flag = reader.read_bit()?;

        // Read log2_parallel_merge_level
        let log2_parallel_merge_level = reader.read_ue()? + 2;

        // Read slice_segment_header_extension_present_flag
        let slice_segment_header_extension_present_flag = reader.read_bit()?;

        // Read pps_extension_present_flag and extension flags
        let pps_extension_present_flag = reader.read_bit()?;
        let (pps_range_extension_flag, pps_multilayer_extension_flag, pps_3d_extension_flag, pps_scc_extension_flag, pps_extension_4bits) =
            if pps_extension_present_flag {
                (reader.read_bit()?, reader.read_bit()?, reader.read_bit()?, reader.read_bit()?, reader.read::<4, u8>()?)
            } else {
                (false, false, false, false, 0)
            };

        Ok(Self {
            pic_parameter_set_id,
            seq_parameter_set_id,
            dependent_slice_segments_enabled_flag,
            output_flag_present_flag,
            num_extra_slice_header_bits,
            sign_data_hiding_enabled_flag,
            cabac_init_present_flag,
            num_ref_idx_l0_default_active,
            num_ref_idx_l1_default_active,
            init_qp,
            constrained_intra_pred_flag,
            transform_skip_enabled_flag,
            cu_qp_delta_enabled_flag,
            diff_cu_qp_delta_depth,
            pps_cb_qp_offset,
            pps_cr_qp_offset,
            pps_slice_chroma_qp_offsets_present_flag,
            weighted_pred_flag,
            weighted_bipred_flag,
            transquant_bypass_enabled_flag,
            tiles_enabled_flag,
            entropy_coding_sync_enabled_flag,
            tile_info,
            pps_loop_filter_across_slices_enabled_flag,
            deblocking_filter_control_present_flag,
            deblocking_filter_params,
            pps_scaling_list_data_present_flag,
            scaling_list_data,
            lists_modification_present_flag,
            log2_parallel_merge_level,
            slice_segment_header_extension_present_flag,
            pps_extension_present_flag,
            pps_range_extension_flag,
            pps_multilayer_extension_flag,
            pps_3d_extension_flag,
            pps_scc_extension_flag,
            pps_extension_4bits,
        })
    }

    /// Get actual number of reference pictures in list 0
    #[inline]
    pub fn number_of_reference_index_l0_default_active(&self) -> u32 {
        self.num_ref_idx_l0_default_active
    }

    /// Get actual number of reference pictures in list 1
    #[inline]
    pub fn number_of_reference_index_l1_default_active(&self) -> u32 {
        self.num_ref_idx_l1_default_active
    }

    /// Get initial QP (actual value: 0 to 51)
    #[inline]
    pub fn init_qp(&self) -> i32 {
        self.init_qp
    }

    /// Get log2 parallel merge level
    #[inline]
    pub fn log2_parallel_merge_level(&self) -> u32 {
        self.log2_parallel_merge_level
    }

    /// Check if weighted prediction is enabled for P slices
    #[inline]
    pub fn has_weighted_prediction(&self) -> bool {
        self.weighted_pred_flag
    }

    /// Check if weighted bi-prediction is enabled for B slices
    #[inline]
    pub fn has_weighted_bi_prediction(&self) -> bool {
        self.weighted_bipred_flag
    }

    /// Check if tiles are enabled
    #[inline]
    pub fn has_tiles(&self) -> bool {
        self.tiles_enabled_flag
    }

    /// Check if WPP (Wavefront Parallel Processing) is enabled
    #[inline]
    pub fn has_wpp(&self) -> bool {
        self.entropy_coding_sync_enabled_flag
    }

    /// Check if transform skip is enabled
    #[inline]
    pub fn has_transform_skip(&self) -> bool {
        self.transform_skip_enabled_flag
    }

    /// Check if CU QP delta is enabled
    #[inline]
    pub fn has_cu_qp_delta(&self) -> bool {
        self.cu_qp_delta_enabled_flag
    }

    /// Check if deblocking filter is disabled
    #[inline]
    pub fn is_deblocking_filter_disabled(&self) -> bool {
        self.deblocking_filter_params.as_ref().is_some_and(|p| p.pps_deblocking_filter_disabled_flag)
    }

    /// Get the Cb QP offset
    #[inline]
    pub fn cb_qp_offset(&self) -> i32 {
        self.pps_cb_qp_offset
    }

    /// Get the Cr QP offset
    #[inline]
    pub fn cr_qp_offset(&self) -> i32 {
        self.pps_cr_qp_offset
    }

    /// Get number of tile columns (1 if tiles not enabled)
    #[inline]
    pub fn num_tile_columns(&self) -> u32 {
        self.tile_info.as_ref().map_or(1, |t| t.num_tile_columns())
    }

    /// Get number of tile rows (1 if tiles not enabled)
    #[inline]
    pub fn num_tile_rows(&self) -> u32 {
        self.tile_info.as_ref().map_or(1, |t| t.num_tile_rows())
    }

    /// Get total number of tiles (1 if tiles not enabled)
    #[inline]
    pub fn num_tiles(&self) -> u32 {
        self.tile_info.as_ref().map_or(1, |t| t.num_tiles())
    }
}
