use crate::{audio::AudioFrameDescriptor, data::DataFrameDescriptor, video::VideoFrameDescriptor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaType {
    Audio = 0,
    Video,
    Data,
}

#[derive(Clone, Debug)]
pub enum MediaFrameDescriptor {
    Audio(AudioFrameDescriptor),
    Video(VideoFrameDescriptor),
    Data(DataFrameDescriptor),
}

impl MediaFrameDescriptor {
    pub fn media_type(&self) -> MediaType {
        match self {
            MediaFrameDescriptor::Audio(_) => MediaType::Audio,
            MediaFrameDescriptor::Video(_) => MediaType::Video,
            MediaFrameDescriptor::Data(_) => MediaType::Data,
        }
    }

    pub fn is_audio(&self) -> bool {
        matches!(self, MediaFrameDescriptor::Audio(_))
    }

    pub fn is_video(&self) -> bool {
        matches!(self, MediaFrameDescriptor::Video(_))
    }

    pub fn is_data(&self) -> bool {
        matches!(self, MediaFrameDescriptor::Data(_))
    }
}
