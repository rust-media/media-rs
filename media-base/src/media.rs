use crate::{audio::AudioFrameDescriptor, data::DataFrameDescriptor, video::VideoFrameDescriptor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaType {
    Audio = 0,
    Video,
    Data,
}

#[derive(Clone, Debug)]
pub enum FrameDescriptor {
    Audio(AudioFrameDescriptor),
    Video(VideoFrameDescriptor),
    Data(DataFrameDescriptor),
}

impl FrameDescriptor {
    pub fn media_type(&self) -> MediaType {
        match self {
            FrameDescriptor::Audio(_) => MediaType::Audio,
            FrameDescriptor::Video(_) => MediaType::Video,
            FrameDescriptor::Data(_) => MediaType::Data,
        }
    }

    pub fn is_audio(&self) -> bool {
        matches!(self, FrameDescriptor::Audio(_))
    }

    pub fn is_video(&self) -> bool {
        matches!(self, FrameDescriptor::Video(_))
    }

    pub fn is_data(&self) -> bool {
        matches!(self, FrameDescriptor::Data(_))
    }
}

#[deprecated = "Use 'FrameDescriptor' directly"]
pub type MediaFrameDescriptor = FrameDescriptor;
