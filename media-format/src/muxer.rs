//! Muxer

use std::sync::{Arc, LazyLock, RwLock};

use media_codec_types::packet::Packet;
use media_core::Result;
pub use media_format_types::muxer::*;

use crate::format::{find_format_by_extension, register_format, FormatList, LazyFormatList};

/// Global muxer registry
static MUXER_LIST: LazyFormatList<dyn MuxerBuilder> = LazyLock::new(|| RwLock::new(FormatList::new()));

/// Registers a muxer builder
pub fn register_muxer(builder: Arc<dyn MuxerBuilder>) -> Result<()> {
    register_format(&MUXER_LIST, builder)
}

/// Finds a muxer builder by file extension
pub fn find_muxer_by_extension(ext: &str) -> Result<Arc<dyn MuxerBuilder>> {
    find_format_by_extension(&MUXER_LIST, ext)
}

/// Context for managing muxer operations
pub struct MuxerContext<M: Muxer, W: Writer> {
    muxer: M,
    writer: W,
    /// Muxer state
    pub state: MuxerState,
}

impl<M: Muxer, W: Writer> MuxerContext<M, W> {
    /// Creates a new muxer context
    pub fn new(muxer: M, writer: W) -> Self {
        Self {
            muxer,
            writer,
            state: MuxerState::new(),
        }
    }

    /// Writes the container header
    pub fn write_header(&mut self) -> Result<()> {
        self.muxer.write_header(&mut self.writer, &self.state)
    }

    /// Writes a packet to the container
    pub fn write_packet(&mut self, packet: &Packet) -> Result<()> {
        self.muxer.write_packet(&mut self.writer, &self.state, packet)
    }

    /// Writes the container trailer
    pub fn write_trailer(&mut self) -> Result<()> {
        self.muxer.write_trailer(&mut self.writer)
    }
}
