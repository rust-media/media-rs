use media_core::{audio::*, frame::*};

#[test]
fn test_sample_format() {
    assert_eq!(SampleFormat::U8.bits(), 8);
    assert_eq!(SampleFormat::S16.bits(), 16);
    assert_eq!(SampleFormat::S32.bits(), 32);
    assert_eq!(SampleFormat::F32.bits(), 32);
    assert_eq!(SampleFormat::F64.bits(), 64);

    assert_eq!(SampleFormat::U8.bytes(), 1);
    assert_eq!(SampleFormat::S16.bytes(), 2);
    assert_eq!(SampleFormat::S32.bytes(), 4);
    assert_eq!(SampleFormat::F32.bytes(), 4);
    assert_eq!(SampleFormat::F64.bytes(), 8);

    assert!(!SampleFormat::U8.is_planar());
    assert!(!SampleFormat::S16.is_planar());
    assert!(!SampleFormat::S32.is_planar());
    assert!(!SampleFormat::F32.is_planar());
    assert!(!SampleFormat::F64.is_planar());
    assert!(SampleFormat::U8P.is_planar());
    assert!(SampleFormat::S16P.is_planar());
    assert!(SampleFormat::S32P.is_planar());
    assert!(SampleFormat::F32P.is_planar());
    assert!(SampleFormat::F64P.is_planar());
}

#[test]
fn test_channel_layout_from_mask() {
    let layout = ChannelLayout::from_mask(ChannelMasks::Stereo).unwrap();
    assert_eq!(layout.channels.get(), 2);
    assert_eq!(layout.order, ChannelOrder::Native);

    let layout = ChannelLayout::from_mask(ChannelMasks::Surround_5_1).unwrap();
    assert_eq!(layout.channels.get(), 6);
    assert_eq!(layout.order, ChannelOrder::Native);
}

#[test]
fn test_channel_layout_default() {
    let mono = ChannelLayout::default_from_channels(1).unwrap();
    assert_eq!(mono.channels.get(), 1);
    assert_eq!(mono.spec, ChannelLayoutSpec::Mask(ChannelMasks::Mono));

    let stereo = ChannelLayout::default_from_channels(2).unwrap();
    assert_eq!(stereo.channels.get(), 2);
    assert_eq!(stereo.spec, ChannelLayoutSpec::Mask(ChannelMasks::Stereo));

    assert!(ChannelLayout::default_from_channels(0).is_err());
}

#[test]
fn test_audio_frame_descriptor() {
    let desc = AudioFrameDescriptor::try_new(SampleFormat::F32, 2, 1024, 44100).unwrap();

    assert_eq!(desc.format, SampleFormat::F32);
    assert_eq!(desc.channels().get(), 2);
    assert_eq!(desc.samples.get(), 1024);
    assert_eq!(desc.sample_rate.get(), 44100);
}

#[test]
fn test_audio_frame() {
    let frame = Frame::audio_creator().create(SampleFormat::S16, 2, 1024, 44100);

    assert!(frame.is_ok());

    let frame = frame.unwrap();

    let desc = frame.audio_descriptor().unwrap();
    assert_eq!(desc.format, SampleFormat::S16);
    assert_eq!(desc.channels().get(), 2);
    assert_eq!(desc.samples.get(), 1024);
    assert_eq!(desc.sample_rate.get(), 44100);
}

#[test]
fn test_audio_frame_from_buffer() {
    let sample_format = SampleFormat::F32;
    let data_size = 2 * 1024 * sample_format.bytes() as u32;
    let buffer = vec![0u8; data_size as usize];

    let frame = Frame::audio_creator().create_from_buffer(sample_format, 2, 1024, 44100, buffer.as_slice());

    assert!(frame.is_ok());

    let frame = frame.unwrap();

    let desc = frame.audio_descriptor().unwrap();
    assert_eq!(desc.format, SampleFormat::F32);
    assert_eq!(desc.channels().get(), 2);
    assert_eq!(desc.samples.get(), 1024);
    assert_eq!(desc.sample_rate.get(), 44100);
}
