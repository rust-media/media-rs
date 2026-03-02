use media_codec_h265::sps::{ChromaFormat, Sps};

#[rustfmt::skip]
const SPS_DATA: &[u8] = &[
    0x01,                               // vps_id = 0, max_sub_layers = 0, temporal_id_nesting = 1
    0x01,                               // profile_space = 0, tier = 0 (Main), profile_idc = 1 (Main)
    0x60, 0x00, 0x00, 0x00,             // profile_compatibility_flags
    0x90, 0x00, 0x00, 0x00, 0x00, 0x00, // constraint_flags (48 bits)
    0x3C,                               // level_idc = 60 (Level 2.0)
    // exp-golomb encoded fields (sps_id, chroma, width, height, etc.)
    0xA0, 0x0A, 0x08, 0x0F, 0x16, 0x59, 0x59, 0xA4,
    0x93, 0x2B, 0xC0, 0x5A, 0x02, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x00, 0x00, 0x3C, 0x10,
];

#[test]
fn test_parse_sps() {
    let sps = Sps::parse(SPS_DATA).unwrap();

    assert_eq!(sps.video_parameter_set_id, 0);
    assert_eq!(sps.sps_max_sub_layers, 1);
    assert_eq!(sps.sps_temporal_id_nesting_flag, true);

    // Profile/Tier/Level
    assert_eq!(sps.profile_tier_level.general_profile_idc, 1); // Main
    assert_eq!(sps.profile_tier_level.general_tier_flag, false); // Main tier
    assert_eq!(sps.profile_tier_level.general_level_idc, 60); // Level 2.0

    // Video format
    assert_eq!(sps.chroma_format, ChromaFormat::YUV420);
    assert_eq!(sps.pic_width_in_luma_samples, 320);
    assert_eq!(sps.pic_height_in_luma_samples, 240);
    assert_eq!(sps.bit_depth_luma, 8);
    assert_eq!(sps.bit_depth_chroma, 8);
}
