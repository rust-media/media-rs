use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, RwLock},
};

use media_base::{frame::Frame, Result};
use x_variant::Variant;

use crate::{
    codec::{find_codec, find_codec_by_name, register_codec, CodecBuilder, CodecID, CodecList, CodecParameters, LazyCodecList},
    packet::Packet,
};

pub trait Decoder: Send + Sync {
    fn send_packet(&mut self, parameters: Option<&CodecParameters>, packet: &Packet) -> Result<()>;
    fn receive_frame(&mut self, parameters: Option<&CodecParameters>) -> Result<Frame<'_>>;
}

pub trait DecoderBuilder: CodecBuilder {
    fn new_decoder(&self, codec_id: CodecID, parameters: Option<CodecParameters>, options: Option<Variant>) -> Result<Box<dyn Decoder>>;
}

type DecoderBuilderList = CodecList<Arc<dyn DecoderBuilder>>;

pub struct DecoderContext {
    pub parameters: Option<CodecParameters>,
    pub options: Option<Variant>,
    decoder: Box<dyn Decoder>,
}

static CODEC_LIST: LazyCodecList<dyn DecoderBuilder> = LazyLock::new(|| {
    RwLock::new(DecoderBuilderList {
        codecs: HashMap::new(),
    })
});

pub fn register_decoder(builder: Arc<dyn DecoderBuilder>, default: bool) -> Result<()> {
    register_codec(&CODEC_LIST, builder, default)
}

pub(crate) fn find_decoder(codec_id: CodecID) -> Result<Arc<dyn DecoderBuilder>> {
    find_codec(&CODEC_LIST, codec_id)
}

pub(crate) fn find_decoder_by_name(name: &str) -> Result<Arc<dyn DecoderBuilder>> {
    find_codec_by_name(&CODEC_LIST, name)
}

impl DecoderContext {
    pub fn from_codec_id(codec_id: CodecID, parameters: Option<CodecParameters>, options: Option<Variant>) -> Result<Self> {
        let builder = find_decoder(codec_id)?;
        let decoder = builder.new_decoder(codec_id, parameters.clone(), options.clone())?;

        Ok(Self {
            parameters,
            options,
            decoder,
        })
    }

    pub fn from_codec_name(name: &str, parameters: Option<CodecParameters>, options: Option<Variant>) -> Result<Self> {
        let builder = find_decoder_by_name(name)?;
        let decoder = builder.new_decoder(builder.id(), parameters.clone(), options.clone())?;

        Ok(Self {
            parameters,
            options,
            decoder,
        })
    }

    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        let params = self.parameters.as_ref();
        self.decoder.send_packet(params, packet)
    }

    pub fn receive_frame(&mut self) -> Result<Frame<'_>> {
        let params = self.parameters.as_ref();
        self.decoder.receive_frame(params)
    }
}
