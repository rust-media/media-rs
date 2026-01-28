use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, LazyLock, RwLock},
};

#[cfg(feature = "audio")]
use media_core::audio::AudioFrameDescriptor;
#[cfg(feature = "video")]
use media_core::video::VideoFrameDescriptor;
use media_core::{
    buffer::BufferPool,
    error::Error,
    frame::{Frame, SharedFrame},
    invalid_param_error,
    rational::Rational64,
    variant::Variant,
    MediaType, Result,
};

#[cfg(feature = "audio")]
use crate::AudioParameters;
#[cfg(feature = "video")]
use crate::VideoParameters;
use crate::{
    find_codec, find_codec_by_name, packet::Packet, register_codec, Codec, CodecBuilder, CodecID, CodecList, CodecParameters, CodecParametersType,
    CodecSpec, CodecType, LazyCodecList, MediaParametersType,
};

#[derive(Clone, Debug, Default)]
pub struct EncoderParameters {
    pub bit_rate: Option<u64>,
    pub profile: Option<i32>,
    pub level: Option<i32>,
    pub use_pool: Option<bool>,
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
impl CodecSpec for AudioEncoder {
    type FrameDescriptor = AudioFrameDescriptor;

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
impl CodecSpec for VideoEncoder {
    type FrameDescriptor = VideoFrameDescriptor;

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
        let video_params = (&params.media).try_into()?;
        let encoder_params = (&params.codec).try_into()?;
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

pub trait Encoder<T: CodecSpec>: Codec<T> + Send + Sync {
    fn init(&mut self, _config: &T) -> Result<()> {
        Ok(())
    }
    fn send_frame(&mut self, config: &T, pool: Option<&Arc<BufferPool>>, frame: SharedFrame<Frame<'static, T::FrameDescriptor>>) -> Result<()>;
    fn receive_packet(&mut self, config: &T, pool: Option<&Arc<BufferPool>>) -> Result<Packet<'static>>;
    fn flush(&mut self, config: &T) -> Result<()>;
}

pub trait EncoderBuilder<T: CodecSpec>: CodecBuilder<T> {
    fn new_encoder(&self, id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Box<dyn Encoder<T>>>;
}

pub trait EncoderSpec: CodecSpec {
    fn register(builder: Arc<dyn EncoderBuilder<Self>>, default: bool) -> Result<()>;
    fn find(id: CodecID) -> Result<Arc<dyn EncoderBuilder<Self>>>;
    fn find_by_name(name: &str) -> Result<Arc<dyn EncoderBuilder<Self>>>;
}

pub struct EncoderContext<T: EncoderSpec> {
    pub config: T,
    pub time_base: Option<Rational64>,
    encoder: Box<dyn Encoder<T>>,
    pool: Option<Arc<BufferPool>>,
}

#[cfg(feature = "audio")]
static AUDIO_ENCODER_LIST: LazyCodecList<AudioEncoder, dyn EncoderBuilder<AudioEncoder>> = LazyLock::new(|| {
    RwLock::new(CodecList::<AudioEncoder, dyn EncoderBuilder<AudioEncoder>> {
        codecs: HashMap::new(),
        _marker: PhantomData,
    })
});

#[cfg(feature = "audio")]
impl EncoderSpec for AudioEncoder {
    fn register(builder: Arc<dyn EncoderBuilder<Self>>, default: bool) -> Result<()> {
        register_codec(&AUDIO_ENCODER_LIST, builder, default)
    }

    fn find(id: CodecID) -> Result<Arc<dyn EncoderBuilder<Self>>> {
        find_codec(&AUDIO_ENCODER_LIST, id)
    }

    fn find_by_name(name: &str) -> Result<Arc<dyn EncoderBuilder<Self>>> {
        find_codec_by_name(&AUDIO_ENCODER_LIST, name)
    }
}

#[cfg(feature = "video")]
static VIDEO_ENCODER_LIST: LazyCodecList<VideoEncoder, dyn EncoderBuilder<VideoEncoder>> = LazyLock::new(|| {
    RwLock::new(CodecList::<VideoEncoder, dyn EncoderBuilder<VideoEncoder>> {
        codecs: HashMap::new(),
        _marker: PhantomData,
    })
});

#[cfg(feature = "video")]
impl EncoderSpec for VideoEncoder {
    fn register(builder: Arc<dyn EncoderBuilder<Self>>, default: bool) -> Result<()> {
        register_codec(&VIDEO_ENCODER_LIST, builder, default)
    }

    fn find(id: CodecID) -> Result<Arc<dyn EncoderBuilder<Self>>> {
        find_codec(&VIDEO_ENCODER_LIST, id)
    }

    fn find_by_name(name: &str) -> Result<Arc<dyn EncoderBuilder<Self>>> {
        find_codec_by_name(&VIDEO_ENCODER_LIST, name)
    }
}

pub fn register_encoder<T: EncoderSpec>(builder: Arc<dyn EncoderBuilder<T>>, default: bool) -> Result<()> {
    T::register(builder, default)
}

pub(crate) fn find_encoder<T: EncoderSpec>(id: CodecID) -> Result<Arc<dyn EncoderBuilder<T>>> {
    T::find(id)
}

pub(crate) fn find_encoder_by_name<T: EncoderSpec>(name: &str) -> Result<Arc<dyn EncoderBuilder<T>>> {
    T::find_by_name(name)
}

impl<T: EncoderSpec> EncoderContext<T> {
    pub fn new(codec_id: CodecID, codec_name: Option<&str>, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        let builder = if let Some(codec_name) = codec_name {
            find_encoder_by_name(codec_name)?
        } else {
            find_encoder(codec_id)?
        };

        let encoder = builder.new_encoder(codec_id, params, options)?;

        Self::new_with_encoder(encoder, params)
    }

    pub fn new_with_encoder(encoder: Box<dyn Encoder<T>>, params: &CodecParameters) -> Result<Self> {
        let config = T::from_parameters(params)?;

        let buffer_pool = match &params.codec {
            CodecParametersType::Encoder(encoder_params) => {
                if encoder_params.use_pool.unwrap_or(false) {
                    Some(BufferPool::new(0))
                } else {
                    None
                }
            }
            _ => return Err(invalid_param_error!(params)),
        };

        Ok(Self {
            config,
            time_base: None,
            encoder,
            pool: buffer_pool,
        })
    }

    pub fn codec_id(&self) -> CodecID {
        self.encoder.id()
    }

    pub fn codec_name(&self) -> &'static str {
        self.encoder.name()
    }

    pub fn configure(&mut self, params: Option<&CodecParameters>, options: Option<&Variant>) -> Result<()> {
        if let Some(params) = params {
            self.config.configure(params)?;
        }
        self.encoder.configure(params, options)
    }

    pub fn set_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.encoder.set_option(key, value)
    }

    pub fn send_frame(&mut self, frame: SharedFrame<Frame<'static, T::FrameDescriptor>>) -> Result<()> {
        self.encoder.send_frame(&self.config, self.pool.as_ref(), frame)
    }

    pub fn receive_packet(&mut self) -> Result<Packet<'static>> {
        let mut packet = self.encoder.receive_packet(&self.config, self.pool.as_ref())?;

        packet.time_base = packet.time_base.or(self.time_base);

        Ok(packet)
    }
}
