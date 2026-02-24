use media_codec_h265::pps::Pps;

#[rustfmt::skip]
const PPS_DATA: &[u8] = &[
    0xC1, // 1100 0001
    0x72, // 0111 0010
    0xB4, // 1011 0100
    0x62, // 0110 0010
    0x40, // 0100 0000
];

#[test]
fn test_parse_pps() {
    let pps = Pps::parse(PPS_DATA).unwrap();

    assert_eq!(pps.pic_parameter_set_id, 0);
    assert_eq!(pps.seq_parameter_set_id, 0);
    assert_eq!(pps.dependent_slice_segments_enabled_flag, false);
    assert_eq!(pps.sign_data_hiding_enabled_flag, true);
    assert_eq!(pps.cabac_init_present_flag, false);
    assert_eq!(pps.transform_skip_enabled_flag, false);
    assert_eq!(pps.cu_qp_delta_enabled_flag, true);
    assert_eq!(pps.tiles_enabled_flag, false);
    assert_eq!(pps.tile_info, None);
    assert_eq!(pps.pps_loop_filter_across_slices_enabled_flag, true);
}
