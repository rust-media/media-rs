#[cfg(feature = "audio")]
use crate::audio::AudioFrameDescriptor;
use crate::data::DataFrameDescriptor;
#[cfg(feature = "video")]
use crate::video::VideoFrameDescriptor;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaType {
    Audio = 0,
    Video,
    Data,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameDescriptor {
    #[cfg(feature = "audio")]
    Audio(AudioFrameDescriptor),
    #[cfg(feature = "video")]
    Video(VideoFrameDescriptor),
    Data(DataFrameDescriptor),
}

impl FrameDescriptor {
    pub fn media_type(&self) -> MediaType {
        match self {
            #[cfg(feature = "audio")]
            FrameDescriptor::Audio(_) => MediaType::Audio,
            #[cfg(feature = "video")]
            FrameDescriptor::Video(_) => MediaType::Video,
            FrameDescriptor::Data(_) => MediaType::Data,
        }
    }

    #[cfg(feature = "audio")]
    pub fn is_audio(&self) -> bool {
        matches!(self, FrameDescriptor::Audio(_))
    }

    #[cfg(feature = "video")]
    pub fn is_video(&self) -> bool {
        matches!(self, FrameDescriptor::Video(_))
    }

    pub fn is_data(&self) -> bool {
        matches!(self, FrameDescriptor::Data(_))
    }
}

#[deprecated = "Use 'FrameDescriptor' directly"]
pub type MediaFrameDescriptor = FrameDescriptor;
