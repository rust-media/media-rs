use std::{
    borrow::Cow,
    num::{NonZeroU32, NonZeroU8},
};

use crate::{
    audio::{AudioFrameDescriptor, SampleFormat},
    error::Error,
    frame::{Frame, FrameData, MemoryData},
    media::FrameDescriptor,
    Result,
};

pub struct AudioDataCreator;

impl AudioDataCreator {
    fn create(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32) -> Result<MemoryData<'static>> {
        let (size, planes) = format.calc_data(channels.get(), samples.get());
        let initial_value = if matches!(format, SampleFormat::U8 | SampleFormat::U8P) {
            0x80
        } else {
            0
        };

        Ok(MemoryData {
            data: vec![initial_value; size as usize].into(),
            planes,
        })
    }

    fn create_from_buffer<'a, T>(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32, buffer: T) -> Result<MemoryData<'a>>
    where
        T: Into<Cow<'a, [u8]>>,
    {
        let (size, planes) = format.calc_data(channels.get(), samples.get());
        let buffer = buffer.into();

        if buffer.len() != size as usize {
            return Err(Error::Invalid("buffer size".to_string()));
        }

        Ok(MemoryData {
            data: buffer,
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
}
