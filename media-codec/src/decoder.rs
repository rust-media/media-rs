use std::{
    any::TypeId,
    collections::HashMap,
    mem,
    sync::{Arc, LazyLock, RwLock},
};

#[cfg(feature = "audio")]
use media_core::audio::AudioFrameDescriptor;
#[cfg(feature = "video")]
use media_core::video::VideoFrameDescriptor;
use media_core::{
    error::Error,
    frame::{Frame, SharedFrame},
    frame_pool::{FrameCreator, FramePool},
    invalid_param_error,
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
pub struct DecoderParameters {
    pub extra_data: Option<Vec<u8>>,
    pub use_pool: Option<bool>,
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
            "use_pool" => self.use_pool = value.get_bool(),
            _ => {}
        }
    }
}

impl TryFrom<&CodecParametersType> for DecoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParametersType) -> Result<Self> {
        match params {
            CodecParametersType::Decoder(params) => Ok(params.clone()),
            _ => Err(invalid_param_error!(params)),
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
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for AudioDecoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: match &params.media {
                MediaParametersType::Audio(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            decoder: match &params.codec {
                CodecParametersType::Decoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
pub struct AudioDecoder {
    pub audio: AudioParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "audio")]
#[deprecated = "Use 'AudioDecoder' instead"]
pub type AudioDecoderConfiguration = AudioDecoder;

#[cfg(feature = "audio")]
impl CodecSpec for AudioDecoder {
    type FrameDescriptor = AudioFrameDescriptor;

    fn media_type() -> MediaType {
        MediaType::Audio
    }

    fn codec_type() -> CodecType {
        CodecType::Decoder
    }

    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: (&params.media).try_into()?,
            decoder: (&params.codec).try_into()?,
        })
    }

    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let audio_params = (&params.media).try_into()?;
        let decoder_params = (&params.codec).try_into()?;
        self.audio.update(&audio_params);
        self.decoder.update(&decoder_params);
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
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for VideoDecoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            decoder: match &params.codec {
                CodecParametersType::Decoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug)]
pub struct VideoDecoder {
    pub video: VideoParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "video")]
#[deprecated = "Use 'VideoDecoder' instead"]
pub type VideoDecoderConfiguration = VideoDecoder;

#[cfg(feature = "video")]
impl CodecSpec for VideoDecoder {
    type FrameDescriptor = VideoFrameDescriptor;

    fn media_type() -> MediaType {
        MediaType::Video
    }

    fn codec_type() -> CodecType {
        CodecType::Decoder
    }

    #[allow(unreachable_patterns)]
    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            decoder: match &params.codec {
                CodecParametersType::Decoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }

    #[allow(unreachable_patterns)]
    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let video_params = (&params.media).try_into()?;
        let decoder_params = (&params.codec).try_into()?;
        self.video.update(&video_params);
        self.decoder.update(&decoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.video.update_with_option(key, value);
        self.decoder.update_with_option(key, value);
        Ok(())
    }
}

pub trait Decoder<T: CodecSpec>: Codec<T> + Send + Sync {
    fn init(&mut self, _config: &T) -> Result<()> {
        Ok(())
    }
    fn send_packet(&mut self, config: &T, pool: Option<&Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>, packet: Packet) -> Result<()>;
    fn receive_frame(
        &mut self,
        config: &T,
        pool: Option<&Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>,
    ) -> Result<SharedFrame<Frame<'static, T::FrameDescriptor>>>;
    fn flush(&mut self, config: &T) -> Result<()>;
}

pub trait DecoderBuilder<T: CodecSpec>: CodecBuilder<T> {
    fn new_decoder(&self, id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Box<dyn Decoder<T>>>;
}

pub struct DecoderContext<T: CodecSpec> {
    pub config: T,
    decoder: Box<dyn Decoder<T>>,
    pool: Option<Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>,
}

#[cfg(feature = "audio")]
static AUDIO_DECODER_LIST: LazyCodecList<AudioDecoder> = LazyLock::new(|| {
    RwLock::new(CodecList::<AudioDecoder> {
        codecs: HashMap::new(),
    })
});

#[cfg(feature = "video")]
static VIDEO_DECODER_LIST: LazyCodecList<VideoDecoder> = LazyLock::new(|| {
    RwLock::new(CodecList::<VideoDecoder> {
        codecs: HashMap::new(),
    })
});

pub fn register_decoder<T: CodecSpec>(builder: Arc<dyn DecoderBuilder<T>>, default: bool) -> Result<()> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioDecoder>() => {
            let builder = unsafe { mem::transmute::<Arc<dyn DecoderBuilder<T>>, Arc<dyn CodecBuilder<AudioDecoder>>>(builder) };
            register_codec(&AUDIO_DECODER_LIST, builder, default)
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoDecoder>() => {
            let builder = unsafe { mem::transmute::<Arc<dyn DecoderBuilder<T>>, Arc<dyn CodecBuilder<VideoDecoder>>>(builder) };
            register_codec(&VIDEO_DECODER_LIST, builder, default)
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_decoder<T: CodecSpec>(id: CodecID) -> Result<Arc<dyn DecoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        type_id if type_id == TypeId::of::<AudioDecoder>() => {
            let builder = find_codec(&AUDIO_DECODER_LIST, id)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<AudioDecoder>>, Arc<dyn DecoderBuilder<T>>>(builder)) }
        }
        #[cfg(feature = "video")]
        type_id if type_id == TypeId::of::<VideoDecoder>() => {
            let builder = find_codec(&VIDEO_DECODER_LIST, id)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<VideoDecoder>>, Arc<dyn DecoderBuilder<T>>>(builder)) }
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

pub(crate) fn find_decoder_by_name<T: CodecSpec>(name: &str) -> Result<Arc<dyn DecoderBuilder<T>>> {
    match TypeId::of::<T>() {
        #[cfg(feature = "audio")]
        id if id == TypeId::of::<AudioDecoder>() => {
            let builder = find_codec_by_name(&AUDIO_DECODER_LIST, name)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<AudioDecoder>>, Arc<dyn DecoderBuilder<T>>>(builder)) }
        }
        #[cfg(feature = "video")]
        id if id == TypeId::of::<VideoDecoder>() => {
            let builder = find_codec_by_name(&VIDEO_DECODER_LIST, name)?;
            unsafe { Ok(mem::transmute::<Arc<dyn CodecBuilder<VideoDecoder>>, Arc<dyn DecoderBuilder<T>>>(builder)) }
        }
        _ => Err(Error::Unsupported("codec parameters type".to_string())),
    }
}

impl<T: CodecSpec> DecoderContext<T> {
    fn new_with_builder(builder: Arc<dyn DecoderBuilder<T>>, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        let decoder = builder.new_decoder(builder.id(), params, options)?;
        let config = T::from_parameters(params)?;

        let frame_pool = match &params.codec {
            CodecParametersType::Decoder(decoder_params) => {
                if decoder_params.use_pool.unwrap_or(false) {
                    Some(FramePool::<Frame<'static, T::FrameDescriptor>>::new())
                } else {
                    None
                }
            }
            _ => return Err(invalid_param_error!(params)),
        };

        Ok(Self {
            config,
            decoder,
            pool: frame_pool,
        })
    }

    pub fn from_codec_id(id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        Self::new_with_builder(find_decoder(id)?, params, options)
    }

    pub fn from_codec_name(name: &str, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        Self::new_with_builder(find_decoder_by_name(name)?, params, options)
    }

    pub fn with_frame_creator(mut self, creator: Box<dyn FrameCreator<T::FrameDescriptor>>) -> Self {
        if let Some(pool) = self.pool.as_mut().and_then(Arc::get_mut) {
            pool.configure(None, Some(creator));
        }

        self
    }

    pub fn codec_id(&self) -> CodecID {
        self.decoder.id()
    }

    pub fn codec_name(&self) -> &'static str {
        self.decoder.name()
    }

    pub fn configure(&mut self, params: Option<&CodecParameters>, options: Option<&Variant>) -> Result<()> {
        if let Some(params) = params {
            self.config.configure(params)?;
        }
        self.decoder.configure(params, options)
    }

    pub fn set_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.decoder.set_option(key, value)
    }

    pub fn send_packet(&mut self, packet: Packet) -> Result<()> {
        self.decoder.send_packet(&self.config, self.pool.as_ref(), packet)
    }

    pub fn receive_frame(&mut self) -> Result<SharedFrame<Frame<'static, T::FrameDescriptor>>> {
        self.decoder.receive_frame(&self.config, self.pool.as_ref())
    }
}
