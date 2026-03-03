use media_codec_h264::{
    nal::NalUnitType,
    pps::Pps,
    slice::{SliceHeader, SliceType},
    sps::Sps,
};

/// SPS data
#[rustfmt::skip]
const SPS_DATA: &[u8] = &[
    0x42, // profile_idc = 66
    0xC0, // flag0 = 1, flag1 = 1, others = 0, reserved = 0
    0x0A, // level_idc = 10
    0xDA, // 1101 1010
    0x0B, // 0000 1011
    0x13, // 0001 0011
    0x90, // 1001 0000
];

/// PPS data
#[rustfmt::skip]
const PPS_DATA: &[u8] = &[
    0xCE, // 1100 1110
    0x38, // 0011 1000
];

/// IDR slice header data
#[rustfmt::skip]
const IDR_SLICE_HEADER: &[u8] = &[
    0xB8, // first_mb = 0, slice_type = 2, pps_id = 0
    0x40, // frame_num = 0, idr_pic_id = 0, poc_lsb = 0
    0x80, // slice_qp_delta = 0
];

#[test]
fn test_parse_idr_slice_header() {
    let sps = Sps::parse(SPS_DATA).unwrap();
    let pps = Pps::parse_with_sps(PPS_DATA, &sps).unwrap();

    let header = SliceHeader::parse(
        IDR_SLICE_HEADER,
        NalUnitType::SliceIdr,
        3, // nal_ref_idc (reference picture)
        &sps,
        &pps,
    )
    .unwrap();

    assert_eq!(header.first_mb_in_slice, 0);
    assert_eq!(header.slice_type, SliceType::I);
    assert_eq!(header.pic_parameter_set_id, 0);
    assert!(header.is_first_mb_in_pic());
    assert!(header.is_frame());
    assert!(!header.is_field_pic());
}

#[test]
fn test_slice_type_from_u8() {
    assert_eq!(SliceType::from_u8(0), Some(SliceType::P));
    assert_eq!(SliceType::from_u8(1), Some(SliceType::B));
    assert_eq!(SliceType::from_u8(2), Some(SliceType::I));
    assert_eq!(SliceType::from_u8(3), Some(SliceType::SP));
    assert_eq!(SliceType::from_u8(4), Some(SliceType::SI));
    // Values 5-9 map to same types (all slices in picture have same type)
    assert_eq!(SliceType::from_u8(5), Some(SliceType::P));
    assert_eq!(SliceType::from_u8(6), Some(SliceType::B));
    assert_eq!(SliceType::from_u8(7), Some(SliceType::I));
    assert_eq!(SliceType::from_u8(8), Some(SliceType::SP));
    assert_eq!(SliceType::from_u8(9), Some(SliceType::SI));
}

#[test]
fn test_slice_type_properties() {
    // Intra slices
    assert!(SliceType::I.is_intra());
    assert!(SliceType::SI.is_intra());

    // Inter slices
    assert!(!SliceType::P.is_intra());
    assert!(!SliceType::B.is_intra());
    assert!(!SliceType::SP.is_intra());
}
