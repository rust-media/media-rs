use media_codec_h264::{H264NalHeader, NalUnitType};
use media_codec_nal::NalHeader;

#[test]
fn test_parse_idr_header() {
    // IDR NAL unit with nal_ref_idc=3
    let data = &[0x65]; // 0b0110_0101 = F:0 NRI:3 Type:5
    let header = H264NalHeader::parse(data).unwrap();
    assert!(!header.forbidden_zero_bit);
    assert_eq!(header.nal_ref_idc, 3);
    assert_eq!(header.nal_unit_type(), 5);
    assert!(header.is_idr());
    assert!(header.is_vcl());
}

#[test]
fn test_parse_sps_header() {
    // SPS NAL unit with nal_ref_idc=3
    let data = &[0x67]; // 0b0110_0111 = F:0 NRI:3 Type:7
    let header = H264NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type(), 7);
    assert!(header.is_parameter_set());
    assert!(!header.is_vcl());
}

#[test]
fn test_parse_pps_header() {
    // PPS NAL unit with nal_ref_idc=3
    let data = &[0x68]; // 0b0110_1000 = F:0 NRI:3 Type:8
    let header = H264NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type(), 8);
    assert!(header.is_parameter_set());
}

#[test]
fn test_parse_non_idr_slice() {
    // Non-IDR slice with nal_ref_idc=2
    let data = &[0x41]; // 0b0100_0001 = F:0 NRI:2 Type:1
    let header = H264NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type(), 1);
    assert!(header.is_vcl());
    assert!(!header.is_idr());
}

#[test]
fn test_forbidden_bit_error() {
    // Forbidden bit is set
    let data = &[0x85]; // 0b1000_0101 = F:1 NRI:0 Type:5
    let result = H264NalHeader::parse(data);
    assert!(result.is_err());
}

#[test]
fn test_to_byte() {
    let header = H264NalHeader::new(false, 3, NalUnitType::from_u8(5).unwrap());
    assert_eq!(header.to_byte(), 0x65);
}

#[test]
fn test_unit_type_enum() {
    assert_eq!(NalUnitType::from_u8(5).unwrap(), NalUnitType::SliceIdr);
    assert_eq!(NalUnitType::from_u8(7).unwrap(), NalUnitType::Sps);
    assert_eq!(NalUnitType::from_u8(8).unwrap(), NalUnitType::Pps);
}
