use media_codec_h265::vps::Vps;

#[rustfmt::skip]
const VPS_DATA: &[u8] = &[
    0x0C, // vps_id = 0, base_internal = 1, base_available = 1, max_layers[5:4] = 0
    0x01, // max_layers[3:0] = 0, max_sub_layers = 0, temporal_id_nesting = 1
    0xFF, 0xFF, // reserved
    0x01, // profile_space = 0, tier = 0(Main), profile_idc = 1(Main)
    0x60, 0x00, 0x00, 0x00, // profile_compatibility_flags
    0xB0, 0x00, 0x00, 0x00, 0x00, 0x00, // constraint_flags (48 bits)
    0x5D, // level_idc = 93 (Level 3.1)
    0x95, 0xC0, 0x80, // 1001 0101 1100 0000 1000 0000
];

#[test]
fn test_parse_vps() {
    let vps = Vps::parse(VPS_DATA).unwrap();

    assert_eq!(vps.video_parameter_set_id, 0);
    assert_eq!(vps.vps_max_layers_minus1, 0);
    assert_eq!(vps.vps_max_sub_layers_minus1, 0);
    assert_eq!(vps.vps_temporal_id_nesting_flag, true);

    assert_eq!(vps.profile_tier_level.general_profile_idc, 1); // Main
    assert_eq!(vps.profile_tier_level.general_tier_flag, false); // Main tier
    assert_eq!(vps.profile_tier_level.general_level_idc, 93); // Level 3.1

    assert_eq!(vps.vps_num_layer_sets_minus1, 0);
    assert_eq!(vps.vps_timing_info_present_flag, false);
}
