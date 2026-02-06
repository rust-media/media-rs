use std::{slice, sync::Arc};

use media_codec::{CodecID, CodecParameters};
use media_core::{buffer::BufferPool, rational::Rational64, variant::Variant, MediaType};

#[derive(Clone, Debug)]
pub struct Track {
    index: usize,
    pub id: isize,
    pub codec_id: CodecID,
    pub params: CodecParameters,
    pub start_time: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Rational64,
    pub metadata: Option<Variant>,
    pub pool: Arc<BufferPool>,
}

impl Track {
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

    pub fn media_type(&self) -> MediaType {
        self.codec_id.media_type()
    }
}

impl Track {
    pub fn index(&self) -> usize {
        self.index
    }
}

pub struct TrackCollection {
    tracks: Vec<Track>,
}

impl Default for TrackCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackCollection {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
        }
    }

    pub fn add_track(&mut self, mut track: Track) -> usize {
        let index = self.tracks.len();
        track.index = index;
        self.tracks.push(track);
        index
    }

    pub fn find_track(&self, id: isize) -> Option<&Track> {
        self.tracks.iter().find(|s| s.id == id)
    }

    pub fn get_track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

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
