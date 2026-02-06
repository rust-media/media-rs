use std::slice;

use media_core::variant::Variant;

pub struct Stream {
    index: usize,
    pub id: i64,
    pub metadata: Variant,
    pub tracks: Vec<usize>,
}

impl Stream {
    pub fn new(id: i64) -> Self {
        Self {
            index: 0,
            id,
            metadata: Variant::new_dict(),
            tracks: Vec::new(),
        }
    }

    pub fn add_track(&mut self, track_index: usize) {
        self.tracks.push(track_index);
    }
}

pub struct StreamCollection {
    streams: Vec<Stream>,
}

impl Default for StreamCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamCollection {
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
        }
    }

    pub fn add_stream(&mut self, mut stream: Stream) -> usize {
        let index = self.streams.len();
        stream.index = index;
        self.streams.push(stream);
        index
    }

    pub fn get_stream(&self, index: usize) -> Option<&Stream> {
        self.streams.get(index)
    }

    pub fn len(&self) -> usize {
        self.streams.len()
    }

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
