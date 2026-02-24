use media_codec_h264::{avcc::Avcc, pps::Pps, sps::Sps, H264NalHeader};
use media_codec_nal::NalParser;

// Sample AVCC data for testing
#[rustfmt::skip]
const AVCC_DATA: &[u8] = &[
    0x01,       // configurationVersion
    0x64,       // AVCProfileIndication (100 = High Profile)
    0x00,       // profile_compatibility
    0x2A,       // AVCLevelIndication (42 = Level 4.2)
    0x03,       // lengthSizeMinusOne = 3 (length_size = 4)
    0x01,       // numOfSequenceParameterSets = 1
    0x00, 0x26, // SPS length (38 bytes)
    // SPS data
    0x27, 0x64, 0x00, 0x2A, 0xAC, 0x24, 0x8C, 0x07, 0x80, 0x22, 0x7E, 0x5C,
    0x04, 0x40, 0x00, 0x00, 0x03, 0x00, 0x40, 0x00, 0x00, 0x1E, 0x38, 0xA0,
    0x00, 0x0B, 0x71, 0xB0, 0x00, 0x16, 0xE3, 0x7B, 0xDE, 0xE0, 0x3E, 0x11,
    0x08, 0xA7,
    0x01,       // numOfPictureParameterSets = 1
    0x00, 0x04, // PPS length (4 bytes)
    // PPS data
    0x28, 0xDE, 0xBC, 0xB0,
    // Extended configuration
    0xFD,       // chroma_format = 1 (4:2:0)
    0xF8,       // bit_depth_luma_minus8 = 0 (bit_depth = 8)
    0xF8,       // bit_depth_chroma_minus8 = 0 (bit_depth = 8)
    0x00,       // numOfSequenceParameterSetExt = 0
];

#[test]
fn test_parse_avcc() {
    let avcc = Avcc::parse(AVCC_DATA).unwrap();

    assert_eq!(avcc.configuration_version, 1);
    assert_eq!(avcc.avc_profile_indication, 100);
    assert_eq!(avcc.profile_compatibility, 0);
    assert_eq!(avcc.avc_level_indication, 42);
    assert_eq!(avcc.length_size, 4);
    assert_eq!(avcc.sequence_parameter_sets.len(), 1);
    assert_eq!(avcc.picture_parameter_sets.len(), 1);
    assert_eq!(avcc.sequence_parameter_sets[0].len(), 38);
    assert_eq!(avcc.picture_parameter_sets[0].len(), 4);

    // Check SPS
    let parser = NalParser::<H264NalHeader>::new(None);
    let nal_unit = parser.parse(&avcc.sequence_parameter_sets[0]).unwrap();
    let sps = Sps::parse(nal_unit.rbsp()).unwrap();
    assert_eq!(sps.profile_idc, 100);

    // Check PPS
    let nal_unit = parser.parse(&avcc.picture_parameter_sets[0]).unwrap();
    let pps = Pps::parse(nal_unit.rbsp()).unwrap();
    assert_eq!(pps.pic_parameter_set_id, 0);

    // Check extended configuration
    assert!(avcc.ext.is_some());
    let ext = avcc.ext.unwrap();
    assert_eq!(ext.chroma_format, 1);
    assert_eq!(ext.bit_depth_luma, 8);
    assert_eq!(ext.bit_depth_chroma, 8);
    assert!(ext.sequence_parameter_sets_ext.is_empty());
}
