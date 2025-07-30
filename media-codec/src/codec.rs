use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, LazyLock, RwLock},
};

use media_base::{
    audio::{ChannelLayout, SampleFormat},
    error::Error,
    video::{ChromaLocation, ColorMatrix, ColorPrimaries, ColorRange, ColorTransferCharacteristics, PixelFormat},
    MediaType, Result,
};
use num_rational::Rational64;

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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

pub trait CodecBuilder: Send + Sync {
    fn id(&self) -> CodecID;
    fn name(&self) -> &'static str;
}

pub(crate) struct CodecList<T> {
    pub(crate) codecs: HashMap<CodecID, Vec<T>>,
}

pub(crate) type LazyCodecList<T> = LazyLock<RwLock<CodecList<Arc<T>>>>;

pub(crate) fn register_codec<T>(codec_list: &LazyCodecList<T>, builder: Arc<T>, default: bool) -> Result<()>
where
    T: CodecBuilder + ?Sized,
{
    let mut codec_list = codec_list.write().map_err(|err| Error::Invalid(err.to_string()))?;
    let entry = codec_list.codecs.entry(builder.id()).or_default();

    if default {
        entry.insert(0, builder);
    } else {
        entry.push(builder);
    }

    Ok(())
}

pub(crate) fn find_codec<T>(codec_list: &LazyCodecList<T>, codec_id: CodecID) -> Result<Arc<T>>
where
    T: CodecBuilder + ?Sized,
{
    let codec_list = codec_list.read().map_err(|err| Error::Invalid(err.to_string()))?;

    if let Some(builders) = codec_list.codecs.get(&codec_id) {
        if let Some(builder) = builders.first() {
            return Ok(builder.clone());
        }
    }

    Err(Error::NotFound(format!("codec: {:?}", codec_id)))
}

pub(crate) fn find_codec_by_name<T>(codec_list: &LazyCodecList<T>, name: &str) -> Result<Arc<T>>
where
    T: CodecBuilder + ?Sized,
{
    let codec_list = codec_list.read().map_err(|err| Error::Invalid(err.to_string()))?;

    for builders in codec_list.codecs.values() {
        for builder in builders {
            if builder.name() == name {
                return Ok(builder.clone());
            }
        }
    }

    Err(Error::NotFound(format!("codec with name: {}", name)))
}
