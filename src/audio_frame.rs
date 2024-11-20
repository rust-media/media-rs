use std::num::{NonZeroU32, NonZeroU8};

use crate::{
    audio::{AudioFormat, AudioFrameDescription},
    error::MediaError,
    media::MediaFrameType,
    media_frame::{Data, MediaFrame, MediaFrameData, MediaFrameDescription, MemoryData},
};

impl<'a> MemoryData<'a> {
    pub fn new_audio_data(format: AudioFormat, channels: NonZeroU8, samples: NonZeroU32) -> Result<Self, MediaError> {
        let (size, planes) = format.data_calc(channels.get(), samples.get());
        let initial_value = if matches!(format, AudioFormat::U8 | AudioFormat::U8P) {
            0x80
        } else {
            0
        };

        Ok(Self {
            data: Data::Owned(vec![initial_value; size as usize]),
            planes,
        })
    }
}

impl<'a> MediaFrame<'a> {
    pub fn new_audio_frame(desc: AudioFrameDescription) -> Result<Self, MediaError> {
        let data = MemoryData::new_audio_data(desc.format, desc.channels, desc.samples)?;

        Ok(Self {
            media_type: MediaFrameType::Audio,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescription::Audio(desc),
            metadata: None,
            data: MediaFrameData::Memory(data),
        })
    }
}
