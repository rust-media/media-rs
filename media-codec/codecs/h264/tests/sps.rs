use media_codec_h264::sps::{ChromaFormat, ConstraintSetFlags, Sps};

#[rustfmt::skip]
const SPS_DATA: &[u8] = &[
    0x42, // profile_idc = 66
    0xC0, // flag0 = 1, flag1 = 1, others = 0, reserved = 0
    0x0A, // level_idc = 10
    0xDA, // 1101 1010
    0x0B, // 0000 1011
    0x13, // 0001 0011
    0x90, // 1001 0000 (last bit is rbsp_stop_one_bit, rest is padding)
];

#[test]
fn test_parse_sps() {
    let sps = Sps::parse(SPS_DATA).unwrap();

    assert_eq!(sps.profile_idc, 66); // Baseline
    assert_eq!(sps.level_idc, 10); // Level 1.0
    assert_eq!(sps.seq_parameter_set_id, 0);
    assert_eq!(sps.chroma_format, ChromaFormat::YUV420); // Default for baseline
    assert_eq!(sps.bit_depth_luma_minus8, 0); // Raw value
    assert_eq!(sps.bit_depth_luma(), 8); // Corrected value
    assert_eq!(sps.bit_depth_chroma_minus8, 0); // Raw value
    assert_eq!(sps.bit_depth_chroma(), 8); // Corrected value

    // Check dimensions (raw values)
    assert_eq!(sps.pic_width_in_mbs_minus1, 10); // Raw: 176 / 16 - 1
    assert_eq!(sps.pic_height_in_map_units_minus1, 8); // Raw: 144 / 16 - 1
                                                       // Corrected values via getter methods
    assert_eq!(sps.pic_width_in_mbs(), 11); // 176 / 16
    assert_eq!(sps.pic_height_in_map_units(), 9); // 144 / 16
    assert!(sps.frame_mbs_only_flag);

    // Check calculated dimensions
    assert_eq!(sps.width(), 176);
    assert_eq!(sps.height(), 144);

    // No VUI
    assert!(sps.vui_parameters.is_none());
}
