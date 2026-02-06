use std::io::{BufWriter, Seek, Write};

use media_codec::packet::Packet;
use media_core::{variant::Variant, Result};

use crate::{format::Format, stream::StreamCollection, track::TrackCollection};

pub trait Writer: Write + Seek {}

impl<W: Write + Seek> Writer for BufWriter<W> {}

pub struct MuxerState {
    pub metadata: Variant,
    pub tracks: TrackCollection,
    pub streams: StreamCollection,
}

impl MuxerState {
    pub fn new() -> Self {
        Self {
            streams: StreamCollection::new(),
            tracks: TrackCollection::new(),
            metadata: Variant::new_dict(),
        }
    }
}

impl Default for MuxerState {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Muxer: Format {
    fn write_header<W: Writer>(&mut self, writer: &mut W, state: &MuxerState) -> Result<()>;
    fn write_packet<W: Writer>(&mut self, writer: &mut W, state: &MuxerState, packet: &Packet) -> Result<()>;
    fn write_trailer<W: Writer>(&mut self, writer: &mut W) -> Result<()>;
}

pub struct MuxerContext<D: Muxer, W: Writer> {
    muxer: D,
    writer: W,
    pub state: MuxerState,
}

impl<D: Muxer, W: Writer> MuxerContext<D, W> {
    pub fn new(muxer: D, writer: W) -> Self {
        Self {
            muxer,
            writer,
            state: MuxerState::new(),
        }
    }

    pub fn write_header(&mut self) -> Result<()> {
        self.muxer.write_header(&mut self.writer, &self.state)
    }

    pub fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        self.muxer.write_packet(&mut self.writer, &self.state, packet)
    }

    pub fn write_trailer(&mut self) -> Result<()> {
        self.muxer.write_trailer(&mut self.writer)
    }
}
