use std::io::{BufRead, BufReader, Seek};

use bitflags::bitflags;
use media_codec::packet::Packet;
use media_core::{variant::Variant, Result};

use crate::{format::Format, stream::StreamCollection, track::TrackCollection};

pub trait Reader: BufRead + Seek {}

impl<R: BufRead + Seek> Reader for BufReader<R> {}

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SeekFlags: u32 {
        const BACKWARD = 1;
        const ANY = 2;
    }
}

pub struct DemuxerState {
    pub start_time: Option<i64>,
    pub duration: Option<i64>,
    pub metadata: Option<Variant>,
    pub tracks: TrackCollection,
    pub streams: StreamCollection,
}

impl DemuxerState {
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

pub trait Demuxer: Format {
    fn read_header<R: Reader>(&mut self, reader: &mut R, state: &mut DemuxerState) -> Result<()>;
    fn read_packet<R: Reader>(&mut self, reader: &mut R, state: &DemuxerState) -> Result<Packet<'_>>;
    fn seek<R: Reader>(
        &mut self,
        reader: &mut R,
        state: &DemuxerState,
        track_index: Option<usize>,
        timestamp_us: i64,
        flags: SeekFlags,
    ) -> Result<()>;
}

pub struct DemuxerContext<D: Demuxer, R: Reader> {
    demuxer: D,
    reader: R,
    pub max_delay: i64,
    pub state: DemuxerState,
}

impl<D: Demuxer, R: Reader> DemuxerContext<D, R> {
    pub fn new(demuxer: D, reader: R) -> Self {
        Self {
            demuxer,
            reader,
            max_delay: 0,
            state: DemuxerState::new(),
        }
    }

    pub fn read_header(&mut self) -> Result<()> {
        self.demuxer.read_header(&mut self.reader, &mut self.state)
    }

    pub fn read_packet(&mut self) -> Result<Packet<'_>> {
        self.demuxer.read_packet(&mut self.reader, &self.state)
    }

    pub fn seek(&mut self, track_index: Option<usize>, timestamp_us: i64, flags: SeekFlags) -> Result<()> {
        self.demuxer.seek(&mut self.reader, &self.state, track_index, timestamp_us, flags)
    }

    pub fn tracks(&self) -> &TrackCollection {
        &self.state.tracks
    }

    pub fn streams(&self) -> &StreamCollection {
        &self.state.streams
    }
}
