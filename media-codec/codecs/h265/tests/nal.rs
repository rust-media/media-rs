use media_codec_h265::{H265NalHeader, NalUnitType};
use media_codec_nal::NalHeader;

#[test]
fn test_parse_idr_header() {
    // IDR_W_RADL NAL unit: type=19, layer_id=0, temporal_id=1
    // byte0 = 0b0_100110_0 = 0x26 (F:0, Type:19, LayerId[5]:0)
    // byte1 = 0b00000_001 = 0x01 (LayerId[0-4]:0, TID:1)
    let data = &[0x26, 0x01];
    let header = H265NalHeader::parse(data).unwrap();
    assert!(!header.forbidden_zero_bit);
    assert_eq!(header.nal_unit_type, NalUnitType::IdrWRadl);
    assert_eq!(header.nuh_layer_id, 0);
    assert_eq!(header.nuh_temporal_id_plus1, 1);
    assert!(header.is_idr());
    assert!(header.is_vcl());
    assert!(header.is_irap());
}

#[test]
fn test_parse_vps_header() {
    // VPS NAL unit: type=32, layer_id=0, temporal_id=1
    // byte0 = 0b0_100000_0 = 0x40 (F:0, Type:32, LayerId[5]:0)
    // byte1 = 0b00000_001 = 0x01 (LayerId[0-4]:0, TID:1)
    let data = &[0x40, 0x01];
    let header = H265NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type, NalUnitType::VpsNut);
    assert!(header.is_parameter_set());
    assert!(!header.is_vcl());
}

#[test]
fn test_parse_sps_header() {
    // SPS NAL unit: type=33, layer_id=0, temporal_id=1
    // byte0 = 0b0_100001_0 = 0x42 (F:0, Type:33, LayerId[5]:0)
    // byte1 = 0b00000_001 = 0x01 (LayerId[0-4]:0, TID:1)
    let data = &[0x42, 0x01];
    let header = H265NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type, NalUnitType::SpsNut);
    assert!(header.is_parameter_set());
}

#[test]
fn test_parse_pps_header() {
    // PPS NAL unit: type=34, layer_id=0, temporal_id=1
    // byte0 = 0b0_100010_0 = 0x44 (F:0, Type:34, LayerId[5]:0)
    // byte1 = 0b00000_001 = 0x01 (LayerId[0-4]:0, TID:1)
    let data = &[0x44, 0x01];
    let header = H265NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type, NalUnitType::PpsNut);
    assert!(header.is_parameter_set());
}

#[test]
fn test_parse_trailing_picture() {
    // TRAIL_R NAL unit: type=1, layer_id=0, temporal_id=1
    // byte0 = 0b0_000001_0 = 0x02 (F:0, Type:1, LayerId[5]:0)
    // byte1 = 0b00000_001 = 0x01 (LayerId[0-4]:0, TID:1)
    let data = &[0x02, 0x01];
    let header = H265NalHeader::parse(data).unwrap();
    assert_eq!(header.nal_unit_type, NalUnitType::TrailR);
    assert!(header.is_vcl());
    assert!(!header.is_idr());
    assert!(!header.is_irap());
}

#[test]
fn test_forbidden_bit_error() {
    // Forbidden bit is set
    let data = &[0x80, 0x01];
    let result = H265NalHeader::parse(data);
    assert!(result.is_err());
}

#[test]
fn test_temporal_id_zero_error() {
    // Temporal ID is 0 (invalid)
    let data = &[0x02, 0x00];
    let result = H265NalHeader::parse(data);
    assert!(result.is_err());
}

#[test]
fn test_to_bytes() {
    let header = H265NalHeader::new(false, NalUnitType::IdrWRadl, 0, 1);
    assert_eq!(header.to_bytes(), [0x26, 0x01]);
}

#[test]
fn test_unit_type_enum() {
    assert_eq!(NalUnitType::from_u8(19), Some(NalUnitType::IdrWRadl));
    assert_eq!(NalUnitType::from_u8(32), Some(NalUnitType::VpsNut));
    assert_eq!(NalUnitType::from_u8(33), Some(NalUnitType::SpsNut));
    assert_eq!(NalUnitType::from_u8(34), Some(NalUnitType::PpsNut));
}
