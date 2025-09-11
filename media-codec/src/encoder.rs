use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, LazyLock, RwLock},
};

use media_core::{error::Error, frame::Frame, variant::Variant, MediaType, Result};

#[cfg(feature = "audio")]
use crate::AudioParameters;
#[cfg(feature = "video")]
use crate::VideoParameters;
use crate::{
    convert_codec_builder, find_codec, find_codec_by_name, packet::Packet, register_codec, Codec, CodecBuilder, CodecConfiguration, CodecID,
    CodecList, CodecParameters, CodecType, LazyCodecList,
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

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioEncoderParameters {
    pub audio: AudioParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "audio")]
impl CodecParameters for AudioEncoderParameters {
    fn media_type() -> MediaType {
        MediaType::Audio
    }

    fn codec_type() -> CodecType {
        CodecType::Encoder
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
pub struct AudioEncoderConfiguration {
    pub audio: AudioParameters,
    pub encoder: EncoderParameters,
    // audio encoder specific configuration
    pub frame_size: Option<u32>,
    pub delay: Option<u32>,
}

#[cfg(feature = "audio")]
impl CodecConfiguration for AudioEncoderConfiguration {
    type Parameters = AudioEncoderParameters;

    fn from_parameters(parameters: &Self::Parameters) -> Result<Self> {
        Ok(Self {
            audio: parameters.audio.clone(),
            encoder: parameters.encoder.clone(),
            frame_size: None,
            delay: None,
        })
    }

    fn configure(&mut self, parameters: &Self::Parameters) -> Result<()> {
        self.audio.update(&parameters.audio);
        self.encoder.update(&parameters.encoder);
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
impl CodecParameters for VideoEncoderParameters {
    fn media_type() -> MediaType {
        MediaType::Video
    }

    fn codec_type() -> CodecType {
        CodecType::Encoder
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug)]
pub struct VideoEncoderConfiguration {
    pub video: VideoParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "video")]
impl CodecConfiguration for VideoEncoderConfiguration {
    type Parameters = VideoEncoderParameters;

    fn from_parameters(parameters: &Self::Parameters) -> Result<Self> {
        Ok(Self {
            video: parameters.video.clone(),
            encoder: parameters.encoder.clone(),
        })
    }

    fn configure(&mut self, parameters: &Self::Parameters) -> Result<()> {
        self.video.update(&parameters.video);
        self.encoder.update(&parameters.encoder);
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
    fn new_encoder(&self, id: CodecID, parameters: &T::Parameters, options: Option<&Variant>) -> Result<Box<dyn Encoder<T>>>;
}

pub struct EncoderContext<T: CodecConfiguration> {
    pub configurations: T,
    encoder: Box<dyn Encoder<T>>,
}

#[cfg(feature = "audio")]
static AUDIO_ENCODER_LIST: LazyCodecList<AudioEncoderConfiguration> = LazyLock::new(|| {
    RwLock::new(CodecList::<AudioEncoderConfiguration> {
        codecs: HashMap::new(),
    })
});

#[cfg(feature = "video")]
static VIDEO_ENCODER_LIST: LazyCodecList<VideoEncoderConfiguration> = LazyLock::new(|| {
    RwLock::new(CodecList::<VideoEncoderConfiguration> {
        codecs: HashMap::new(),
    })
});

pub fn register_encoder<T: CodecConfiguration>(builder: Arc<dyn EncoderBuilder<T>>, default: bool) -> Result<()> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioEncoderConfiguration>() => {
            let builder = convert_codec_builder::<dyn CodecBuilder<AudioEncoderConfiguration>>(builder)?;
            register_codec(&AUDIO_ENCODER_LIST, builder, default)
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoEncoderConfiguration>() => {
            let builder = convert_codec_builder::<dyn CodecBuilder<VideoEncoderConfiguration>>(builder)?;
            register_codec(&VIDEO_ENCODER_LIST, builder, default)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_encoder<T: CodecConfiguration>(id: CodecID) -> Result<Arc<dyn EncoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        type_id if type_id == TypeId::of::<AudioEncoderConfiguration>() => {
            let builder = find_codec(&AUDIO_ENCODER_LIST, id)?;
            convert_codec_builder::<dyn EncoderBuilder<T>>(builder)
        }
        #[cfg(feature = "video")]
        type_id if type_id == TypeId::of::<VideoEncoderConfiguration>() => {
            let builder = find_codec(&VIDEO_ENCODER_LIST, id)?;
            convert_codec_builder::<dyn EncoderBuilder<T>>(builder)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_encoder_by_name<T: CodecConfiguration>(name: &str) -> Result<Arc<dyn EncoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioEncoderConfiguration>() => {
            let builder = find_codec_by_name(&AUDIO_ENCODER_LIST, name)?;
            convert_codec_builder::<dyn EncoderBuilder<T>>(builder)
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoEncoderConfiguration>() => {
            let builder = find_codec_by_name(&VIDEO_ENCODER_LIST, name)?;
            convert_codec_builder::<dyn EncoderBuilder<T>>(builder)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

impl<T: CodecConfiguration> EncoderContext<T> {
    pub fn from_codec_id(id: CodecID, parameters: &T::Parameters, options: Option<&Variant>) -> Result<Self> {
        let builder = find_encoder(id)?;
        let encoder = builder.new_encoder(id, parameters, options)?;
        let config = T::from_parameters(parameters)?;

        Ok(Self {
            configurations: config,
            encoder,
        })
    }

    pub fn from_codec_name(name: &str, parameters: &T::Parameters, options: Option<&Variant>) -> Result<Self> {
        let builder = find_encoder_by_name(name)?;
        let encoder = builder.new_encoder(builder.id(), parameters, options)?;
        let config = T::from_parameters(parameters)?;

        Ok(Self {
            configurations: config,
            encoder,
        })
    }

    pub fn configure(&mut self, parameters: Option<&T::Parameters>, options: Option<&Variant>) -> Result<()> {
        if let Some(params) = parameters {
            self.configurations.configure(params)?;
        }
        self.encoder.configure(parameters, options)
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
