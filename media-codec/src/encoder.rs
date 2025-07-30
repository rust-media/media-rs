use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, RwLock},
};

use media_base::{frame::Frame, Result};
use x_variant::Variant;

use crate::{
    codec::{find_codec, find_codec_by_name, register_codec, CodecBuilder, CodecID, CodecList, CodecParameters},
    packet::Packet,
};

pub trait Encoder: Send + Sync {
    fn send_frame(&mut self, frame: &Frame) -> Result<()>;
    fn receive_packet(&mut self) -> Result<Packet>;
    fn flush(&mut self) -> Result<()>;
}

pub trait EncoderBuilder: CodecBuilder {
    fn new(&self, parameters: Option<CodecParameters>, options: Option<Variant>) -> Result<Box<dyn Encoder>>;
}

type EncoderBuilderList = CodecList<Arc<dyn EncoderBuilder>>;

pub struct EncoderContext {
    pub parameters: Option<CodecParameters>,
    pub options: Option<Variant>,
    encoder: Box<dyn Encoder>,
}

static CODEC_LIST: LazyLock<RwLock<EncoderBuilderList>> = LazyLock::new(|| {
    RwLock::new(EncoderBuilderList {
        codecs: HashMap::new(),
    })
});

pub fn register_encoder(builder: Arc<dyn EncoderBuilder>, default: bool) -> Result<()> {
    register_codec(&CODEC_LIST, builder, default)
}

pub(crate) fn find_encoder(codec_id: CodecID) -> Result<Arc<dyn EncoderBuilder>> {
    find_codec(&CODEC_LIST, codec_id)
}

pub(crate) fn find_encoder_by_name(name: &str) -> Result<Arc<dyn EncoderBuilder>> {
    find_codec_by_name(&CODEC_LIST, name)
}

impl EncoderContext {
    pub fn from_codec_id(codec_id: CodecID, parameters: Option<CodecParameters>, options: Option<Variant>) -> Result<Self> {
        let builder = find_encoder(codec_id)?;
        let encoder = builder.new(parameters.clone(), options.clone())?;

        Ok(Self {
            parameters,
            options,
            encoder,
        })
    }

    pub fn from_codec_name(name: &str, parameters: Option<CodecParameters>, options: Option<Variant>) -> Result<Self> {
        let builder = find_encoder_by_name(name)?;
        let encoder = builder.new(parameters.clone(), options.clone())?;

        Ok(Self {
            parameters,
            options,
            encoder,
        })
    }

    pub fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        self.encoder.send_frame(frame)
    }

    pub fn receive_packet(&mut self) -> Result<Packet> {
        self.encoder.receive_packet()
    }

    pub fn flush(&mut self) -> Result<()> {
        self.encoder.flush()
    }
}
