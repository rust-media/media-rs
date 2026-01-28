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
    error::Error,
    frame::{Frame, SharedFrame},
    frame_pool::{FrameCreator, FramePool},
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
    find_codec, find_codec_by_name,
    packet::{Packet, PacketProperties},
    register_codec, Codec, CodecBuilder, CodecID, CodecList, CodecParameters, CodecParametersType, CodecSpec, CodecType, LazyCodecList,
    MediaParametersType,
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
    fn send_packet(&mut self, config: &T, pool: Option<&Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>, packet: &Packet) -> Result<()>;
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

pub trait DecoderSpec: CodecSpec {
    fn register(builder: Arc<dyn DecoderBuilder<Self>>, default: bool) -> Result<()>;
    fn find(id: CodecID) -> Result<Arc<dyn DecoderBuilder<Self>>>;
    fn find_by_name(name: &str) -> Result<Arc<dyn DecoderBuilder<Self>>>;
}

pub struct DecoderContext<T: DecoderSpec> {
    pub config: T,
    pub time_base: Option<Rational64>,
    decoder: Box<dyn Decoder<T>>,
    pool: Option<Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>,
    last_pkt_props: Option<PacketProperties>,
}

#[cfg(feature = "audio")]
static AUDIO_DECODER_LIST: LazyCodecList<AudioDecoder, dyn DecoderBuilder<AudioDecoder>> = LazyLock::new(|| {
    RwLock::new(CodecList::<AudioDecoder, dyn DecoderBuilder<AudioDecoder>> {
        codecs: HashMap::new(),
        _marker: PhantomData,
    })
});

#[cfg(feature = "audio")]
impl DecoderSpec for AudioDecoder {
    fn register(builder: Arc<dyn DecoderBuilder<Self>>, default: bool) -> Result<()> {
        register_codec(&AUDIO_DECODER_LIST, builder, default)
    }

    fn find(id: CodecID) -> Result<Arc<dyn DecoderBuilder<Self>>> {
        find_codec(&AUDIO_DECODER_LIST, id)
    }

    fn find_by_name(name: &str) -> Result<Arc<dyn DecoderBuilder<Self>>> {
        find_codec_by_name(&AUDIO_DECODER_LIST, name)
    }
}

#[cfg(feature = "video")]
static VIDEO_DECODER_LIST: LazyCodecList<VideoDecoder, dyn DecoderBuilder<VideoDecoder>> = LazyLock::new(|| {
    RwLock::new(CodecList::<VideoDecoder, dyn DecoderBuilder<VideoDecoder>> {
        codecs: HashMap::new(),
        _marker: PhantomData,
    })
});

#[cfg(feature = "video")]
impl DecoderSpec for VideoDecoder {
    fn register(builder: Arc<dyn DecoderBuilder<Self>>, default: bool) -> Result<()> {
        register_codec(&VIDEO_DECODER_LIST, builder, default)
    }

    fn find(id: CodecID) -> Result<Arc<dyn DecoderBuilder<Self>>> {
        find_codec(&VIDEO_DECODER_LIST, id)
    }

    fn find_by_name(name: &str) -> Result<Arc<dyn DecoderBuilder<Self>>> {
        find_codec_by_name(&VIDEO_DECODER_LIST, name)
    }
}

pub fn register_decoder<T: DecoderSpec>(builder: Arc<dyn DecoderBuilder<T>>, default: bool) -> Result<()> {
    T::register(builder, default)
}

pub(crate) fn find_decoder<T: DecoderSpec>(id: CodecID) -> Result<Arc<dyn DecoderBuilder<T>>> {
    T::find(id)
}

pub(crate) fn find_decoder_by_name<T: DecoderSpec>(name: &str) -> Result<Arc<dyn DecoderBuilder<T>>> {
    T::find_by_name(name)
}

impl<T: DecoderSpec> DecoderContext<T> {
    pub fn new(codec_id: CodecID, codec_name: Option<&str>, params: &CodecParameters, options: Option<&Variant>) -> Result<Self> {
        let builder = if let Some(codec_name) = codec_name {
            find_decoder_by_name(codec_name)?
        } else {
            find_decoder(codec_id)?
        };

        let decoder = builder.new_decoder(codec_id, params, options)?;

        Self::new_with_decoder(decoder, params)
    }

    pub fn new_with_decoder(decoder: Box<dyn Decoder<T>>, params: &CodecParameters) -> Result<Self> {
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
            time_base: None,
            last_pkt_props: None,
        })
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

    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        self.decoder.send_packet(&self.config, self.pool.as_ref(), packet)?;
        self.last_pkt_props = Some(PacketProperties::from_packet(packet));

        Ok(())
    }

    pub fn receive_frame(&mut self) -> Result<SharedFrame<Frame<'static, T::FrameDescriptor>>> {
        let mut shared_frame = self.decoder.receive_frame(&self.config, self.pool.as_ref())?;

        let frame = shared_frame.write();
        if let Some(frame) = frame {
            if let Some(pkt_props) = &self.last_pkt_props {
                frame.pts = frame.pts.or(pkt_props.pts);
                frame.dts = frame.dts.or(pkt_props.dts);
                frame.duration = frame.duration.or(pkt_props.duration);
            }

            frame.time_base = frame.time_base.or(self.time_base);
        }

        Ok(shared_frame)
    }
}
