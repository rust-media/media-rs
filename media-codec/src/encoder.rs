use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, LazyLock, RwLock},
};

pub use media_codec_types::encoder::*;
use media_codec_types::{packet::Packet, CodecID, CodecParameters, CodecParametersType, CodecSpec};
use media_core::{
    buffer::BufferPool,
    frame::{Frame, SharedFrame},
    invalid_param_error,
    rational::Rational64,
    variant::Variant,
    Result,
};

use crate::codec::{find_codec, find_codec_by_name, register_codec, CodecList, LazyCodecList};

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

        #[allow(unreachable_patterns)]
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
