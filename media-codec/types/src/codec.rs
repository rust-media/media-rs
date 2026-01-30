use std::{
    any::Any,
    fmt::{self, Debug, Display, Formatter},
    num::NonZeroU32,
};

#[cfg(feature = "audio")]
use media_core::audio::{ChannelLayout, SampleFormat};
#[cfg(feature = "video")]
use media_core::rational::Rational64;
#[cfg(feature = "video")]
use media_core::video::{ChromaLocation, ColorMatrix, ColorPrimaries, ColorRange, ColorTransferCharacteristics, PixelFormat};
use media_core::{error::Error, invalid_param_error, variant::Variant, FrameDescriptorSpec, MediaType, Result};

use crate::{decoder::DecoderParameters, encoder::EncoderParameters};

macro_rules! codecs {
    (@impl $feature:literal, $media_type:ident, $id:expr, $name:ident) => {
        #[cfg(feature = $feature)]
        pub const $name: CodecID = CodecID(((MediaType::$media_type as u32) << 16) | $id);
    };

    (@impl $feature:literal, $media_type:ident, $id:expr, $name:ident, $($rest:ident),+) => {
        #[cfg(feature = $feature)]
        pub const $name: CodecID = CodecID(((MediaType::$media_type as u32) << 16) | $id);
        codecs!(@impl $feature, $media_type, $id + 1, $($rest),+);
    };
}

macro_rules! define_codecs {
    ($(
        #[cfg(feature = $feature:literal)]
        $media_type:ident: [$($name:ident),+ $(,)?]
    )+) => {
        impl CodecID {
            $(
                codecs!(@impl $feature, $media_type, 1, $($name),+);
            )+
        }

        impl CodecID {
            pub fn as_str(&self) -> Option<&'static str> {
                match *self {
                    $(
                        $(
                            #[cfg(feature = $feature)]
                            CodecID::$name => Some(stringify!($name)),
                        )+
                    )+
                    _ => None,
                }
            }
        }
    };
}

#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CodecID(u32);

define_codecs! {
    #[cfg(feature = "audio")]
    Audio: [
        MP1,
        MP2,
        MP3,
        AAC,
        AC3,
        EAC3,
        DTS,
        FLAC,
        ALAC,
        G723_1,
        G729,
        VORBIS,
        OPUS,
        WMA1,
        WMA2,
        WMAVOICE,
        WMAPRO,
        WMALOSSLESS,
    ]

    #[cfg(feature = "video")]
    Video: [
        MPEG1,
        MPEG2,
        MPEG4,
        MJPEG,
        H261,
        H263,
        H264,
        HEVC,
        VVC,
        VP8,
        VP9,
        AV1,
        RV10,
        RV20,
        RV30,
        RV40,
        RV60,
        FLV1,
        WMV1,
        WMV2,
        WMV3,
        VC1,
        AVS,
        CAVS,
        AVS2,
        AVS3,
        BMP,
        PNG,
        APNG,
        GIF,
        TIFF,
        WEBP,
        JPEGXL,
        JPEG2000,
        PRORES,
    ]
}

impl Debug for CodecID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(str) = self.as_str() {
            write!(f, "CodecID::{}", str)
        } else {
            write!(f, "CodecID(0x{:08X})", self.0)
        }
    }
}

impl Display for CodecID {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(str) = self.as_str() {
            f.write_str(str)
        } else {
            write!(f, "0x{:08X}", self.0)
        }
    }
}

impl CodecID {
    pub fn media_type(&self) -> MediaType {
        match ((self.0) >> 16) as u16 {
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

#[derive(Clone, Debug)]
pub struct CodecParameters {
    pub media: MediaParametersType,
    pub codec: CodecParametersType,
}

impl CodecParameters {
    pub fn new<M, C>(media_params: M, codec_params: C) -> Self
    where
        M: Into<MediaParametersType>,
        C: Into<CodecParametersType>,
    {
        Self {
            media: media_params.into(),
            codec: codec_params.into(),
        }
    }
}

pub trait CodecSpec: Clone + Send + Sync + 'static {
    type FrameDescriptor: FrameDescriptorSpec;

    fn media_type() -> MediaType;
    fn codec_type() -> CodecType;
    fn from_parameters(params: &CodecParameters) -> Result<Self>;
    fn configure(&mut self, params: &CodecParameters) -> Result<()>;
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

#[cfg(feature = "audio")]
#[allow(unreachable_patterns)]
impl TryFrom<&MediaParametersType> for AudioParameters {
    type Error = Error;

    fn try_from(params: &MediaParametersType) -> Result<Self> {
        match params {
            MediaParametersType::Audio(params) => Ok(params.clone()),
            _ => Err(invalid_param_error!(params)),
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

#[cfg(feature = "video")]
#[allow(unreachable_patterns)]
impl TryFrom<&MediaParametersType> for VideoParameters {
    type Error = Error;

    fn try_from(params: &MediaParametersType) -> Result<Self> {
        match params {
            MediaParametersType::Video(params) => Ok(params.clone()),
            _ => Err(invalid_param_error!(params)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum MediaParametersType {
    #[cfg(feature = "audio")]
    Audio(AudioParameters),
    #[cfg(feature = "video")]
    Video(VideoParameters),
}

#[cfg(feature = "audio")]
impl From<AudioParameters> for MediaParametersType {
    fn from(params: AudioParameters) -> Self {
        MediaParametersType::Audio(params)
    }
}

#[cfg(feature = "video")]
impl From<VideoParameters> for MediaParametersType {
    fn from(params: VideoParameters) -> Self {
        MediaParametersType::Video(params)
    }
}

#[derive(Clone, Debug)]
pub enum CodecParametersType {
    Decoder(DecoderParameters),
    Encoder(EncoderParameters),
}

impl From<DecoderParameters> for CodecParametersType {
    fn from(params: DecoderParameters) -> Self {
        CodecParametersType::Decoder(params)
    }
}

impl From<EncoderParameters> for CodecParametersType {
    fn from(params: EncoderParameters) -> Self {
        CodecParametersType::Encoder(params)
    }
}

pub trait CodecInformation {
    fn id(&self) -> CodecID;
    fn name(&self) -> &'static str;
}

pub trait Codec<T: CodecSpec>: CodecInformation {
    fn configure(&mut self, params: Option<&CodecParameters>, options: Option<&Variant>) -> Result<()>;
    fn set_option(&mut self, key: &str, value: &Variant) -> Result<()>;
}

pub trait CodecBuilder<T: CodecSpec>: Any + Send + Sync {
    fn ids(&self) -> &[CodecID];
    fn name(&self) -> &'static str;
}

#[macro_export]
macro_rules! define_codec_builder {
    (
        $builder:ident<$decoder_type:ty> {
            name: $codec_name:expr,
            ids: [$($id:ident),* $(,)?]
        }
    ) => {
        pub struct $builder;

        impl $crate::codec::CodecBuilder<$decoder_type> for $builder {
            fn ids(&self) -> &[$crate::codec::CodecID] {
                const IDS: &[$crate::codec::CodecID] = &[$($crate::codec::CodecID::$id),*];
                IDS
            }
            fn name(&self) -> &'static str {
                $codec_name
            }
        }
    };
}
