use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, LazyLock, RwLock},
};

#[cfg(feature = "audio")]
use media_core::{
    audio::{ChannelLayout, SampleFormat},
};

#[cfg(feature = "video")]
use media_core::{
    video::{ChromaLocation, ColorMatrix, ColorPrimaries, ColorRange, ColorTransferCharacteristics, PixelFormat},
};

use media_core::{
    error::Error,
    variant::Variant,
    MediaType, Result,
};
#[cfg(feature = "video")]
use num_rational::Rational64;


#[cfg(feature = "audio")]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[repr(u16)]
enum AudioCodecID {
    AAC = 1,
    Opus,
}


#[cfg(feature = "video")]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
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
    #[cfg(feature = "audio")]
    AAC  = codec_id!(Audio, AudioCodecID, AAC),
    #[cfg(feature = "audio")]
    Opus = codec_id!(Audio, AudioCodecID, Opus),
    // Video codecs
    #[cfg(feature = "video")]
    H264 = codec_id!(Video, VideoCodecID, H264),
    #[cfg(feature = "video")]
    HEVC = codec_id!(Video, VideoCodecID, HEVC),
    #[cfg(feature = "video")]
    VP8  = codec_id!(Video, VideoCodecID, VP8),
    #[cfg(feature = "video")]
    VP9  = codec_id!(Video, VideoCodecID, VP9),
    #[cfg(feature = "video")]
    AV1  = codec_id!(Video, VideoCodecID, AV1),
}

impl CodecID {
    pub fn media_type(&self) -> MediaType {
        match ((*self as u32) >> 16) as u16 {
            #[cfg(feature = "audio")]
            x if x == MediaType::Audio as u16 => MediaType::Audio,
            #[cfg(feature = "video")]
            x if x == MediaType::Video as u16 => MediaType::Video,
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioCodecParameters {
    pub format: Option<SampleFormat>,
    pub samples: Option<NonZeroU32>,
    pub sample_rate: Option<NonZeroU32>,
    pub channel_layout: Option<ChannelLayout>,
}

#[cfg(feature = "video")]
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
    #[cfg(feature = "audio")]
    Audio(AudioCodecParameters),
    #[cfg(feature = "video")]
    Video(VideoCodecParameters),
}

#[cfg(feature = "audio")]
impl From<AudioCodecParameters> for CodecSpecificParameters {
    fn from(params: AudioCodecParameters) -> Self {
        CodecSpecificParameters::Audio(params)
    }
}

#[cfg(feature = "video")]
impl From<VideoCodecParameters> for CodecSpecificParameters {
    fn from(params: VideoCodecParameters) -> Self {
        CodecSpecificParameters::Video(params)
    }
}

impl CodecSpecificParameters {
    pub fn media_type(&self) -> MediaType {
        match self {
            #[cfg(feature = "audio")]
            CodecSpecificParameters::Audio(_) => MediaType::Audio,
            #[cfg(feature = "video")]
            CodecSpecificParameters::Video(_) => MediaType::Video,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CodecParameters {
    pub id: Option<CodecID>,
    pub specific: Option<CodecSpecificParameters>,
}

impl CodecParameters {
    pub fn new<T>(id: CodecID, params: T) -> Self
    where
        T: Into<CodecSpecificParameters>,
    {
        Self {
            id: Some(id),
            specific: Some(params.into()),
        }
    }

    #[cfg(feature = "audio")]
    #[allow(unreachable_patterns)]
    pub fn audio(&self) -> Option<&AudioCodecParameters> {
        self.specific.as_ref().and_then(|spec| match spec {
            CodecSpecificParameters::Audio(params) => Some(params),
            _ => None,
        })
    }

    #[cfg(feature = "video")]
    #[allow(unreachable_patterns)]
    pub fn video(&self) -> Option<&VideoCodecParameters> {
        self.specific.as_ref().and_then(|spec| match spec {
            CodecSpecificParameters::Video(params) => Some(params),
            _ => None,
        })
    }
}

pub trait Codec {
    fn configure(&mut self, parameters: Option<&CodecParameters>, options: Option<&Variant>) -> Result<()>;
    fn set_option(&mut self, key: &str, value: &Variant) -> Result<()>;
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

    Err(Error::NotFound(format!("codec: {}", name)))
}
