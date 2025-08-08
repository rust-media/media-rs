use std::{borrow::Cow, num::NonZeroU32};

use crate::{
    error::Error,
    frame::{Frame, FrameData, MemoryData, PlaneDataVec, PlaneInformation, PlaneInformationVec, SeparateMemoryData},
    invalid_param_error,
    media::FrameDescriptor,
    video::{PixelFormat, VideoFrameDescriptor},
    Result, DEFAULT_ALIGNMENT,
};

pub struct VideoDataBuilder;

impl VideoDataBuilder {
    fn new(format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Result<MemoryData<'static>> {
        let (size, planes) = format.calc_data(width.get(), height.get(), DEFAULT_ALIGNMENT);

        Ok(MemoryData {
            data: vec![0; size as usize].into(),
            planes,
        })
    }

    fn from_buffer<'a, T>(format: PixelFormat, width: NonZeroU32, height: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let (size, planes) = format.calc_data(width.get(), height.get(), 1);
        let buffer = buffer.into();

        if buffer.len() != size as usize {
            return Err(Error::Invalid("buffer size".to_string()));
        }

        Ok(MemoryData {
            data: buffer,
            planes,
        })
    }

    fn from_aligned_buffer<'a, T>(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let (size, planes) = format.calc_data_with_stride(height.get(), stride.get());
        let buffer = buffer.into();

        if buffer.len() != size as usize {
            return Err(Error::Invalid("buffer size".to_string()));
        }

        let data = MemoryData {
            data: buffer,
            planes,
        };

        Ok(data)
    }

    fn from_packed_buffer<'a, T>(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        if !format.is_packed() {
            return Err(Error::Unsupported("format".to_string()));
        }

        let buffer = buffer.into();

        if buffer.len() != (stride.get() * height.get()) as usize {
            return Err(Error::Invalid("buffer size".to_string()));
        }

        let planes = PlaneInformationVec::from_slice(&[PlaneInformation::Video(stride.get(), height.get())]);

        let data = MemoryData {
            data: buffer,
            planes,
        };

        Ok(data)
    }
}

pub struct VideoFrameBuilder;

impl VideoFrameBuilder {
    pub fn new(&self, format: PixelFormat, width: u32, height: u32) -> Result<Frame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: VideoFrameDescriptor) -> Result<Frame<'static>> {
        let data = VideoDataBuilder::new(desc.format, desc.width, desc.height)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_buffer<'a, T>(&self, format: PixelFormat, width: u32, height: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.from_buffer_with_descriptor(desc, buffer)
    }

    pub fn from_buffer_with_descriptor<'a, T>(&self, desc: VideoFrameDescriptor, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataBuilder::from_buffer(desc.format, desc.width, desc.height, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_aligned_buffer<'a, T>(&self, format: PixelFormat, width: u32, height: u32, stride: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or(invalid_param_error!(stride))?;

        self.from_aligned_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn from_aligned_buffer_with_descriptor<'a, T>(&self, desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataBuilder::from_aligned_buffer(desc.format, desc.height, stride, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_packed_buffer<'a, T>(&self, format: PixelFormat, width: u32, height: u32, stride: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or(invalid_param_error!(stride))?;

        self.from_packed_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn from_packed_buffer_with_descriptor<'a, T>(&self, desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataBuilder::from_packed_buffer(desc.format, desc.height, stride, buffer)?;

        Ok(Self::from_data(desc, data))
    }

    pub fn from_buffers<'a>(&self, format: PixelFormat, width: u32, height: u32, buffers: &[(&'a [u8], u32)]) -> Result<Frame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.from_buffers_with_descriptor(desc, buffers)
    }

    pub fn from_buffers_with_descriptor<'a>(&self, desc: VideoFrameDescriptor, buffers: &[(&'a [u8], u32)]) -> Result<Frame<'a>> {
        let data = SeparateMemoryData::from_buffers(desc.format, desc.height, buffers)?;

        Ok(Frame::default(FrameDescriptor::Video(desc), FrameData::SeparateMemory(data)))
    }

    fn from_data<'a>(desc: VideoFrameDescriptor, data: MemoryData<'a>) -> Frame<'a> {
        Frame::default(FrameDescriptor::Video(desc), FrameData::Memory(data))
    }
}

impl<'a> SeparateMemoryData<'a> {
    fn from_buffers(format: PixelFormat, height: NonZeroU32, buffers: &[(&'a [u8], u32)]) -> Result<Self> {
        let mut data_vec = PlaneDataVec::with_capacity(buffers.len());

        for (i, (buffer, stride)) in buffers.iter().enumerate() {
            let height = format.calc_plane_height(i, height.get());

            if buffer.len() != (*stride as usize * height as usize) {
                return Err(Error::Invalid("buffer size".to_string()));
            }

            data_vec.push((*buffer, *stride, height));
        }

        Ok(Self {
            planes: data_vec,
        })
    }
}

impl Frame<'_> {
    pub fn video_builder() -> VideoFrameBuilder {
        VideoFrameBuilder
    }

    pub fn video_descriptor(&self) -> Option<&VideoFrameDescriptor> {
        if let FrameDescriptor::Video(desc) = &self.desc {
            Some(desc)
        } else {
            None
        }
    }

    pub fn is_video(&self) -> bool {
        self.desc.is_video()
    }
}
