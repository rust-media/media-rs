use std::{borrow::Cow, num::NonZeroU32, sync::Arc};

use aligned_vec::avec;

use super::video::{PixelFormat, VideoFrameDescriptor};
use crate::{
    buffer::Buffer,
    error::Error,
    frame::{BufferData, Data, Frame, FrameData, FrameSpec, MemoryData, PlaneDescriptor, PlaneVec, SeparateMemoryData},
    invalid_error, invalid_param_error, unsupported_error, FrameDescriptor, MediaType, Result, DEFAULT_ALIGNMENT,
};

pub type VideoFrame<'a> = Frame<'a, VideoFrameDescriptor>;

pub struct VideoDataCreator;

impl VideoDataCreator {
    fn create(format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Result<MemoryData<'static>> {
        let (size, planes) = format.calc_data_size(width.get(), height.get(), DEFAULT_ALIGNMENT as u32);

        Ok(MemoryData {
            data: Data::Owned(avec![[DEFAULT_ALIGNMENT]| 0u8; size]),
            planes,
        })
    }

    fn create_from_buffer<'a, T>(format: PixelFormat, width: NonZeroU32, height: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let (size, planes) = format.calc_data_size(width.get(), height.get(), 1);
        let buffer = buffer.into();

        if buffer.len() != size {
            return Err(invalid_error!("buffer size"));
        }

        Ok(MemoryData {
            data: buffer.into(),
            planes,
        })
    }

    fn create_from_aligned_buffer<'a, T>(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let (size, planes) = format.calc_data_size_with_stride(height.get(), stride.get());
        let buffer = buffer.into();

        if buffer.len() != size {
            return Err(invalid_error!("buffer size"));
        }

        let data = MemoryData {
            data: buffer.into(),
            planes,
        };

        Ok(data)
    }

    fn create_from_packed_buffer<'a, T>(format: PixelFormat, height: NonZeroU32, stride: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        if !format.is_packed() {
            return Err(unsupported_error!("format"));
        }

        let buffer = buffer.into();

        if buffer.len() != (stride.get() * height.get()) as usize {
            return Err(invalid_error!("buffer size"));
        }

        let planes = PlaneVec::from_slice(&[PlaneDescriptor::Video(stride.get() as usize, height.get())]);

        let data = MemoryData {
            data: buffer.into(),
            planes,
        };

        Ok(data)
    }

    fn create_from_shared_buffer(
        format: PixelFormat,
        height: NonZeroU32,
        buffer: Arc<Buffer>,
        buffer_planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<BufferData> {
        let mut planes = PlaneVec::with_capacity(buffer_planes.len());

        for (i, (offset, stride)) in buffer_planes.iter().enumerate() {
            let height = format.calc_plane_height(i, height.get());

            if *offset + (*stride as usize * height as usize) > buffer.len() {
                return Err(invalid_error!("buffer length"));
            }

            planes.push((*offset, PlaneDescriptor::Video(*stride as usize, height)));
        }

        Ok(BufferData {
            data: buffer.clone(),
            planes,
        })
    }
}

impl BufferData {
    fn attach_video_buffer(
        &mut self,
        format: PixelFormat,
        height: NonZeroU32,
        buffer: Arc<Buffer>,
        buffer_planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<()> {
        let mut planes = PlaneVec::with_capacity(buffer_planes.len());

        for (i, (offset, stride)) in buffer_planes.iter().enumerate() {
            let height = format.calc_plane_height(i, height.get());

            if *offset + (*stride as usize * height as usize) > buffer.len() {
                return Err(invalid_error!("buffer length"));
            }

            planes.push((*offset, PlaneDescriptor::Video(*stride as usize, height)));
        }

        self.data = buffer;
        self.planes = planes;

        Ok(())
    }
}

pub struct VideoFrameCreator;

impl VideoFrameCreator {
    pub fn create(&self, format: PixelFormat, width: u32, height: u32) -> Result<Frame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.create_with_descriptor(desc)
    }

    pub fn create_with_descriptor(&self, desc: VideoFrameDescriptor) -> Result<Frame<'static>> {
        let data = VideoDataCreator::create(desc.format, desc.width(), desc.height())?;

        Ok(Self::create_from_data(desc, data))
    }

    pub fn create_from_buffer<'a, T>(&self, format: PixelFormat, width: u32, height: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.create_from_buffer_with_descriptor(desc, buffer)
    }

    pub fn create_from_buffer_with_descriptor<'a, T>(&self, desc: VideoFrameDescriptor, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataCreator::create_from_buffer(desc.format, desc.width(), desc.height(), buffer)?;

        Ok(Self::create_from_data(desc, data))
    }

    pub fn create_from_aligned_buffer<'a, T>(&self, format: PixelFormat, width: u32, height: u32, stride: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or_else(|| invalid_param_error!(stride))?;

        self.create_from_aligned_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn create_from_aligned_buffer_with_descriptor<'a, T>(&self, desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataCreator::create_from_aligned_buffer(desc.format, desc.height(), stride, buffer)?;

        Ok(Self::create_from_data(desc, data))
    }

    pub fn create_from_packed_buffer<'a, T>(&self, format: PixelFormat, width: u32, height: u32, stride: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or_else(|| invalid_param_error!(stride))?;

        self.create_from_packed_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn create_from_packed_buffer_with_descriptor<'a, T>(&self, desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataCreator::create_from_packed_buffer(desc.format, desc.height(), stride, buffer)?;

        Ok(Self::create_from_data(desc, data))
    }

    pub fn create_from_buffers<'a>(&self, format: PixelFormat, width: u32, height: u32, buffers: &[(&'a [u8], u32)]) -> Result<Frame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.create_from_buffers_with_descriptor(desc, buffers)
    }

    pub fn create_from_buffers_with_descriptor<'a>(&self, desc: VideoFrameDescriptor, buffers: &[(&'a [u8], u32)]) -> Result<Frame<'a>> {
        let data = SeparateMemoryData::from_buffers(desc.format, desc.height(), buffers)?;

        Ok(Frame::from_data(FrameDescriptor::Video(desc), FrameData::SeparateMemory(data)))
    }

    pub fn create_from_shared_buffer(
        &self,
        format: PixelFormat,
        width: u32,
        height: u32,
        buffer: Arc<Buffer>,
        planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<Frame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.create_from_shared_buffer_with_descriptor(desc, buffer, planes)
    }

    pub fn create_from_shared_buffer_with_descriptor(
        &self,
        desc: VideoFrameDescriptor,
        buffer: Arc<Buffer>,
        planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<Frame<'static>> {
        let data = VideoDataCreator::create_from_shared_buffer(desc.format, desc.height(), buffer, planes)?;

        Ok(Frame::from_data(FrameDescriptor::Video(desc), FrameData::Buffer(data)))
    }

    pub fn create_empty(&self, format: PixelFormat, width: u32, height: u32) -> Result<Frame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.create_empty_with_descriptor(desc)
    }

    pub fn create_empty_with_descriptor(&self, desc: VideoFrameDescriptor) -> Result<Frame<'static>> {
        let data = FrameData::Empty;

        Ok(Frame::from_data(FrameDescriptor::Video(desc), data))
    }

    fn create_from_data(desc: VideoFrameDescriptor, data: MemoryData<'_>) -> Frame<'_> {
        Frame::from_data(FrameDescriptor::Video(desc), FrameData::Memory(data))
    }
}

impl<'a> SeparateMemoryData<'a> {
    fn from_buffers(format: PixelFormat, height: NonZeroU32, buffers: &[(&'a [u8], u32)]) -> Result<Self> {
        let mut data_vec = PlaneVec::with_capacity(buffers.len());

        for (i, (buffer, stride)) in buffers.iter().enumerate() {
            let height = format.calc_plane_height(i, height.get());

            if buffer.len() != (*stride as usize * height as usize) {
                return Err(invalid_error!("buffer size"));
            }

            data_vec.push((*buffer, *stride as usize, height));
        }

        Ok(Self {
            planes: data_vec,
        })
    }
}

impl Frame<'_> {
    pub fn video_creator() -> VideoFrameCreator {
        VideoFrameCreator
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

    pub fn attach_video_shared_buffer(
        &mut self,
        format: PixelFormat,
        width: u32,
        height: u32,
        buffer: Arc<Buffer>,
        buffer_planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<()> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.attach_video_shared_buffer_with_descriptor(desc, buffer, buffer_planes)
    }

    pub fn attach_video_shared_buffer_with_descriptor(
        &mut self,
        desc: VideoFrameDescriptor,
        buffer: Arc<Buffer>,
        buffer_planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<()> {
        match &mut self.data {
            FrameData::Buffer(data) => {
                data.attach_video_buffer(desc.format, desc.height(), buffer, buffer_planes)?;
            }
            FrameData::Empty => {
                let buffer_data = VideoDataCreator::create_from_shared_buffer(desc.format, desc.height(), buffer, buffer_planes)?;
                self.data = FrameData::Buffer(buffer_data);
            }
            _ => {
                return Err(invalid_error!("frame data type"));
            }
        }

        self.desc = FrameDescriptor::Video(desc);

        Ok(())
    }
}

impl VideoFrame<'_> {
    pub fn new(format: PixelFormat, width: u32, height: u32) -> Result<Self> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        Self::new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(desc: VideoFrameDescriptor) -> Result<Self> {
        let data = VideoDataCreator::create(desc.format, desc.width(), desc.height())?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Memory(data)))
    }

    pub fn from_buffer<'a, T>(format: PixelFormat, width: u32, height: u32, buffer: T) -> Result<VideoFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        Self::from_buffer_with_descriptor(desc, buffer)
    }

    pub fn from_buffer_with_descriptor<'a, T>(desc: VideoFrameDescriptor, buffer: T) -> Result<VideoFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataCreator::create_from_buffer(desc.format, desc.width(), desc.height(), buffer)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Memory(data)))
    }

    pub fn from_aligned_buffer<'a, T>(format: PixelFormat, width: u32, height: u32, stride: u32, buffer: T) -> Result<VideoFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or_else(|| invalid_param_error!(stride))?;

        Self::from_aligned_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn from_aligned_buffer_with_descriptor<'a, T>(desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: T) -> Result<VideoFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataCreator::create_from_aligned_buffer(desc.format, desc.height(), stride, buffer)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Memory(data)))
    }

    pub fn from_packed_buffer<'a, T>(format: PixelFormat, width: u32, height: u32, stride: u32, buffer: T) -> Result<VideoFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;
        let stride = NonZeroU32::new(stride).ok_or_else(|| invalid_param_error!(stride))?;

        Self::from_packed_buffer_with_descriptor(desc, stride, buffer)
    }

    pub fn from_packed_buffer_with_descriptor<'a, T>(desc: VideoFrameDescriptor, stride: NonZeroU32, buffer: T) -> Result<VideoFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = VideoDataCreator::create_from_packed_buffer(desc.format, desc.height(), stride, buffer)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Memory(data)))
    }

    pub fn from_buffers<'a>(format: PixelFormat, width: u32, height: u32, buffers: &[(&'a [u8], u32)]) -> Result<VideoFrame<'a>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        Self::from_buffers_with_descriptor(desc, buffers)
    }

    pub fn from_buffers_with_descriptor<'a>(desc: VideoFrameDescriptor, buffers: &[(&'a [u8], u32)]) -> Result<VideoFrame<'a>> {
        let data = SeparateMemoryData::from_buffers(desc.format, desc.height(), buffers)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::SeparateMemory(data)))
    }

    pub fn from_shared_buffer(
        format: PixelFormat,
        width: u32,
        height: u32,
        buffer: Arc<Buffer>,
        planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<VideoFrame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        Self::from_shared_buffer_with_descriptor(desc, buffer, planes)
    }

    pub fn from_shared_buffer_with_descriptor(
        desc: VideoFrameDescriptor,
        buffer: Arc<Buffer>,
        planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<VideoFrame<'static>> {
        let data = VideoDataCreator::create_from_shared_buffer(desc.format, desc.height(), buffer, planes)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Buffer(data)))
    }

    pub fn new_empty(format: PixelFormat, width: u32, height: u32) -> Result<VideoFrame<'static>> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        Self::new_empty_with_descriptor(desc)
    }

    pub fn new_empty_with_descriptor(desc: VideoFrameDescriptor) -> Result<VideoFrame<'static>> {
        let data = FrameData::Empty;

        Ok(Frame::from_data_with_generic_descriptor(desc, data))
    }

    pub fn attach_shared_buffer(
        &mut self,
        format: PixelFormat,
        width: u32,
        height: u32,
        buffer: Arc<Buffer>,
        buffer_planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<()> {
        let desc = VideoFrameDescriptor::try_new(format, width, height)?;

        self.attach_shared_buffer_with_descriptor(desc, buffer, buffer_planes)
    }

    pub fn attach_shared_buffer_with_descriptor(
        &mut self,
        desc: VideoFrameDescriptor,
        buffer: Arc<Buffer>,
        buffer_planes: &[(usize, u32)], // (offset, stride), offset from the start of the Buffer
    ) -> Result<()> {
        match &mut self.data {
            FrameData::Buffer(data) => {
                data.attach_video_buffer(desc.format, desc.height(), buffer, buffer_planes)?;
            }
            FrameData::Empty => {
                let buffer_data = VideoDataCreator::create_from_shared_buffer(desc.format, desc.height(), buffer, buffer_planes)?;
                self.data = FrameData::Buffer(buffer_data);
            }
            _ => {
                return Err(invalid_error!("frame data type"));
            }
        }

        self.desc = desc;

        Ok(())
    }
}

impl<'a> From<VideoFrame<'a>> for Frame<'a> {
    fn from(frame: VideoFrame<'a>) -> Self {
        Frame {
            desc: FrameDescriptor::Video(frame.desc),
            source: frame.source,
            pts: frame.pts,
            dts: frame.dts,
            duration: frame.duration,
            time_base: frame.time_base,
            metadata: frame.metadata,
            data: frame.data,
        }
    }
}

impl<'a> TryFrom<Frame<'a>> for VideoFrame<'a> {
    type Error = Error;

    fn try_from(frame: Frame<'a>) -> Result<Self> {
        if let FrameDescriptor::Video(desc) = frame.desc {
            Ok(Frame {
                desc,
                source: frame.source,
                pts: frame.pts,
                dts: frame.dts,
                duration: frame.duration,
                time_base: frame.time_base,
                metadata: frame.metadata,
                data: frame.data,
            })
        } else {
            Err(invalid_error!("not video frame"))
        }
    }
}

impl FrameSpec<VideoFrameDescriptor> for VideoFrame<'_> {
    fn new_with_descriptor(desc: VideoFrameDescriptor) -> Result<Frame<'static, VideoFrameDescriptor>> {
        VideoFrame::new_with_descriptor(desc)
    }

    fn media_type(&self) -> MediaType {
        MediaType::Video
    }
}
