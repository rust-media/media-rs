//! Stream types for media containers

use std::slice;

use media_core::variant::Variant;

/// Represents a stream within a media container
#[derive(Clone, Debug)]
pub struct Stream {
    index: usize,
    /// Stream ID as specified in the container format
    pub id: i64,
    /// Metadata associated with this stream
    pub metadata: Variant,
    /// Indices of tracks belonging to this stream
    pub tracks: Vec<usize>,
}

impl Stream {
    /// Creates a new stream with the given ID
    pub fn new(id: i64) -> Self {
        Self {
            index: 0,
            id,
            metadata: Variant::new_dict(),
            tracks: Vec::new(),
        }
    }

    /// Returns the index of this stream in its collection
    pub fn index(&self) -> usize {
        self.index
    }

    /// Adds a track index to this stream
    pub fn add_track(&mut self, track_index: usize) {
        self.tracks.push(track_index);
    }
}

/// A collection of streams within a media container
#[derive(Clone, Debug)]
pub struct StreamCollection {
    streams: Vec<Stream>,
}

impl Default for StreamCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamCollection {
    /// Creates a new empty stream collection
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
        }
    }

    /// Adds a stream to the collection and returns its index
    pub fn add_stream(&mut self, mut stream: Stream) -> usize {
        let index = self.streams.len();
        stream.index = index;
        self.streams.push(stream);
        index
    }

    /// Gets a stream by index
    pub fn get_stream(&self, index: usize) -> Option<&Stream> {
        self.streams.get(index)
    }

    /// Returns the number of streams in the collection
    pub fn len(&self) -> usize {
        self.streams.len()
    }

    /// Returns true if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }
}

impl<'a> IntoIterator for &'a StreamCollection {
    type Item = &'a Stream;
    type IntoIter = slice::Iter<'a, Stream>;

    fn into_iter(self) -> Self::IntoIter {
        self.streams.iter()
    }
}
