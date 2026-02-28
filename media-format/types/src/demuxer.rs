//! Demuxer types and traits

use std::io::{BufRead, Seek};

use bitflags::bitflags;
use media_codec_types::packet::Packet;
use media_core::{variant::Variant, Result};

use crate::{
    format::{Format, FormatBuilder},
    stream::StreamCollection,
    track::TrackCollection,
};

/// Trait alias for types that can be used as readers
pub trait Reader: BufRead + Seek {}

impl<T: BufRead + Seek> Reader for T {}

bitflags! {
    /// Flags for seek operations
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SeekFlags: u32 {
        /// Seek to the nearest keyframe before the target timestamp
        const BACKWARD = 1;
        /// Seek to the exact position, even if it's not a keyframe
        const ANY = 2;
    }
}

/// State maintained by a demuxer during operation
pub struct DemuxerState {
    /// Start time of the media in microseconds
    pub start_time: Option<i64>,
    /// Duration of the media in microseconds
    pub duration: Option<i64>,
    /// Metadata from the container
    pub metadata: Option<Variant>,
    /// Collection of tracks in the container
    pub tracks: TrackCollection,
    /// Collection of streams in the container
    pub streams: StreamCollection,
}

impl DemuxerState {
    /// Creates a new demuxer state
    pub fn new() -> Self {
        Self {
            streams: StreamCollection::new(),
            tracks: TrackCollection::new(),
            start_time: None,
            duration: None,
            metadata: None,
        }
    }
}

impl Default for DemuxerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for demuxer implementations
pub trait Demuxer: Format {
    /// Reads the container header and populates the demuxer state
    fn read_header(&mut self, reader: &mut dyn Reader, state: &mut DemuxerState) -> Result<()>;

    /// Reads the next packet from the container
    fn read_packet(&mut self, reader: &mut dyn Reader, state: &DemuxerState) -> Result<Packet<'static>>;

    /// Seeks to the specified timestamp
    fn seek(&mut self, reader: &mut dyn Reader, state: &DemuxerState, track_index: Option<usize>, timestamp_us: i64, flags: SeekFlags) -> Result<()>;
}

/// Builder trait for creating demuxer instance
pub trait DemuxerBuilder: FormatBuilder {
    fn new_demuxer(&self) -> Result<Box<dyn Demuxer>>;

    /// Probe the data to determine if it matches this format
    fn probe(&self, reader: &mut dyn Reader) -> bool;
}
