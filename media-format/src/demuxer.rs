//! Demuxer

use std::sync::{Arc, LazyLock, RwLock};

use media_codec_types::packet::Packet;
use media_core::{invalid_error, not_found_error, Result};
pub use media_format_types::demuxer::*;
use media_format_types::{stream::StreamCollection, track::TrackCollection};

use crate::format::{find_format_by_extension, register_format, FormatList, LazyFormatList};

/// Global demuxer registry
static DEMUXER_LIST: LazyFormatList<dyn DemuxerBuilder> = LazyLock::new(|| RwLock::new(FormatList::new()));

/// Registers a demuxer builder
pub fn register_demuxer(builder: Arc<dyn DemuxerBuilder>) -> Result<()> {
    register_format(&DEMUXER_LIST, builder)
}

/// Finds a demuxer builder by file extension
pub fn find_demuxer_by_extension(ext: &str) -> Result<Arc<dyn DemuxerBuilder>> {
    find_format_by_extension(&DEMUXER_LIST, ext)
}

/// Probes data to find the best matching demuxer
pub fn find_demuxer_by_probe(reader: &mut dyn Reader) -> Result<Arc<dyn DemuxerBuilder>> {
    let list = DEMUXER_LIST.read().map_err(|err| invalid_error!(err.to_string()))?;

    let mut best_match: Option<Arc<dyn DemuxerBuilder>> = None;

    // Iterate through all registered demuxers and find one that matches
    for builder in list.iter() {
        if builder.probe(reader) {
            best_match = Some(Arc::clone(builder));
            break;
        }
    }

    best_match.ok_or_else(|| not_found_error!("demuxer for data"))
}

/// Context for managing demuxer operations
pub struct DemuxerContext<R: Reader> {
    demuxer: Box<dyn Demuxer>,
    reader: R,
    /// Maximum delay for packet ordering
    pub max_delay: i64,
    /// Demuxer state
    pub state: DemuxerState,
}

impl<R: Reader> DemuxerContext<R> {
    /// Creates a new demuxer context
    pub fn new(reader: R, ext: &str) -> Result<Self> {
        let demuxer_builder = find_demuxer_by_extension(ext)?;
        let demuxer = demuxer_builder.new_demuxer()?;
        Ok(Self::new_with_demuxer(demuxer, reader))
    }

    pub fn new_with_demuxer(demuxer: Box<dyn Demuxer>, reader: R) -> Self {
        Self {
            demuxer,
            reader,
            max_delay: 0,
            state: DemuxerState::new(),
        }
    }

    /// Reads the container header
    pub fn read_header(&mut self) -> Result<()> {
        self.demuxer.read_header(&mut self.reader, &mut self.state)
    }

    /// Reads the next packet
    pub fn read_packet(&mut self) -> Result<Packet<'static>> {
        self.demuxer.read_packet(&mut self.reader, &self.state)
    }

    /// Seeks to the specified timestamp
    pub fn seek(&mut self, track_index: Option<usize>, timestamp_us: i64, flags: SeekFlags) -> Result<()> {
        self.demuxer.seek(&mut self.reader, &self.state, track_index, timestamp_us, flags)
    }

    /// Returns a reference to the track collection
    pub fn tracks(&self) -> &TrackCollection {
        &self.state.tracks
    }

    /// Returns a reference to the stream collection
    pub fn streams(&self) -> &StreamCollection {
        &self.state.streams
    }
}
