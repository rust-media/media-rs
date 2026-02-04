use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, LazyLock, RwLock},
};

pub use media_codec_types::decoder::*;
use media_codec_types::{
    packet::{Packet, PacketProperties},
    CodecID, CodecParameters, CodecParametersType, CodecSpec,
};
use media_core::{
    frame::{Frame, SharedFrame},
    frame_pool::{FrameCreator, FramePool},
    invalid_param_error,
    rational::Rational64,
    variant::Variant,
    Result,
};

use crate::codec::{find_codec, find_codec_by_name, register_codec, CodecList, LazyCodecList};

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

        #[allow(unreachable_patterns)]
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

    pub fn flush(&mut self) -> Result<()> {
        self.decoder.flush(&self.config)
    }
}
