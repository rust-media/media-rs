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
pub struct DecoderParameters {
    pub extra_data: Option<Vec<u8>>,
}

impl DecoderParameters {
    fn update(&mut self, other: &DecoderParameters) {
        if let Some(ref extra_data) = other.extra_data {
            self.extra_data = Some(extra_data.clone());
        }
    }

    fn update_with_option(&mut self, key: &str, value: &Variant) {
        #[allow(clippy::single_match)]
        match key {
            "extra_data" => self.extra_data = value.get_buffer(),
            _ => {}
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioDecoderParameters {
    pub audio: AudioParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "audio")]
impl CodecParameters for AudioDecoderParameters {
    fn media_type() -> MediaType {
        MediaType::Audio
    }

    fn codec_type() -> CodecType {
        CodecType::Decoder
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
pub struct AudioDecoderConfiguration {
    pub audio: AudioParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "audio")]
impl CodecConfiguration for AudioDecoderConfiguration {
    type Parameters = AudioDecoderParameters;

    fn from_parameters(parameters: &Self::Parameters) -> Result<Self> {
        Ok(Self {
            audio: parameters.audio.clone(),
            decoder: parameters.decoder.clone(),
        })
    }

    fn configure(&mut self, parameters: &Self::Parameters) -> Result<()> {
        self.audio.update(&parameters.audio);
        self.decoder.update(&parameters.decoder);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.audio.update_with_option(key, value);
        self.decoder.update_with_option(key, value);
        Ok(())
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug, Default)]
pub struct VideoDecoderParameters {
    pub video: VideoParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "video")]
impl CodecParameters for VideoDecoderParameters {
    fn media_type() -> MediaType {
        MediaType::Video
    }

    fn codec_type() -> CodecType {
        CodecType::Decoder
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug)]
pub struct VideoDecoderConfiguration {
    pub video: VideoParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "video")]
impl CodecConfiguration for VideoDecoderConfiguration {
    type Parameters = VideoDecoderParameters;

    fn from_parameters(parameters: &Self::Parameters) -> Result<Self> {
        Ok(Self {
            video: parameters.video.clone(),
            decoder: parameters.decoder.clone(),
        })
    }

    fn configure(&mut self, parameters: &Self::Parameters) -> Result<()> {
        self.video.update(&parameters.video);
        self.decoder.update(&parameters.decoder);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.video.update_with_option(key, value);
        self.decoder.update_with_option(key, value);
        Ok(())
    }
}

pub trait Decoder<T: CodecConfiguration>: Codec<T> + Send + Sync {
    fn send_packet(&mut self, config: &T, packet: &Packet) -> Result<()>;
    fn receive_frame(&mut self, config: &T) -> Result<Frame<'static>> {
        self.receive_frame_borrowed(config).map(|frame| frame.into_owned())
    }
    fn receive_frame_borrowed(&mut self, config: &T) -> Result<Frame<'_>>;
    fn flush(&mut self, config: &T) -> Result<()>;
}

pub trait DecoderBuilder<T: CodecConfiguration>: CodecBuilder<T> {
    fn new_decoder(&self, id: CodecID, parameters: &T::Parameters, options: Option<&Variant>) -> Result<Box<dyn Decoder<T>>>;
}

pub struct DecoderContext<T: CodecConfiguration> {
    pub configurations: T,
    decoder: Box<dyn Decoder<T>>,
}

#[cfg(feature = "audio")]
static AUDIO_DECODER_LIST: LazyCodecList<AudioDecoderConfiguration> = LazyLock::new(|| {
    RwLock::new(CodecList::<AudioDecoderConfiguration> {
        codecs: HashMap::new(),
    })
});

#[cfg(feature = "video")]
static VIDEO_DECODER_LIST: LazyCodecList<VideoDecoderConfiguration> = LazyLock::new(|| {
    RwLock::new(CodecList::<VideoDecoderConfiguration> {
        codecs: HashMap::new(),
    })
});

pub fn register_decoder<T: CodecConfiguration>(builder: Arc<dyn DecoderBuilder<T>>, default: bool) -> Result<()> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioDecoderConfiguration>() => {
            let builder = convert_codec_builder::<dyn CodecBuilder<AudioDecoderConfiguration>>(builder)?;
            register_codec(&AUDIO_DECODER_LIST, builder, default)
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoDecoderConfiguration>() => {
            let builder = convert_codec_builder::<dyn CodecBuilder<VideoDecoderConfiguration>>(builder)?;
            register_codec(&VIDEO_DECODER_LIST, builder, default)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_decoder<T: CodecConfiguration>(id: CodecID) -> Result<Arc<dyn DecoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        type_id if type_id == TypeId::of::<AudioDecoderConfiguration>() => {
            let builder = find_codec(&AUDIO_DECODER_LIST, id)?;
            convert_codec_builder::<dyn DecoderBuilder<T>>(builder)
        }
        #[cfg(feature = "video")]
        type_id if type_id == TypeId::of::<VideoDecoderConfiguration>() => {
            let builder = find_codec(&VIDEO_DECODER_LIST, id)?;
            convert_codec_builder::<dyn DecoderBuilder<T>>(builder)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_decoder_by_name<T: CodecConfiguration>(name: &str) -> Result<Arc<dyn DecoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioDecoderConfiguration>() => {
            let builder = find_codec_by_name(&AUDIO_DECODER_LIST, name)?;
            convert_codec_builder::<dyn DecoderBuilder<T>>(builder)
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoDecoderConfiguration>() => {
            let builder = find_codec_by_name(&VIDEO_DECODER_LIST, name)?;
            convert_codec_builder::<dyn DecoderBuilder<T>>(builder)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

impl<T: CodecConfiguration> DecoderContext<T> {
    pub fn from_codec_id(id: CodecID, parameters: &T::Parameters, options: Option<&Variant>) -> Result<Self> {
        let builder = find_decoder(id)?;
        let decoder = builder.new_decoder(id, parameters, options)?;
        let config = T::from_parameters(parameters)?;

        Ok(Self {
            configurations: config,
            decoder,
        })
    }

    pub fn from_codec_name(name: &str, parameters: &T::Parameters, options: Option<&Variant>) -> Result<Self> {
        let builder = find_decoder_by_name(name)?;
        let decoder = builder.new_decoder(builder.id(), parameters, options)?;
        let config = T::from_parameters(parameters)?;

        Ok(Self {
            configurations: config,
            decoder,
        })
    }

    pub fn codec_id(&self) -> CodecID {
        self.decoder.id()
    }

    pub fn codec_name(&self) -> &'static str {
        self.decoder.name()
    }

    pub fn configure(&mut self, parameters: Option<&T::Parameters>, options: Option<&Variant>) -> Result<()> {
        if let Some(params) = parameters {
            self.configurations.configure(params)?;
        }
        self.decoder.configure(parameters, options)
    }

    pub fn set_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.decoder.set_option(key, value)
    }

    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        self.decoder.send_packet(&self.configurations, packet)
    }

    pub fn receive_frame(&mut self) -> Result<Frame<'static>> {
        self.decoder.receive_frame(&self.configurations)
    }

    pub fn receive_frame_borrowed(&mut self) -> Result<Frame<'_>> {
        self.decoder.receive_frame_borrowed(&self.configurations)
    }
}
