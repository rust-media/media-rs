use media_codec_h264::{
    avcc::Avcc,
    pps::Pps,
    sps::{ChromaFormat, Sps},
    H264NalHeader, NalUnitType,
};
use media_codec_nal::{NalHeader, NalParser};

// AVCC data
#[rustfmt::skip]
const AVCC_DATA: &[u8] = &[
    0x01, 0x42, 0xC0, 0x1E, 0xFF, 0xE1, 0x00, 0x09, 0x67, 0x42, 0xC0, 0x1E,
    0xD9, 0x00, 0x50, 0x05, 0xB9, 0x01, 0x00, 0x04, 0x68, 0xCB, 0x8F, 0x20,
];

#[test]
fn test_parse_avcc() {
    let avcc = Avcc::parse(AVCC_DATA).unwrap();

    assert_eq!(avcc.configuration_version, 1);
    assert_eq!(avcc.avc_profile_indication, 66); // Baseline Profile
    assert_eq!(avcc.profile_compatibility, 0xC0);
    assert_eq!(avcc.avc_level_indication, 30); // Level 3.0
    assert_eq!(avcc.length_size, 4);
    assert_eq!(avcc.sequence_parameter_sets.len(), 1);
    assert_eq!(avcc.picture_parameter_sets.len(), 1);
    assert_eq!(avcc.sequence_parameter_sets[0].len(), 9);
    assert_eq!(avcc.picture_parameter_sets[0].len(), 4);

    // Baseline profile doesn't have extended configuration
    assert!(avcc.ext.is_none());
}

#[test]
fn test_parse_sps() {
    let avcc = Avcc::parse(AVCC_DATA).unwrap();
    let parser = NalParser::<H264NalHeader>::new(None);

    // Parse NAL unit and extract SPS
    let nal_unit = parser.parse(&avcc.sequence_parameter_sets[0]).unwrap();

    // Verify NAL header
    let header = nal_unit.header();
    assert!(!header.forbidden_zero_bit);
    assert_eq!(header.nal_ref_idc, 3);
    assert_eq!(header.nal_unit_type(), NalUnitType::Sps as u8);
    assert!(header.is_parameter_set());
    assert!(!header.is_vcl());

    // Parse and verify SPS
    let sps = Sps::parse(nal_unit.payload()).unwrap();
    assert_eq!(sps.profile_idc, 66); // Baseline Profile
    assert_eq!(sps.level_idc, 30); // Level 3.0
    assert_eq!(sps.seq_parameter_set_id, 0);
    // Baseline profile uses default chroma format (YUV420)
    assert_eq!(sps.chroma_format, ChromaFormat::YUV420);
    assert_eq!(sps.bit_depth_luma, 8);
    assert_eq!(sps.bit_depth_chroma, 8);

    // Verify frame number and POC type
    assert_eq!(sps.log2_max_frame_num, 4);
    assert_eq!(sps.pic_order_cnt_type, 2);

    // Verify reference frames
    assert_eq!(sps.max_num_ref_frames, 3);

    // Verify picture dimensions
    assert_eq!(sps.pic_width_in_mbs, 80); // 1280 pixels
    assert_eq!(sps.pic_height_in_map_units, 45); // 720 pixels
    assert_eq!(sps.width(), 1280);
    assert_eq!(sps.height(), 720);

    // Verify frame/field flags
    assert!(sps.frame_mbs_only_flag);
    assert!(sps.direct_8x8_inference_flag);

    // Verify no cropping
    assert!(!sps.frame_cropping_flag);

    // Verify no VUI
    assert!(sps.vui_parameters.is_none());
}

#[test]
fn test_parse_pps() {
    let avcc = Avcc::parse(AVCC_DATA).unwrap();
    let parser = NalParser::<H264NalHeader>::new(None);

    // Parse SPS (needed for PPS parsing)
    let sps_nal = parser.parse(&avcc.sequence_parameter_sets[0]).unwrap();
    let sps = Sps::parse(sps_nal.payload()).unwrap();

    // Parse NAL unit and extract PPS
    let nal_unit = parser.parse(&avcc.picture_parameter_sets[0]).unwrap();

    // Verify NAL header
    let header = nal_unit.header();
    assert!(!header.forbidden_zero_bit);
    assert_eq!(header.nal_ref_idc, 3);
    assert_eq!(header.nal_unit_type(), NalUnitType::Pps as u8);
    assert!(header.is_parameter_set());
    assert!(!header.is_vcl());

    // Parse and verify PPS
    let pps = Pps::parse_with_sps(nal_unit.payload(), &sps).unwrap();

    // Verify basic PPS parameters
    assert_eq!(pps.pic_parameter_set_id, 0);
    assert_eq!(pps.seq_parameter_set_id, 0);

    // Verify entropy coding mode (Baseline profile must use CAVLC)
    assert!(pps.is_cavlc());
    assert!(!pps.is_cabac());
    assert!(!pps.entropy_coding_mode_flag);

    // Verify picture order count related flag
    assert!(!pps.bottom_field_pic_order_in_frame_present_flag);

    // Verify slice group parameters (1 slice group = no FMO)
    assert_eq!(pps.num_slice_groups, 1);
    assert!(pps.slice_group_params.is_none());

    // Verify reference picture list defaults
    assert_eq!(pps.num_ref_idx_l0_default_active, 3);
    assert_eq!(pps.num_ref_idx_l1_default_active, 1);

    // Verify weighted prediction flags
    assert!(!pps.weighted_pred_flag);
    assert!(!pps.has_weighted_prediction());
    assert_eq!(pps.weighted_bipred_idc, 0);
    assert!(!pps.has_weighted_bi_prediction());
    assert_eq!(pps.weighted_bi_prediction_mode(), 0);

    // Verify QP parameters
    assert_eq!(pps.pic_init_qp, 26);
    assert_eq!(pps.pic_init_qs, 26);
    assert_eq!(pps.chroma_qp_index_offset, 0);
    assert_eq!(pps.cb_qp_offset(), 0);
    assert_eq!(pps.cr_qp_offset(), 0);
    assert_eq!(pps.second_chroma_qp_index_offset, 0);

    // Verify deblocking filter control
    assert!(pps.deblocking_filter_control_present_flag);
    assert!(pps.has_deblocking_filter_control());

    // Verify constrained intra prediction
    assert!(!pps.constrained_intra_pred_flag);
    assert!(!pps.is_constrained_intra_pred());

    // Verify redundant picture count flag
    assert!(!pps.redundant_pic_cnt_present_flag);
    assert!(!pps.has_redundant_pic_cnt());

    // Verify 8x8 transform
    assert!(!pps.transform_8x8_mode_flag);
    assert!(!pps.has_8x8_transform());

    // Verify scaling matrix
    assert!(pps.scaling_matrix.is_none());
}
