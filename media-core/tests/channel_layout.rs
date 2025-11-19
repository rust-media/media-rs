use media_core::audio::channel_layout::*;

#[test]
fn test_channel_layout_from_mask() {
    let layout = ChannelLayout::from_mask(ChannelMasks::Mono).unwrap();
    assert_eq!(layout, ChannelLayout::MONO);
    assert_eq!(layout.channels.get(), 1);

    let layout = ChannelLayout::from_mask(ChannelMasks::Stereo).unwrap();
    assert_eq!(layout, ChannelLayout::STEREO);
    assert_eq!(layout.channels.get(), 2);

    let layout = ChannelLayout::from_mask(ChannelMasks::Surround_5_1).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_5_1);
    assert_eq!(layout.channels.get(), 6);

    let layout = ChannelLayout::from_mask(ChannelMasks::Surround_7_1).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_7_1);
    assert_eq!(layout.channels.get(), 8);

    let layout = ChannelLayout::from_mask(ChannelMasks::Surround_9_1_4_BACK).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_9_1_4_BACK);
    assert_eq!(layout.channels.get(), 14);
}

#[test]
fn test_channel_layout_default() {
    let layout = ChannelLayout::default_from_channels(1).unwrap();
    assert_eq!(layout, ChannelLayout::MONO);
    assert_eq!(layout.channels.get(), 1);

    let layout = ChannelLayout::default_from_channels(2).unwrap();
    assert_eq!(layout, ChannelLayout::STEREO);
    assert_eq!(layout.channels.get(), 2);

    let layout = ChannelLayout::default_from_channels(3).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_2_1);
    assert_eq!(layout.channels.get(), 3);

    let layout = ChannelLayout::default_from_channels(4).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_4_0);
    assert_eq!(layout.channels.get(), 4);

    let layout = ChannelLayout::default_from_channels(5).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_5_0_BACK);
    assert_eq!(layout.channels.get(), 5);

    let layout = ChannelLayout::default_from_channels(6).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_5_1_BACK);
    assert_eq!(layout.channels.get(), 6);

    let layout = ChannelLayout::default_from_channels(7).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_6_1);
    assert_eq!(layout.channels.get(), 7);

    let layout = ChannelLayout::default_from_channels(8).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_7_1);
    assert_eq!(layout.channels.get(), 8);

    let layout = ChannelLayout::default_from_channels(10).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_5_1_4_BACK);
    assert_eq!(layout.channels.get(), 10);

    let layout = ChannelLayout::default_from_channels(12).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_7_1_4_BACK);
    assert_eq!(layout.channels.get(), 12);

    let layout = ChannelLayout::default_from_channels(14).unwrap();
    assert_eq!(layout, ChannelLayout::SURROUND_9_1_4_BACK);
    assert_eq!(layout.channels.get(), 14);

    assert!(ChannelLayout::default_from_channels(0).is_err());
}

#[test]
fn test_channel_layout_subset() {
    assert!(ChannelLayout::STEREO.subset(ChannelMasks::Mono).bits() == 0);
    assert!(ChannelLayout::SURROUND_3_0.subset(ChannelMasks::Mono | ChannelMasks::Stereo).bits() != 0);
    assert!(ChannelLayout::SURROUND_5_1.subset(ChannelMasks::Surround).bits() != 0);
    assert!(ChannelLayout::SURROUND_5_1.subset(ChannelMasks::LowFrequency).bits() != 0);
    assert!(ChannelLayout::SURROUND_7_1.subset(ChannelMasks::Surround_5_1).bits() != 0);
}
