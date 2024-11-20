use std::num::NonZeroU32;

use crate::{
    error::MediaError,
    invalid_param_error,
    media::MediaFrameType,
    media_frame::{Data, MediaFrame, MediaFrameData, MediaFrameDescription, MemoryData, MemoryPlanes, PlaneInformation},
    video::{PixelFormat, VideoFrameDescription},
    DEFAULT_ALIGNMENT,
};

impl<'a> MemoryData<'a> {
    pub fn new_video_data(format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Result<Self, MediaError> {
        let (size, planes) = format.calc_data(width.get(), height.get(), DEFAULT_ALIGNMENT);

        Ok(Self {
            data: Data::Owned(vec![0; size as usize]),
            planes,
        })
    }

    pub fn attach_video_data(format: PixelFormat, width: NonZeroU32, height: NonZeroU32, buffer: &'a [u8]) -> Result<Self, MediaError> {
        let (size, planes) = format.calc_data(width.get(), height.get(), 1);

        if buffer.len() != size as usize {
            return Err(MediaError::Invalid("buffer size".to_string()).into());
        }

        Ok(Self {
            data: Data::Borrowed(buffer),
            planes,
        })
    }

    pub fn attach_video_data_with_stride(
        format: PixelFormat,
        width: NonZeroU32,
        height: NonZeroU32,
        stride: NonZeroU32,
        buffer: &'a [u8],
    ) -> Result<Self, MediaError> {
        if stride.get() < width.get() {
            return Err(invalid_param_error!(stride).into());
        }

        let (size, planes) = format.calc_data_with_stride(height.get(), stride.get());

        if buffer.len() != size as usize {
            return Err(MediaError::Invalid("buffer size".to_string()).into());
        }

        let data = Self {
            data: Data::Borrowed(buffer),
            planes,
        };

        Ok(data)
    }

    pub fn attach_packed_video_data(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: &'a [u8]) -> Result<Self, MediaError> {
        if !format.is_packed() {
            return Err(MediaError::Unsupported("format".to_string()).into());
        }

        if buffer.len() != (stride.get() * height.get()) as usize {
            return Err(MediaError::Invalid("buffer size".to_string()).into());
        }

        let planes = MemoryPlanes::from_slice(&[PlaneInformation::Video(stride.get(), height.get())]);

        let data = Self {
            data: Data::Borrowed(buffer),
            planes,
        };

        Ok(data)
    }
}

impl<'a> MediaFrame<'a> {
    pub fn new_video_frame(desc: VideoFrameDescription) -> Result<Self, MediaError> {
        let data = MemoryData::new_video_data(desc.format, desc.width, desc.height)?;

        Ok(Self {
            media_type: MediaFrameType::Video,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescription::Video(desc),
            metadata: None,
            data: MediaFrameData::Memory(data),
        })
    }

    fn from_data(desc: VideoFrameDescription, data: MemoryData<'a>) -> Self {
        Self {
            media_type: MediaFrameType::Video,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescription::Video(desc),
            metadata: None,
            data: MediaFrameData::Memory(data),
        }
    }

    pub fn from_data_buffer(desc: VideoFrameDescription, buffer: &'a [u8]) -> Result<Self, MediaError> {
        let data = MemoryData::attach_video_data(desc.format, desc.width, desc.height, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_data_buffer_with_stride(desc: VideoFrameDescription, stride: NonZeroU32, buffer: &'a [u8]) -> Result<Self, MediaError> {
        let data = MemoryData::attach_video_data_with_stride(desc.format, desc.width, desc.height, stride, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_packed_data_buffer(desc: VideoFrameDescription, stride: NonZeroU32, buffer: &'a [u8]) -> Result<Self, MediaError> {
        let data = MemoryData::attach_packed_video_data(desc.format, desc.height, stride, buffer)?;

        Ok(Self::from_data(desc, data))
    }
}
