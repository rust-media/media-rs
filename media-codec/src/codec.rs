use std::{collections::HashMap, num::NonZeroU32};

use media_base::{
    audio::{ChannelLayout, SampleFormat},
    error::Error,
    frame::Frame,
    video::{ChromaLocation, ColorMatrix, ColorPrimaries, ColorRange, ColorTransferCharacteristics, PixelFormat},
    MediaType, Result,
};
use num_rational::Rational64;

use crate::packet::Packet;

#[repr(u16)]
enum AudioCodecID {
    AAC = 1,
    Opus,
}

#[repr(u16)]
enum VideoCodecID {
    H264 = 1,
    HEVC,
    VP8,
    VP9,
    AV1,
}

macro_rules! codec_id {
    ($media_type:ident, $id_enum:ident, $id:ident) => {
        ((MediaType::$media_type as u32) << 16) | ($id_enum::$id as u32)
    };
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecID {
    // Audio codecs
    AAC  = codec_id!(Audio, AudioCodecID, AAC),
    Opus = codec_id!(Audio, AudioCodecID, Opus),
    // Video codecs
    H264 = codec_id!(Video, VideoCodecID, H264),
    HEVC = codec_id!(Video, VideoCodecID, HEVC),
    VP8  = codec_id!(Video, VideoCodecID, VP8),
    VP9  = codec_id!(Video, VideoCodecID, VP9),
    AV1  = codec_id!(Video, VideoCodecID, AV1),
}

impl CodecID {
    pub fn media_type(&self) -> MediaType {
        match ((*self as u32) >> 16) as u16 {
            x if x == MediaType::Audio as u16 => MediaType::Audio,
            x if x == MediaType::Video as u16 => MediaType::Video,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AudioCodecParameters {
    pub format: Option<SampleFormat>,
    pub samples: Option<NonZeroU32>,
    pub sample_rate: Option<NonZeroU32>,
    pub channel_layout: Option<ChannelLayout>,
}

#[derive(Clone, Debug, Default)]
pub struct VideoCodecParameters {
    pub format: Option<PixelFormat>,
    pub width: Option<NonZeroU32>,
    pub height: Option<NonZeroU32>,
    pub color_range: Option<ColorRange>,
    pub color_matrix: Option<ColorMatrix>,
    pub color_primaries: Option<ColorPrimaries>,
    pub color_transfer_characteristics: Option<ColorTransferCharacteristics>,
    pub chroma_location: Option<ChromaLocation>,
    pub frame_rate: Option<Rational64>,
}

#[derive(Clone, Debug)]
pub enum CodecSpecificParameters {
    Audio(AudioCodecParameters),
    Video(VideoCodecParameters),
}

impl CodecSpecificParameters {
    pub fn media_type(&self) -> MediaType {
        match self {
            CodecSpecificParameters::Audio(_) => MediaType::Audio,
            CodecSpecificParameters::Video(_) => MediaType::Video,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CodecParameters {
    pub id: Option<CodecID>,
    pub specific: Option<CodecSpecificParameters>,
}
