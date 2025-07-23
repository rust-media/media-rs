use std::num::{NonZeroU32, NonZeroU8};

use crate::{
    audio::{AudioFrameDescriptor, SampleFormat},
    error::MediaError,
    media::MediaFrameDescriptor,
    media_frame::{Data, MediaFrame, MediaFrameData, MemoryData},
    Result,
};

pub struct AudioDataBuilder;

impl AudioDataBuilder {
    fn new(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32) -> Result<MemoryData<'static>> {
        let (size, planes) = format.calc_data(channels.get(), samples.get());
        let initial_value = if matches!(format, SampleFormat::U8 | SampleFormat::U8P) {
            0x80
        } else {
            0
        };

        Ok(MemoryData {
            data: Data::Owned(vec![initial_value; size as usize]),
            planes,
        })
    }

    fn from_buffer<'a>(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32, buffer: &'a [u8]) -> Result<MemoryData<'a>> {
        let (size, planes) = format.calc_data(channels.get(), samples.get());

        if buffer.len() != size as usize {
            return Err(MediaError::Invalid("buffer size".to_string()));
        }

        Ok(MemoryData {
            data: Data::Borrowed(buffer),
            planes,
        })
    }
}

pub struct AudioFrameBuilder;

impl AudioFrameBuilder {
    pub fn new(&self, format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<MediaFrame<'static>> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: AudioFrameDescriptor) -> Result<MediaFrame<'static>> {
        let data = AudioDataBuilder::new(desc.format, desc.channels(), desc.samples)?;

        Ok(MediaFrame::default(MediaFrameDescriptor::Audio(desc), MediaFrameData::Memory(data)))
    }

    pub fn from_buffer<'a>(&self, format: SampleFormat, channels: u8, samples: u32, sample_rate: u32, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.from_buffer_with_descriptor(desc, buffer)
    }

    pub fn from_buffer_with_descriptor<'a>(&self, desc: AudioFrameDescriptor, buffer: &'a [u8]) -> Result<MediaFrame<'a>> {
        let data = AudioDataBuilder::from_buffer(desc.format, desc.channels(), desc.samples, buffer)?;

        Ok(MediaFrame::default(MediaFrameDescriptor::Audio(desc), MediaFrameData::Memory(data)))
    }
}

impl MediaFrame<'_> {
    pub fn audio_builder() -> AudioFrameBuilder {
        AudioFrameBuilder
    }

    pub fn audio_descriptor(&self) -> Option<&AudioFrameDescriptor> {
        if let MediaFrameDescriptor::Audio(desc) = &self.desc {
            Some(desc)
        } else {
            None
        }
    }

    pub fn is_audio(&self) -> bool {
        self.desc.is_audio()
    }
}
