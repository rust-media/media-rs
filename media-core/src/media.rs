#[cfg(feature = "audio")]
use crate::audio::AudioFrameDescriptor;
#[cfg(feature = "video")]
use crate::video::VideoFrameDescriptor;
use crate::{data::DataFrameDescriptor, frame::Frame, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaType {
    Audio = 0,
    Video,
    Data,
}

pub trait FrameDescriptorSpec: Clone + PartialEq + Send + Sync + Into<FrameDescriptor> + 'static {
    fn media_type(&self) -> MediaType;
    fn create_frame(&self) -> Result<Frame<'static, Self>>;
    #[cfg(feature = "audio")]
    fn as_audio(&self) -> Option<&AudioFrameDescriptor> {
        None
    }
    #[cfg(feature = "video")]
    fn as_video(&self) -> Option<&VideoFrameDescriptor> {
        None
    }
    fn as_data(&self) -> Option<&DataFrameDescriptor> {
        None
    }
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
    pub fn as_audio(&self) -> Option<&AudioFrameDescriptor> {
        if let FrameDescriptor::Audio(desc) = self {
            Some(desc)
        } else {
            None
        }
    }

    #[cfg(feature = "video")]
    pub fn as_video(&self) -> Option<&VideoFrameDescriptor> {
        if let FrameDescriptor::Video(desc) = self {
            Some(desc)
        } else {
            None
        }
    }

    pub fn as_data(&self) -> Option<&DataFrameDescriptor> {
        #[allow(irrefutable_let_patterns)]
        if let FrameDescriptor::Data(desc) = self {
            Some(desc)
        } else {
            None
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

impl FrameDescriptorSpec for FrameDescriptor {
    fn media_type(&self) -> MediaType {
        self.media_type()
    }

    fn create_frame(&self) -> Result<Frame<'static, Self>> {
        Frame::new_with_generic_descriptor(self.clone())
    }

    #[cfg(feature = "audio")]
    fn as_audio(&self) -> Option<&AudioFrameDescriptor> {
        self.as_audio()
    }

    #[cfg(feature = "video")]
    fn as_video(&self) -> Option<&VideoFrameDescriptor> {
        self.as_video()
    }

    fn as_data(&self) -> Option<&DataFrameDescriptor> {
        self.as_data()
    }
}
