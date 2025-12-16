use std::{
    borrow::Cow,
    num::{NonZeroU32, NonZeroU8},
};

use aligned_vec::avec;

use super::audio::{AudioFrameDescriptor, SampleFormat};
use crate::{
    error::Error,
    frame::{Data, Frame, FrameData, MemoryData},
    invalid_param_error, unsupported_error, FrameDescriptor, Result, DEFAULT_ALIGNMENT,
};

pub type AudioFrame<'a> = Frame<'a, AudioFrameDescriptor>;

pub struct AudioDataCreator;

impl AudioDataCreator {
    fn create(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32) -> Result<MemoryData<'static>> {
        let (size, planes) = format.calc_data_size(channels.get(), samples.get(), DEFAULT_ALIGNMENT as u32);
        let initial_value = if matches!(format, SampleFormat::U8 | SampleFormat::U8P) {
            0x80
        } else {
            0
        };

        Ok(MemoryData {
            data: Data::Owned(avec![[DEFAULT_ALIGNMENT]| initial_value; size]),
            planes,
        })
    }

    fn create_from_buffer<'a, T>(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let (size, planes) = format.calc_data_size(channels.get(), samples.get(), 1);
        let buffer = buffer.into();

        if buffer.len() != size {
            return Err(Error::Invalid("buffer size".to_string()));
        }

        Ok(MemoryData {
            data: buffer.into(),
            planes,
        })
    }
}

pub struct AudioFrameCreator;

impl AudioFrameCreator {
    pub fn create(&self, format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Frame<'static>> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.create_with_descriptor(desc)
    }

    pub fn create_with_descriptor(&self, desc: AudioFrameDescriptor) -> Result<Frame<'static>> {
        let data = AudioDataCreator::create(desc.format, desc.channels(), desc.samples)?;

        Ok(Frame::from_data(FrameDescriptor::Audio(desc), FrameData::Memory(data)))
    }

    pub fn create_from_buffer<'a, T>(&self, format: SampleFormat, channels: u8, samples: u32, sample_rate: u32, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.create_from_buffer_with_descriptor(desc, buffer)
    }

    pub fn create_from_buffer_with_descriptor<'a, T>(&self, desc: AudioFrameDescriptor, buffer: T) -> Result<Frame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = AudioDataCreator::create_from_buffer(desc.format, desc.channels(), desc.samples, buffer)?;

        Ok(Frame::from_data(FrameDescriptor::Audio(desc), FrameData::Memory(data)))
    }

    pub fn create_empty(&self, format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Frame<'static>> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.create_empty_with_descriptor(desc)
    }

    pub fn create_empty_with_descriptor(&self, desc: AudioFrameDescriptor) -> Result<Frame<'static>> {
        let data = FrameData::Empty;

        Ok(Frame::from_data(FrameDescriptor::Audio(desc), data))
    }
}

impl Frame<'_> {
    pub fn audio_creator() -> AudioFrameCreator {
        AudioFrameCreator
    }

    pub fn audio_descriptor(&self) -> Option<&AudioFrameDescriptor> {
        if let FrameDescriptor::Audio(desc) = &self.desc {
            Some(desc)
        } else {
            None
        }
    }

    pub fn is_audio(&self) -> bool {
        self.desc.is_audio()
    }

    pub fn truncate(&mut self, samples: u32) -> Result<()> {
        let FrameDescriptor::Audio(desc) = &mut self.desc else {
            return Err(unsupported_error!(self.desc));
        };

        AudioFrame::truncate_internal(desc, &mut self.data, samples)
    }
}

impl AudioFrame<'_> {
    pub fn new(format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Self> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        Self::new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(desc: AudioFrameDescriptor) -> Result<Self> {
        let data = AudioDataCreator::create(desc.format, desc.channels(), desc.samples)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Memory(data)))
    }

    pub fn from_buffer<'a, T>(format: SampleFormat, channels: u8, samples: u32, sample_rate: u32, buffer: T) -> Result<AudioFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        Self::from_buffer_with_descriptor(desc, buffer)
    }

    pub fn from_buffer_with_descriptor<'a, T>(desc: AudioFrameDescriptor, buffer: T) -> Result<AudioFrame<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let data = AudioDataCreator::create_from_buffer(desc.format, desc.channels(), desc.samples, buffer)?;

        Ok(Frame::from_data_with_generic_descriptor(desc, FrameData::Memory(data)))
    }

    pub fn new_empty(format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Self> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        Self::new_empty_with_descriptor(desc)
    }

    pub fn new_empty_with_descriptor(desc: AudioFrameDescriptor) -> Result<Self> {
        let data = FrameData::Empty;

        Ok(Frame::from_data_with_generic_descriptor(desc, data))
    }

    fn truncate_internal(desc: &mut AudioFrameDescriptor, data: &mut FrameData, samples: u32) -> Result<()> {
        if desc.samples.get() < samples || samples == 0 {
            return Err(invalid_param_error!(samples));
        }

        let actual_bytes = desc.format.calc_plane_size(desc.channels().get(), samples);
        data.truncate(actual_bytes)?;

        desc.samples = NonZeroU32::new(samples).unwrap();

        Ok(())
    }

    pub fn truncate(&mut self, samples: u32) -> Result<()> {
        Self::truncate_internal(&mut self.desc, &mut self.data, samples)
    }
}

impl<'a> From<AudioFrame<'a>> for Frame<'a> {
    fn from(frame: AudioFrame<'a>) -> Self {
        Frame {
            desc: FrameDescriptor::Audio(frame.desc),
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

impl<'a> TryFrom<Frame<'a>> for AudioFrame<'a> {
    type Error = crate::Error;

    fn try_from(frame: Frame<'a>) -> Result<Self> {
        if let FrameDescriptor::Audio(desc) = frame.desc {
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
            Err(crate::Error::Invalid("not audio frame".to_string()))
        }
    }
}
