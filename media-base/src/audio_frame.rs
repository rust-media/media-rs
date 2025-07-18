use std::num::{NonZeroU32, NonZeroU8};

use super::{
    audio::{AudioFormat, AudioFrameDescriptor},
    error::MediaError,
    media::MediaFrameType,
    media_frame::{Data, MediaFrame, MediaFrameData, MediaFrameDescriptor, MemoryData},
};

pub struct AudioDataBuilder;

impl AudioDataBuilder {
    fn new(format: AudioFormat, channels: NonZeroU8, samples: NonZeroU32) -> Result<MemoryData<'static>, MediaError> {
        let (size, planes) = format.calc_data(channels.get(), samples.get());
        let initial_value = if matches!(format, AudioFormat::U8 | AudioFormat::U8P) {
            0x80
        } else {
            0
        };

        Ok(MemoryData {
            data: Data::Owned(vec![initial_value; size as usize]),
            planes,
        })
    }

    fn from_buffer<'a>(format: AudioFormat, channels: NonZeroU8, samples: NonZeroU32, buffer: &'a [u8]) -> Result<MemoryData<'a>, MediaError> {
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
    pub fn new(&self, format: AudioFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<MediaFrame<'static>, MediaError> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: AudioFrameDescriptor) -> Result<MediaFrame<'static>, MediaError> {
        let data = AudioDataBuilder::new(desc.format, desc.channels, desc.samples)?;

        Ok(MediaFrame {
            media_type: MediaFrameType::Audio,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescriptor::Audio(desc),
            metadata: None,
            data: MediaFrameData::Memory(data),
        })
    }

    pub fn from_buffer<'a>(&self, format: AudioFormat, channels: u8, samples: u32, sample_rate: u32, buffer: &'a [u8]) -> Result<MediaFrame<'a>, MediaError> {
        let desc = AudioFrameDescriptor::try_new(format, channels, samples, sample_rate)?;

        self.from_buffer_with_descriptor(desc, buffer)
    }

    pub fn from_buffer_with_descriptor<'a>(
        &self,
        desc: AudioFrameDescriptor,
        buffer: &'a [u8],
    ) -> Result<MediaFrame<'a>, MediaError> {
        let data = AudioDataBuilder::from_buffer(desc.format, desc.channels, desc.samples, buffer)?;

        Ok(MediaFrame {
            media_type: MediaFrameType::Audio,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescriptor::Audio(desc),
            metadata: None,
            data: MediaFrameData::Memory(data),
        })
    }
}

impl MediaFrame<'_> {
    pub fn audio_builder() -> AudioFrameBuilder {
        AudioFrameBuilder
    }
}
