//! Muxer types and traits

use std::io::{BufWriter, Seek, Write};

use media_codec_types::packet::Packet;
use media_core::{variant::Variant, Result};

use crate::{
    format::{Format, FormatBuilder},
    stream::StreamCollection,
    track::TrackCollection,
};

/// Trait alias for types that can be used as writers
pub trait Writer: Write + Seek {}

impl<W: Write + Seek> Writer for BufWriter<W> {}

/// State maintained by a muxer during operation
pub struct MuxerState {
    /// Metadata to write to the container
    pub metadata: Variant,
    /// Collection of tracks to mux
    pub tracks: TrackCollection,
    /// Collection of streams to mux
    pub streams: StreamCollection,
}

impl MuxerState {
    /// Creates a new muxer state
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

/// Trait for muxer implementations
pub trait Muxer: Format {
    /// Writes the container header
    fn write_header(&mut self, writer: &mut dyn Write, state: &MuxerState) -> Result<()>;

    /// Writes a packet to the container
    fn write_packet(&mut self, writer: &mut dyn Writer, state: &MuxerState, packet: &Packet) -> Result<()>;

    /// Writes the container trailer and finalizes the output
    fn write_trailer(&mut self, writer: &mut dyn Writer) -> Result<()>;
}

/// Builder trait for creating muxer instance
pub trait MuxerBuilder: FormatBuilder {
    fn new_muxer(&self) -> Result<Box<dyn Muxer>>;
}
