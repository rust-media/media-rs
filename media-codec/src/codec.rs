use std::{
    any::Any,
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, LazyLock, RwLock},
};

#[cfg(feature = "audio")]
use media_core::audio::{ChannelLayout, SampleFormat};
#[cfg(feature = "video")]
use media_core::video::{ChromaLocation, ColorMatrix, ColorPrimaries, ColorRange, ColorTransferCharacteristics, PixelFormat};
use media_core::{error::Error, variant::Variant, MediaType, Result};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecType {
    Decoder,
    Encoder,
}

pub trait CodecParameters: Clone + Send + Sync + 'static {
    fn media_type() -> MediaType;
    fn codec_type() -> CodecType;
}

pub trait CodecConfiguration: Clone + Send + Sync + 'static {
    type Parameters: CodecParameters;

    fn media_type() -> MediaType {
        Self::Parameters::media_type()
    }
    fn codec_type() -> CodecType {
        Self::Parameters::codec_type()
    }
    fn from_parameters(parameters: &Self::Parameters) -> Result<Self>;
    fn configure(&mut self, parameters: &Self::Parameters) -> Result<()>;
    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()>;
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioParameters {
    pub format: Option<SampleFormat>,
    pub samples: Option<NonZeroU32>,
    pub sample_rate: Option<NonZeroU32>,
    pub channel_layout: Option<ChannelLayout>,
}

#[cfg(feature = "audio")]
impl AudioParameters {
    pub(crate) fn update(&mut self, other: &AudioParameters) {
        self.format = other.format.or(self.format);
        self.samples = other.samples.or(self.samples);
        self.sample_rate = other.sample_rate.or(self.sample_rate);
        if other.channel_layout.is_some() {
            self.channel_layout = other.channel_layout.clone();
        }
    }

    pub(crate) fn update_with_option(&mut self, key: &str, value: &Variant) {
        match key {
            "sample_format" => self.format = value.get_uint32().and_then(|fmt| SampleFormat::try_from(fmt as usize).ok()),
            "samples" => self.samples = value.get_uint32().and_then(NonZeroU32::new),
            "sample_rate" => self.sample_rate = value.get_uint32().and_then(NonZeroU32::new),
            "channels" => self.channel_layout = value.get_uint8().and_then(|c| ChannelLayout::default_from_channels(c).ok()),
            _ => {}
        }
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug, Default)]
pub struct VideoParameters {
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

#[cfg(feature = "video")]
impl VideoParameters {
    pub(crate) fn update(&mut self, other: &VideoParameters) {
        self.format = other.format.or(self.format);
        self.width = other.width.or(self.width);
        self.height = other.height.or(self.height);
        self.color_range = other.color_range.or(self.color_range);
        self.color_matrix = other.color_matrix.or(self.color_matrix);
        self.color_primaries = other.color_primaries.or(self.color_primaries);
        self.color_transfer_characteristics = other.color_transfer_characteristics.or(self.color_transfer_characteristics);
        self.chroma_location = other.chroma_location.or(self.chroma_location);
        self.frame_rate = other.frame_rate.or(self.frame_rate);
    }

    pub(crate) fn update_with_option(&mut self, key: &str, value: &Variant) {
        match key {
            "pixel_format" => self.format = value.get_uint32().and_then(|f| PixelFormat::try_from(f as usize).ok()),
            "width" => self.width = value.get_uint32().and_then(NonZeroU32::new),
            "height" => self.height = value.get_uint32().and_then(NonZeroU32::new),
            "color_range" => self.color_range = value.get_uint32().map(|v| ColorRange::from(v as usize)),
            "color_matrix" => self.color_matrix = value.get_uint32().and_then(|v| ColorMatrix::try_from(v as usize).ok()),
            "color_primaries" => self.color_primaries = value.get_uint32().and_then(|v| ColorPrimaries::try_from(v as usize).ok()),
            "color_transfer_characteristics" => {
                self.color_transfer_characteristics = value.get_uint32().and_then(|v| ColorTransferCharacteristics::try_from(v as usize).ok())
            }
            "chroma_location" => self.chroma_location = value.get_uint32().map(|v| ChromaLocation::from(v as usize)),
            _ => {}
        }
    }
}

pub trait CodecInfomation {
    fn id(&self) -> CodecID;
    fn name(&self) -> &'static str;
}

pub trait Codec<T: CodecConfiguration>: CodecInfomation {
    fn configure(&mut self, parameters: Option<&T::Parameters>, options: Option<&Variant>) -> Result<()>;
    fn set_option(&mut self, key: &str, value: &Variant) -> Result<()>;
}

pub trait CodecBuilder<T: CodecConfiguration>: Any + Send + Sync {
    fn id(&self) -> CodecID;
    fn name(&self) -> &'static str;
}

pub(crate) struct CodecList<T: CodecConfiguration> {
    pub(crate) codecs: HashMap<CodecID, Vec<Arc<dyn CodecBuilder<T>>>>,
}

pub(crate) type LazyCodecList<T> = LazyLock<RwLock<CodecList<T>>>;

pub(crate) fn register_codec<T>(codec_list: &LazyCodecList<T>, builder: Arc<dyn CodecBuilder<T>>, default: bool) -> Result<()>
where
    T: CodecConfiguration,
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

pub(crate) fn find_codec<T>(codec_list: &LazyCodecList<T>, id: CodecID) -> Result<Arc<dyn CodecBuilder<T>>>
where
    T: CodecConfiguration,
{
    let codec_list = codec_list.read().map_err(|err| Error::Invalid(err.to_string()))?;

    if let Some(builders) = codec_list.codecs.get(&id) {
        if let Some(builder) = builders.first() {
            return Ok(builder.clone());
        }
    }

    Err(Error::NotFound(format!("codec: {:?}", id)))
}

pub(crate) fn find_codec_by_name<T>(codec_list: &LazyCodecList<T>, name: &str) -> Result<Arc<dyn CodecBuilder<T>>>
where
    T: CodecConfiguration,
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
