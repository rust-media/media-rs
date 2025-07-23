use std::num::NonZeroU32;

use crate::{
    error::MediaError,
    invalid_param_error,
    media::MediaFrameDescriptor,
    media_frame::{Data, MediaFrame, MediaFrameData, MemoryData, PlaneDataVec, PlaneInformation, PlaneInformationVec, SeparateMemoryData},
    video::{PixelFormat, VideoFrameDescriptor},
    Result, DEFAULT_ALIGNMENT,
};

pub struct VideoDataBuilder;

impl VideoDataBuilder {
    fn new(format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Result<MemoryData<'static>> {
        let (size, planes) = format.calc_data(width.get(), height.get(), DEFAULT_ALIGNMENT);

        Ok(MemoryData {
            data: Data::Owned(vec![0; size as usize]),
            planes,
        })
    }

    fn from_buffer<'a>(format: PixelFormat, width: NonZeroU32, height: NonZeroU32, buffer: &'a [u8]) -> Result<MemoryData<'a>> {
        let (size, planes) = format.calc_data(width.get(), height.get(), 1);

        if buffer.len() != size as usize {
            return Err(MediaError::Invalid("buffer size".to_string()));
        }

        Ok(MemoryData {
            data: Data::Borrowed(buffer),
            planes,
        })
    }

    fn from_aligned_buffer<'a>(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: &'a [u8]) -> Result<MemoryData<'a>> {
        let (size, planes) = format.calc_data_with_stride(height.get(), stride.get());

        if buffer.len() != size as usize {
            return Err(MediaError::Invalid("buffer size".to_string()));
        }

        let data = MemoryData {
            data: Data::Borrowed(buffer),
            planes,
        };

        Ok(data)
    }

    fn from_packed_buffer<'a>(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: &'a [u8]) -> Result<MemoryData<'a>> {
        if !format.is_packed() {
            return Err(MediaError::Unsupported("format".to_string()));
        }

        if buffer.len() != (stride.get() * height.get()) as usize {
            return Err(MediaError::Invalid("buffer size".to_string()));
        }

        let planes = PlaneInformationVec::from_slice(&[PlaneInformation::Video(stride.get(), height.get())]);

        let data = MemoryData {
            data: Data::Borrowed(buffer),
            planes,
        };

        Ok(data)
    }
}

pub struct VideoFrameBuilder;

impl VideoFrameBuilder {
    pub fn new(&self, format: PixelFormat, width: u32, height: u32) -> Result<MediaFrame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: VideoFrameDescriptor) -> Result<MediaFrame<'static>> {
        let data = VideoDataBuilder::new(desc.format, desc.width, desc.height)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_buffer<'a>(&self, format: PixelFormat, width: u32, height: u32, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.from_buffer_with_descriptor(desc, buffer)
    }

    pub fn from_buffer_with_descriptor<'a>(&self, desc: VideoFrameDescriptor, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let data = VideoDataBuilder::from_buffer(desc.format, desc.width, desc.height, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_aligned_buffer<'a>(&self, format: PixelFormat, width: u32, height: u32, stride: u32, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or(invalid_param_error!(stride))?;

        self.from_aligned_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn from_aligned_buffer_with_descriptor<'a>(
        &self,
        desc: VideoFrameDescriptor,
        stride: NonZeroU32,
        buffer: &'a [u8],
    ) -> Result<MediaFrame<'a>> {
        let data = VideoDataBuilder::from_aligned_buffer(desc.format, desc.height, stride, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_packed_buffer<'a>(&self, format: PixelFormat, width: u32, height: u32, stride: u32, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or(invalid_param_error!(stride))?;

        self.from_packed_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn from_packed_buffer_with_descriptor<'a>(&self, desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let data = VideoDataBuilder::from_packed_buffer(desc.format, desc.height, stride, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_buffers<'a>(&self, format: PixelFormat, width: u32, height: u32, buffers: &[(&'a [u8], u32)]) -> Result<MediaFrame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.from_buffers_with_descriptor(desc, buffers)
    }

    pub fn from_buffers_with_descriptor<'a>(&self, desc: VideoFrameDescriptor, buffers: &[(&'a [u8], u32)]) -> Result<MediaFrame<'a>> {
        let data = SeparateMemoryData::from_buffers(desc.format, desc.height, buffers)?;

        Ok(MediaFrame::default(MediaFrameDescriptor::Video(desc), MediaFrameData::SeparateMemory(data)))
    }

    fn from_data<'a>(desc: VideoFrameDescriptor, data: MemoryData<'a>) -> MediaFrame<'a> {
        MediaFrame::default(MediaFrameDescriptor::Video(desc), MediaFrameData::Memory(data))
    }
}

impl<'a> SeparateMemoryData<'a> {
    fn from_buffers(format: PixelFormat, height: NonZeroU32, buffers: &[(&'a [u8], u32)]) -> Result<Self> {
        let mut data_vec = PlaneDataVec::with_capacity(buffers.len());

        for (i, (buffer, stride)) in buffers.iter().enumerate() {
            let height = format.calc_plane_height(i, height.get());

            if buffer.len() != (*stride as usize * height as usize) {
                return Err(MediaError::Invalid("buffer size".to_string()));
            }

            data_vec.push((*buffer, *stride, height));
        }

        Ok(Self {
            planes: data_vec,
        })
    }
}

impl MediaFrame<'_> {
    pub fn video_builder() -> VideoFrameBuilder {
        VideoFrameBuilder
    }

    pub fn is_video(&self) -> bool {
        self.desc.is_video()
    }
}
