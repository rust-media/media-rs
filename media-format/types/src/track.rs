//! Track types for media containers

use std::{slice, sync::Arc};

use media_codec_types::{CodecID, CodecParameters};
use media_core::{buffer::BufferPool, rational::Rational64, variant::Variant, MediaType};

/// Represents a media track within a container
#[derive(Clone, Debug)]
pub struct Track {
    index: usize,
    /// Track ID as specified in the container format
    pub id: isize,
    /// Codec identifier for this track
    pub codec_id: CodecID,
    /// Codec parameters for this track
    pub params: CodecParameters,
    /// Start time in track's time base units
    pub start_time: Option<i64>,
    /// Duration in track's time base units
    pub duration: Option<i64>,
    /// Time base for timestamps in this track
    pub time_base: Rational64,
    /// Metadata associated with this track
    pub metadata: Option<Variant>,
    /// Buffer pool for allocating packet buffers
    pub pool: Arc<BufferPool>,
}

impl Track {
    /// Creates a new track with the given parameters
    pub fn new(id: isize, codec_id: CodecID, params: CodecParameters, time_base: Rational64) -> Self {
        Self {
            index: 0,
            id,
            codec_id,
            params,
            start_time: None,
            duration: None,
            time_base,
            metadata: None,
            pool: BufferPool::new(0),
        }
    }

    /// Returns the media type of this track
    pub fn media_type(&self) -> MediaType {
        self.codec_id.media_type()
    }

    /// Returns the index of this track in its collection
    pub fn index(&self) -> usize {
        self.index
    }
}

/// A collection of tracks within a media container
#[derive(Clone, Debug)]
pub struct TrackCollection {
    tracks: Vec<Track>,
}

impl Default for TrackCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackCollection {
    /// Creates a new empty track collection
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
        }
    }

    /// Adds a track to the collection and returns its index
    pub fn add_track(&mut self, mut track: Track) -> usize {
        let index = self.tracks.len();
        track.index = index;
        self.tracks.push(track);
        index
    }

    /// Finds a track by its ID
    pub fn find_track(&self, id: isize) -> Option<&Track> {
        self.tracks.iter().find(|s| s.id == id)
    }

    /// Gets a track by index
    pub fn get_track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    /// Returns the number of tracks in the collection
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Returns true if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

impl<'a> IntoIterator for &'a TrackCollection {
    type Item = &'a Track;
    type IntoIter = slice::Iter<'a, Track>;

    fn into_iter(self) -> Self::IntoIter {
        self.tracks.iter()
    }
}
