use std::{
    any::TypeId,
    collections::HashMap,
    mem,
    sync::{Arc, LazyLock, RwLock},
};

use media_core::{error::Error, frame::Frame, invalid_param_error, variant::Variant, MediaType, Result};

#[cfg(feature = "audio")]
use crate::AudioParameters;
#[cfg(feature = "video")]
use crate::VideoParameters;
use crate::{
    find_codec, find_codec_by_name, packet::Packet, register_codec, Codec, CodecBuilder, CodecConfiguration, CodecID, CodecList, CodecParameters,
    CodecParametersType, CodecType, LazyCodecList, MediaParametersType,
};

#[derive(Clone, Debug, Default)]
pub struct EncoderParameters {
    pub bit_rate: Option<u64>,
    pub profile: Option<i32>,
    pub level: Option<i32>,
}

impl EncoderParameters {
    fn update(&mut self, other: &EncoderParameters) {
        if other.bit_rate.is_some() {
            self.bit_rate = other.bit_rate;
        }
        if other.profile.is_some() {
            self.profile = other.profile;
        }
        if other.level.is_some() {
            self.level = other.level;
        }
    }

    fn update_with_option(&mut self, key: &str, value: &Variant) {
        match key {
            "bit_rate" => self.bit_rate = value.get_uint64(),
            "profile" => self.profile = value.get_int32(),
            "level" => self.level = value.get_int32(),
            _ => {}
        }
    }
}

impl TryFrom<&CodecParametersType> for EncoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParametersType) -> Result<Self> {
        match params {
            CodecParametersType::Encoder(params) => Ok(params.clone()),
            _ => Err(invalid_param_error!(params)),
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioEncoderParameters {
    pub audio: AudioParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "audio")]
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for AudioEncoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: match &params.media {
                MediaParametersType::Audio(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            encoder: match &params.codec {
                CodecParametersType::Encoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
pub struct AudioEncoder {
    pub audio: AudioParameters,
    pub encoder: EncoderParameters,
    // audio encoder specific configuration
    pub frame_size: Option<u32>,
    pub delay: Option<u32>,
}

#[cfg(feature = "audio")]
#[deprecated = "Use 'AudioEncoder' instead"]
pub type AudioEncoderConfiguration = AudioEncoder;

#[cfg(feature = "audio")]
impl CodecConfiguration for AudioEncoder {
    fn media_type() -> MediaType {
        MediaType::Audio
    }

    fn codec_type() -> CodecType {
        CodecType::Encoder
    }

    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: (&params.media).try_into()?,
            encoder: (&params.codec).try_into()?,
            frame_size: None,
            delay: None,
        })
    }

    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let audio_params = (&params.media).try_into()?;
        let encoder_params = (&params.codec).try_into()?;
        self.audio.update(&audio_params);
        self.encoder.update(&encoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.audio.update_with_option(key, value);
        self.encoder.update_with_option(key, value);

        match key {
            "frame_size" => self.frame_size = value.get_uint32(),
            "delay" => self.delay = value.get_uint32(),
            _ => {}
        }

        Ok(())
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug, Default)]
pub struct VideoEncoderParameters {
    pub video: VideoParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "video")]
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for VideoEncoderParameters {
    type Error = Error;

    #[allow(unreachable_patterns)]
    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            encoder: match &params.codec {
                CodecParametersType::Encoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug)]
pub struct VideoEncoder {
    pub video: VideoParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "video")]
#[deprecated = "Use 'VideoEncoder' instead"]
pub type VideoEncoderConfiguration = VideoEncoder;

#[cfg(feature = "video")]
impl CodecConfiguration for VideoEncoder {
    fn media_type() -> MediaType {
        MediaType::Video
    }

    fn codec_type() -> CodecType {
        CodecType::Encoder
    }

    #[allow(unreachable_patterns)]
    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            encoder: match &params.codec {
                CodecParametersType::Encoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }

    #[allow(unreachable_patterns)]
    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let video_params = match &params.media {
            MediaParametersType::Video(params) => params,
            _ => return Err(invalid_param_error!(params)),
        };

        let encoder_params = match &params.codec {
            CodecParametersType::Encoder(params) => params,
            _ => return Err(invalid_param_error!(params)),
        };

        self.video.update(&video_params);
        self.encoder.update(&encoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.video.update_with_option(key, value);
        self.encoder.update_with_option(key, value);
        Ok(())
    }
}

pub trait Encoder<T: CodecConfiguration>: Codec<T> + Send + Sync {
    fn send_frame(&mut self, config: &T, frame: &Frame) -> Result<()>;
    fn receive_packet(&mut self, config: &T) -> Result<Packet<'static>> {
        self.receive_packet_borrowed(config).map(|packet| packet.into_owned())
    }
    fn receive_packet_borrowed(&mut self, config: &T) -> Result<Packet<'_>>;
    fn flush(&mut self, config: &T) -> Result<()>;
}

pub trait EncoderBuilder<T: CodecConfiguration>: CodecBuilder<T> {
    fn new_encoder(&self, id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Box<dyn Encoder<T>>>;
}

pub struct EncoderContext<T: CodecConfiguration> {
    pub configurations: T,
    encoder: Box<dyn Encoder<T>>,
}

#[cfg(feature = "audio")]
static AUDIO_ENCODER_LIST: LazyCodecList<AudioEncoder> = LazyLock::new(|| {
    RwLock::new(CodecList::<AudioEncoder> {
        codecs: HashMap::new(),
    })
});

#[cfg(feature = "video")]
static VIDEO_ENCODER_LIST: LazyCodecList<VideoEncoder> = LazyLock::new(|| {
    RwLock::new(CodecList::<VideoEncoder> {
        codecs: HashMap::new(),
    })
});

pub fn register_encoder<T: CodecConfiguration>(builder: Arc<dyn EncoderBuilder<T>>, default: bool) -> Result<()> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioEncoder>() => {
            let builder = unsafe { mem::transmute::<Arc<dyn EncoderBuilder<T>>, Arc<dyn CodecBuilder<AudioEncoder>>>(builder) };
            register_codec(&AUDIO_ENCODER_LIST, builder, default)
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoEncoder>() => {
            let builder = unsafe { mem::transmute::<Arc<dyn EncoderBuilder<T>>, Arc<dyn CodecBuilder<VideoEncoder>>>(builder) };
            register_codec(&VIDEO_ENCODER_LIST, builder, default)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_encoder<T: CodecConfiguration>(id: CodecID) -> Result<Arc<dyn EncoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        type_id if type_id == TypeId::of::<AudioEncoder>() => {
            let builder = find_codec(&AUDIO_ENCODER_LIST, id)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<AudioEncoder>>, Arc<dyn EncoderBuilder<T>>>(builder)) }
        }
        #[cfg(feature = "video")]
        type_id if type_id == TypeId::of::<VideoEncoder>() => {
            let builder = find_codec(&VIDEO_ENCODER_LIST, id)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<VideoEncoder>>, Arc<dyn EncoderBuilder<T>>>(builder)) }
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_encoder_by_name<T: CodecConfiguration>(name: &str) -> Result<Arc<dyn EncoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioEncoder>() => {
            let builder = find_codec_by_name(&AUDIO_ENCODER_LIST, name)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<AudioEncoder>>, Arc<dyn EncoderBuilder<T>>>(builder)) }
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoEncoder>() => {
            let builder = find_codec_by_name(&VIDEO_ENCODER_LIST, name)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<VideoEncoder>>, Arc<dyn EncoderBuilder<T>>>(builder)) }
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

impl<T: CodecConfiguration> EncoderContext<T> {
    pub fn from_codec_id(id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        let builder = find_encoder(id)?;
        let encoder = builder.new_encoder(id, params, options)?;
        let config = T::from_parameters(params)?;

        Ok(Self {
            configurations: config,
            encoder,
        })
    }

    pub fn from_codec_name(name: &str, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        let builder = find_encoder_by_name(name)?;
        let encoder = builder.new_encoder(builder.id(), params, options)?;
        let config = T::from_parameters(params)?;

        Ok(Self {
            configurations: config,
            encoder,
        })
    }

    pub fn configure(&mut self, params: Option<&CodecParameters>, options: Option<&Variant>) -> Result<()> {
        if let Some(params) = params {
            self.configurations.configure(params)?;
        }
        self.encoder.configure(params, options)
    }

    pub fn set_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.encoder.set_option(key, value)
    }

    pub fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        self.encoder.send_frame(&self.configurations, frame)
    }

    pub fn receive_packet(&mut self) -> Result<Packet<'static>> {
        self.encoder.receive_packet(&self.configurations)
    }

    pub fn receive_packet_borrowed(&mut self) -> Result<Packet<'_>> {
        self.encoder.receive_packet_borrowed(&self.configurations)
    }
}
