use media_codec_h264::pps::Pps;

#[rustfmt::skip]
const PPS_DATA: &[u8] = &[
    0xCE, // 1100 1110
    0x38, // 0011 1000
];

#[test]
fn test_parse_pps() {
    let pps = Pps::parse(PPS_DATA).unwrap();

    assert_eq!(pps.pic_parameter_set_id, 0);
    assert_eq!(pps.seq_parameter_set_id, 0);
    assert!(!pps.entropy_coding_mode_flag); // CAVLC
    assert!(pps.is_cavlc());
    assert!(!pps.is_cabac());
    assert!(!pps.bottom_field_pic_order_in_frame_present_flag);
    assert_eq!(pps.num_slice_groups_minus1, 0);
    assert_eq!(pps.num_slice_groups(), 1);
    assert_eq!(pps.num_ref_idx_l0_default_active_minus1, 0);
    assert_eq!(pps.number_of_reference_index_l0_default_active(), 1);
    assert_eq!(pps.num_ref_idx_l1_default_active_minus1, 0);
    assert_eq!(pps.number_of_reference_index_l1_default_active(), 1);
    assert!(!pps.weighted_pred_flag);
    assert!(!pps.has_weighted_prediction());
    assert_eq!(pps.weighted_bipred_idc, 0);
    assert!(!pps.has_weighted_bi_prediction());
    assert_eq!(pps.pic_init_qp_minus26, 0);
    assert_eq!(pps.pic_init_qp(), 26);
    assert_eq!(pps.pic_init_qs_minus26, 0);
    assert_eq!(pps.pic_init_qs(), 26);
    assert_eq!(pps.chroma_qp_index_offset, 0);
    assert_eq!(pps.cb_qp_offset(), 0);
    assert!(!pps.deblocking_filter_control_present_flag);
    assert!(!pps.constrained_intra_pred_flag);
    assert!(!pps.redundant_pic_cnt_present_flag);
}
