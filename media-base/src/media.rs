use crate::{audio::AudioFrameDescriptor, data::DataFrameDescriptor, video::VideoFrameDescriptor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaFrameType {
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
    pub fn media_type(&self) -> MediaFrameType {
        match self {
            MediaFrameDescriptor::Audio(_) => MediaFrameType::Audio,
            MediaFrameDescriptor::Video(_) => MediaFrameType::Video,
            MediaFrameDescriptor::Data(_) => MediaFrameType::Data,
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
