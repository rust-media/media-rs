use media_codec_h265::{
    nal::NalUnitType,
    pps::Pps,
    slice::{SliceSegmentHeader, SliceType},
    sps::Sps,
};

/// SPS data
#[rustfmt::skip]
const SPS_DATA: &[u8] = &[
    0x01,                               // vps_id = 0, max_sub_layers_minus1 = 0, temporal_id_nesting = 1
    0x04,                               // profile_space = 0, tier = 0, profile_idc = 4 (Main 4:4:4)
    0x08, 0x00, 0x00, 0x00,             // profile_compatibility_flags
    0x00, 0x9E, 0x28, 0x00, 0x00, 0x00, // constraint_flags (48 bits)
    0x1E,                               // level_idc = 0x1E (30 = Level 1.0)
    // exp-golomb encoded fields (sps_id, chroma, width, height, etc.)
    0x90, 0x04, 0x10, 0x20, 0xB2, 0xDD, 0x49, 0x26,
    0x17, 0x80, 0xB7, 0x02, 0x02, 0x00, 0x04, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x20,
];

/// PPS data
#[rustfmt::skip]
const PPS_DATA: &[u8] = &[
    0xC1, 0x72, 0x86, 0x0C, 0x02, 0x24,
];

/// IDR slice header data
#[rustfmt::skip]
const IDR_SLICE_HEADER: &[u8] = &[
    0xAD,                               // first_slice = 1, no_output = 0, pps_id = 0, slice_type = I(2)
    0xE0, 0x1F, 0x04, 0x84,             // qp_delta continuation, five_minus_max_merge_cand, ...
    0x7F, 0x82, 0xFD, 0xFC,             // slice data (CABAC)
    0xD3, 0xFF, 0xA9, 0x70,             // slice data
    0x92, 0xB3, 0x65, 0x59,             // slice data
    0x41, 0xDF, 0x15, 0x95,             // slice data
    0x79, 0xE5, 0x45, 0xEB,             // slice data
    0x1B, 0x68, 0x0D, 0xB3, 0xFA,       // slice data + rbsp_trailing_bits
];

#[test]
fn test_parse_idr_slice_header() {
    let sps = Sps::parse(SPS_DATA).unwrap();
    let pps = Pps::parse(PPS_DATA).unwrap();

    let header = SliceSegmentHeader::parse(IDR_SLICE_HEADER, NalUnitType::IdrNLp, &sps, &pps).unwrap();

    assert!(header.first_slice_segment_in_pic_flag);
    assert_eq!(header.slice_pic_parameter_set_id, 0);
    assert_eq!(header.slice_type, SliceType::I);
}

#[test]
fn test_slice_type() {
    assert_eq!(SliceType::from_u8(0), Some(SliceType::B));
    assert_eq!(SliceType::from_u8(1), Some(SliceType::P));
    assert_eq!(SliceType::from_u8(2), Some(SliceType::I));
    assert_eq!(SliceType::from_u8(3), None);

    assert!(SliceType::I.is_intra());
    assert!(!SliceType::I.is_inter());
    assert!(!SliceType::P.is_intra());
    assert!(SliceType::P.is_inter());
    assert!(SliceType::P.is_p());
    assert!(!SliceType::P.is_b());
    assert!(SliceType::B.is_b());
    assert!(!SliceType::B.is_p());
}
