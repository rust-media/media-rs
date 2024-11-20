use std::{
    iter,
    num::{NonZeroU32, NonZeroU8},
};

use crate::{
    media_frame::{MemoryPlanes, PlaneInformation},
    time,
};

pub const SAMPLE_RATE_TELEPHONE: u32 = 8000;
pub const SAMPLE_RATE_VOIP: u32 = 16000;
pub const SAMPLE_RATE_CD: u32 = 44100;
pub const SAMPLE_RATE_DVD: u32 = 48000;
pub const SAMPLE_RATE_HIGH: u32 = 96000;
pub const SAMPLE_RATE_ULTRA_HIGH: u32 = 192000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioFormat {
    U8 = 0, // unsigned 8 bits
    S16,    // signed 16 bits
    S32,    // signed 32 bits
    F32,    // float 32 bits
    F64,    // float 64 bits
    U8P,    // unsigned 8 bits, planar
    S16P,   // signed 16 bits, planar
    S32P,   // signed 32 bits, planar
    F32P,   // float 32 bits, planar
    F64P,   // float 64 bits, planar
    MAX,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioFrameDescription {
    pub format: AudioFormat,
    pub channels: NonZeroU8,
    pub samples: NonZeroU32,
    pub sample_rate: NonZeroU32,
}

impl AudioFrameDescription {
    pub fn new(format: AudioFormat, channels: NonZeroU8, samples: NonZeroU32, sample_rate: NonZeroU32) -> Self {
        Self {
            format,
            channels,
            samples,
            sample_rate,
        }
    }

    pub fn duration_equal(&self, other: &AudioFrameDescription) -> bool {
        let duration1 = self.samples.get() as u64 * time::MSEC_PER_SEC / self.sample_rate.get() as u64;
        let duration2 = other.samples.get() as u64 * time::MSEC_PER_SEC / other.sample_rate.get() as u64;
        duration1 == duration2
    }
}

struct AudioFormatInfo {
    pub bits: u8,
    pub is_planar: bool,
}

static AUDIO_FORMAT_INFO: [AudioFormatInfo; AudioFormat::MAX as usize] = [
    // U8
    AudioFormatInfo {
        bits: 8,
        is_planar: false,
    },
    // S16
    AudioFormatInfo {
        bits: 16,
        is_planar: false,
    },
    // S32
    AudioFormatInfo {
        bits: 32,
        is_planar: false,
    },
    // F32
    AudioFormatInfo {
        bits: 32,
        is_planar: false,
    },
    // F64
    AudioFormatInfo {
        bits: 64,
        is_planar: false,
    },
    // U8P
    AudioFormatInfo {
        bits: 8,
        is_planar: true,
    },
    // S16P
    AudioFormatInfo {
        bits: 16,
        is_planar: true,
    },
    // S32P
    AudioFormatInfo {
        bits: 32,
        is_planar: true,
    },
    // F32P
    AudioFormatInfo {
        bits: 32,
        is_planar: true,
    },
    // F64P
    AudioFormatInfo {
        bits: 64,
        is_planar: true,
    },
];

impl AudioFormat {
    pub fn bits(&self) -> u8 {
        AUDIO_FORMAT_INFO[*self as usize].bits
    }

    pub fn bytes(&self) -> u8 {
        self.bits() >> 3
    }

    pub fn is_planar(&self) -> bool {
        AUDIO_FORMAT_INFO[*self as usize].is_planar
    }

    pub fn stride(&self, channels: u8, samples: u32) -> u32 {
        if self.is_planar() {
            self.bytes() as u32 * samples
        } else {
            self.bytes() as u32 * channels as u32 * samples
        }
    }

    pub(super) fn data_calc(&self, channels: u8, samples: u32) -> (u32, MemoryPlanes) {
        let mut size = 0;
        let mut planes = MemoryPlanes::new();
        let stride = self.stride(channels, samples);

        if self.is_planar() {
            planes.extend(iter::repeat(PlaneInformation::Audio(stride)).take(channels as usize));
            size += stride * channels as u32;
        } else {
            planes.push(PlaneInformation::Audio(stride));
            size = stride;
        }

        (size, planes)
    }
}
