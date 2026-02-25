use media_codec_h265::{hvcc::Hvcc, pps::Pps, sps::Sps, vps::Vps, H265NalHeader, NalUnitType};
use media_codec_nal::NalParser;

// Sample HVCC data for testing
#[rustfmt::skip]
const HVCC_DATA: &[u8] = &[
    0x01,                               // configurationVersion = 1
    0x04,                               // general_profile_space = 0, general_tier_flag = 0, general_profile_idc = 4 (Main Intra)
    0x08, 0x00, 0x00, 0x00,             // general_profile_compatibility_flags
    0x9F, 0xA8, 0x00, 0x00, 0x00, 0x00, // general_constraint_indicator_flags (48 bits)
    0x3C,                               // general_level_idc = 60 (Level 2.0)
    0xF0, 0x00,                         // min_spatial_segmentation_idc = 0
    0xFC,                               // parallelism_type = 0
    0xFD,                               // chroma_format_idc = 1 (4:2:0)
    0xF8,                               // bit_depth_luma_minus8 = 0 (bit_depth = 8)
    0xF8,                               // bit_depth_chroma_minus8 = 0 (bit_depth = 8)
    0x00, 0x00,                         // avg_frame_rate = 0 (unspecified)
    0x0F,                               // constant_frame_rate = 0, num_temporal_layers = 1, temporal_id_nested = 1, length_size = 4
    0x03,                               // numOfArrays = 3 (VPS, SPS, PPS)
    // VPS array
    0x20,                               // array_completeness = 0, nal_unit_type = 32 (VPS)
    0x00, 0x01,                         // numNalus = 1
    0x00, 0x17,                         // nalUnitLength = 23
    // VPS data (23 bytes)
    0x40, 0x01, 0x0C, 0x01, 0xFF, 0xFF, 0x04, 0x08, 0x00, 0x00, 0x03, 0x00,
    0x9F, 0xA8, 0x00, 0x00, 0x03, 0x00, 0x00, 0x3C, 0xBA, 0x02, 0x40,
    // SPS array
    0x21,                               // array_completeness = 0, nal_unit_type = 33 (SPS)
    0x00, 0x01,                         // numNalus = 1
    0x00, 0x27,                         // nalUnitLength = 39
    // SPS data (39 bytes)
    0x42, 0x01, 0x01, 0x04, 0x08, 0x00, 0x00, 0x03, 0x00, 0x9F, 0xA8, 0x00,
    0x00, 0x03, 0x00, 0x00, 0x3C, 0xA0, 0x0A, 0x08, 0x0F, 0x16, 0x5B, 0xA4,
    0xA4, 0xC2, 0xF0, 0x16, 0x80, 0x80, 0x00, 0x00, 0x03, 0x00, 0x80, 0x00,
    0x00, 0x0F, 0x04,
    // PPS array
    0x22,                               // array_completeness = 0, nal_unit_type = 34 (PPS)
    0x00, 0x01,                         // numNalus = 1
    0x00, 0x06,                         // nalUnitLength = 6
    // PPS data (6 bytes)
    0x44, 0x01, 0xC0, 0x71, 0x83, 0x12,
];

#[test]
fn test_parse_hvcc() {
    let hvcc = Hvcc::parse(HVCC_DATA).unwrap();

    assert_eq!(hvcc.configuration_version, 1);
    assert_eq!(hvcc.general_profile_space, 0);
    assert!(!hvcc.general_tier_flag);
    assert_eq!(hvcc.general_profile_idc, 4); // Main Intra Profile
    assert_eq!(hvcc.general_level_idc, 60); // Level 2.0
    assert_eq!(hvcc.chroma_format_idc, 1); // 4:2:0
    assert_eq!(hvcc.bit_depth_luma, 8);
    assert_eq!(hvcc.bit_depth_chroma, 8);
    assert_eq!(hvcc.length_size, 4);
    assert_eq!(hvcc.num_temporal_layers, 1);
    assert!(hvcc.temporal_id_nested);

    // Check NAL unit arrays
    assert_eq!(hvcc.nalu_arrays.len(), 3);

    // VPS array
    assert_eq!(hvcc.nalu_arrays[0].nal_unit_type, NalUnitType::VpsNut as u8);
    assert!(!hvcc.nalu_arrays[0].array_completeness);
    assert_eq!(hvcc.nalu_arrays[0].nalus.len(), 1);
    assert_eq!(hvcc.nalu_arrays[0].nalus[0].len(), 23);

    // Check VPS
    let parser = NalParser::<H265NalHeader>::new(None);
    let nal_unit = parser.parse(&hvcc.nalu_arrays[0].nalus[0]).unwrap();
    let vps = Vps::parse(nal_unit.payload()).unwrap();
    assert_eq!(vps.video_parameter_set_id, 0);

    // SPS array
    assert_eq!(hvcc.nalu_arrays[1].nal_unit_type, NalUnitType::SpsNut as u8);
    assert!(!hvcc.nalu_arrays[1].array_completeness);
    assert_eq!(hvcc.nalu_arrays[1].nalus.len(), 1);
    assert_eq!(hvcc.nalu_arrays[1].nalus[0].len(), 39);

    // Check SPS
    let parser = NalParser::<H265NalHeader>::new(None);
    let nal_unit = parser.parse(&hvcc.nalu_arrays[1].nalus[0]).unwrap();
    let sps = Sps::parse(nal_unit.payload()).unwrap();
    assert_eq!(sps.video_parameter_set_id, 0);
    assert_eq!(sps.pic_width_in_luma_samples, 320);
    assert_eq!(sps.pic_height_in_luma_samples, 240);

    // PPS array
    assert_eq!(hvcc.nalu_arrays[2].nal_unit_type, NalUnitType::PpsNut as u8);
    assert!(!hvcc.nalu_arrays[2].array_completeness);
    assert_eq!(hvcc.nalu_arrays[2].nalus.len(), 1);
    assert_eq!(hvcc.nalu_arrays[2].nalus[0].len(), 6);

    // Check PPS
    let parser = NalParser::<H265NalHeader>::new(None);
    let nal_unit = parser.parse(&hvcc.nalu_arrays[2].nalus[0]).unwrap();
    let pps = Pps::parse(nal_unit.payload()).unwrap();
    assert_eq!(pps.pic_parameter_set_id, 0);
}

#[test]
fn test_get_nalus() {
    let hvcc = Hvcc::parse(HVCC_DATA).unwrap();

    // Test VPS getter
    let vps_list: Vec<_> = hvcc.vps().collect();
    assert_eq!(vps_list.len(), 1);
    assert_eq!(vps_list[0].len(), 23);

    // Test SPS getter
    let sps_list: Vec<_> = hvcc.sps().collect();
    assert_eq!(sps_list.len(), 1);
    assert_eq!(sps_list[0].len(), 39);

    // Test PPS getter
    let pps_list: Vec<_> = hvcc.pps().collect();
    assert_eq!(pps_list.len(), 1);
    assert_eq!(pps_list[0].len(), 6);
}
