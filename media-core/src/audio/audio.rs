use std::{
    fmt::{Display, Formatter},
    iter, mem,
    num::{NonZeroU32, NonZeroU8},
};

use strum::EnumCount;

pub use super::channel_layout::*;
use crate::{
    align_to,
    audio::AudioFrame,
    error::Error,
    frame::{Frame, PlaneDescriptor, PlaneVec},
    invalid_param_error, time, FrameDescriptor, FrameDescriptorSpec, MediaType, Result,
};

pub const DEFAULT_MAX_CHANNELS: usize = 16;

// Common audio sample rate constants
/// 8 kHz - Telephone quality
pub const SAMPLE_RATE_8K: u32 = 8_000;
pub const SAMPLE_RATE_TELEPHONE: u32 = SAMPLE_RATE_8K;
/// 11.025 kHz - 1/4 CD sample rate
pub const SAMPLE_RATE_11025: u32 = 11_025;
/// 16 kHz - VoIP/Wideband speech
pub const SAMPLE_RATE_16K: u32 = 16_000;
pub const SAMPLE_RATE_VOIP: u32 = SAMPLE_RATE_16K;
/// 22.05 kHz - 1/2 CD sample rate
pub const SAMPLE_RATE_22050: u32 = 22_050;
/// 32 kHz - Broadcast/DAB (Digital Audio Broadcasting) quality
pub const SAMPLE_RATE_32K: u32 = 32_000;
pub const SAMPLE_RATE_DAB: u32 = SAMPLE_RATE_32K;
/// 44.1 kHz - CD quality
pub const SAMPLE_RATE_44100: u32 = 44_100;
pub const SAMPLE_RATE_CD: u32 = SAMPLE_RATE_44100;
/// 48 kHz - DVD/Professional audio
pub const SAMPLE_RATE_48K: u32 = 48_000;
pub const SAMPLE_RATE_DVD: u32 = SAMPLE_RATE_48K;
/// 88.2 kHz - 2x CD sample rate
pub const SAMPLE_RATE_88200: u32 = 88_200;
/// 96 kHz - High-resolution audio
pub const SAMPLE_RATE_96K: u32 = 96_000;
pub const SAMPLE_RATE_HIGH: u32 = SAMPLE_RATE_96K;
/// 176.4 kHz - 4x CD sample rate
pub const SAMPLE_RATE_176400: u32 = 176_400;
/// 192 kHz - Ultra high-resolution audio
pub const SAMPLE_RATE_192K: u32 = 192_000;
pub const SAMPLE_RATE_ULTRA_HIGH: u32 = SAMPLE_RATE_192K;
/// 352.8 kHz - DXD (Digital eXtreme Definition)
pub const SAMPLE_RATE_352800: u32 = 352_800;
pub const SAMPLE_RATE_DXD: u32 = 352_800;
/// 384 kHz - Extreme high-resolution audio
pub const SAMPLE_RATE_384K: u32 = 384_000;
pub const SAMPLE_RATE_EXTREME_HIGH: u32 = SAMPLE_RATE_384K;

/// Standard audio sample rates
pub const STANDARD_SAMPLE_RATES: &[u32] = &[
    SAMPLE_RATE_8K,
    SAMPLE_RATE_11025,
    SAMPLE_RATE_16K,
    SAMPLE_RATE_22050,
    SAMPLE_RATE_32K,
    SAMPLE_RATE_44100,
    SAMPLE_RATE_48K,
    SAMPLE_RATE_88200,
    SAMPLE_RATE_96K,
    SAMPLE_RATE_176400,
    SAMPLE_RATE_192K,
    SAMPLE_RATE_352800,
    SAMPLE_RATE_384K,
];

#[derive(Clone, Copy, Debug, EnumCount, Eq, PartialEq)]
pub enum SampleFormat {
    U8 = 0, // unsigned 8 bits
    S16,    // signed 16 bits
    S32,    // signed 32 bits
    S64,    // signed 64 bits
    F32,    // float 32 bits
    F64,    // float 64 bits
    U8P,    // unsigned 8 bits, planar
    S16P,   // signed 16 bits, planar
    S32P,   // signed 32 bits, planar
    S64P,   // signed 64 bits, planar
    F32P,   // float 32 bits, planar
    F64P,   // float 64 bits, planar
}

impl Display for SampleFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<SampleFormat> for usize {
    fn from(value: SampleFormat) -> Self {
        value as usize
    }
}

impl TryFrom<usize> for SampleFormat {
    type Error = Error;

    fn try_from(value: usize) -> Result<Self> {
        if value < SampleFormat::COUNT {
            Ok(unsafe { mem::transmute::<u8, SampleFormat>(value as u8) })
        } else {
            Err(invalid_param_error!(value))
        }
    }
}

struct SampleFormatDescriptor {
    pub bits: u8,
    pub planar: bool,
}

static SAMPLE_FORMAT_DESC: [SampleFormatDescriptor; SampleFormat::COUNT] = [
    // U8
    SampleFormatDescriptor {
        bits: 8,
        planar: false,
    },
    // S16
    SampleFormatDescriptor {
        bits: 16,
        planar: false,
    },
    // S32
    SampleFormatDescriptor {
        bits: 32,
        planar: false,
    },
    // S64
    SampleFormatDescriptor {
        bits: 64,
        planar: false,
    },
    // F32
    SampleFormatDescriptor {
        bits: 32,
        planar: false,
    },
    // F64
    SampleFormatDescriptor {
        bits: 64,
        planar: false,
    },
    // U8P
    SampleFormatDescriptor {
        bits: 8,
        planar: true,
    },
    // S16P
    SampleFormatDescriptor {
        bits: 16,
        planar: true,
    },
    // S32P
    SampleFormatDescriptor {
        bits: 32,
        planar: true,
    },
    // S64P
    SampleFormatDescriptor {
        bits: 64,
        planar: true,
    },
    // F32P
    SampleFormatDescriptor {
        bits: 32,
        planar: true,
    },
    // F64P
    SampleFormatDescriptor {
        bits: 64,
        planar: true,
    },
];

impl SampleFormat {
    pub fn bits(&self) -> u8 {
        SAMPLE_FORMAT_DESC[*self as usize].bits
    }

    pub fn bytes(&self) -> u8 {
        self.bits() >> 3
    }

    pub fn is_float(&self) -> bool {
        matches!(*self, SampleFormat::F32 | SampleFormat::F64 | SampleFormat::F32P | SampleFormat::F64P)
    }

    pub fn is_planar(&self) -> bool {
        SAMPLE_FORMAT_DESC[*self as usize].planar
    }

    pub fn is_packed(&self) -> bool {
        !SAMPLE_FORMAT_DESC[*self as usize].planar
    }

    pub fn planar_sample_format(&self) -> SampleFormat {
        match *self {
            SampleFormat::U8 | SampleFormat::U8P => SampleFormat::U8P,
            SampleFormat::S16 | SampleFormat::S16P => SampleFormat::S16P,
            SampleFormat::S32 | SampleFormat::S32P => SampleFormat::S32P,
            SampleFormat::S64 | SampleFormat::S64P => SampleFormat::S64P,
            SampleFormat::F32 | SampleFormat::F32P => SampleFormat::F32P,
            SampleFormat::F64 | SampleFormat::F64P => SampleFormat::F64P,
        }
    }

    pub fn packed_sample_format(&self) -> SampleFormat {
        match *self {
            SampleFormat::U8 | SampleFormat::U8P => SampleFormat::U8,
            SampleFormat::S16 | SampleFormat::S16P => SampleFormat::S16,
            SampleFormat::S32 | SampleFormat::S32P => SampleFormat::S32,
            SampleFormat::S64 | SampleFormat::S64P => SampleFormat::S64,
            SampleFormat::F32 | SampleFormat::F32P => SampleFormat::F32,
            SampleFormat::F64 | SampleFormat::F64P => SampleFormat::F64,
        }
    }

    pub(crate) fn calc_plane_size(&self, channels: u8, samples: u32) -> usize {
        if self.is_planar() {
            self.bytes() as usize * samples as usize
        } else {
            self.bytes() as usize * samples as usize * channels as usize
        }
    }

    pub(crate) fn calc_data_size(&self, channels: u8, samples: u32, alignment: u32) -> (usize, PlaneVec<PlaneDescriptor>) {
        let mut planes = PlaneVec::new();
        let used_bytes = self.calc_plane_size(channels, samples);
        let allocated_size = align_to(used_bytes, alignment as usize);

        let size = if self.is_planar() {
            planes.extend(iter::repeat_n(PlaneDescriptor::Audio(allocated_size, used_bytes), channels as usize));
            allocated_size * channels as usize
        } else {
            planes.push(PlaneDescriptor::Audio(allocated_size, used_bytes));
            allocated_size
        };

        (size, planes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioFrameDescriptor {
    pub format: SampleFormat,
    pub samples: NonZeroU32,
    pub sample_rate: NonZeroU32,
    pub channel_layout: ChannelLayout,
}

impl AudioFrameDescriptor {
    pub fn new(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32, sample_rate: NonZeroU32) -> Self {
        Self {
            format,
            samples,
            sample_rate,
            channel_layout: ChannelLayout::default_from_channels(channels.get()).unwrap_or_default(),
        }
    }

    pub fn try_new(format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Self> {
        let channels = NonZeroU8::new(channels).ok_or_else(|| invalid_param_error!(channels))?;
        let samples = NonZeroU32::new(samples).ok_or_else(|| invalid_param_error!(samples))?;
        let sample_rate = NonZeroU32::new(sample_rate).ok_or_else(|| invalid_param_error!(sample_rate))?;

        Ok(Self::new(format, channels, samples, sample_rate))
    }

    pub fn from_channel_layout(format: SampleFormat, samples: NonZeroU32, sample_rate: NonZeroU32, channel_layout: ChannelLayout) -> Self {
        Self {
            format,
            samples,
            sample_rate,
            channel_layout,
        }
    }

    pub fn try_from_channel_layout(format: SampleFormat, samples: u32, sample_rate: u32, channel_layout: ChannelLayout) -> Result<Self> {
        let samples = NonZeroU32::new(samples).ok_or_else(|| invalid_param_error!(samples))?;
        let sample_rate = NonZeroU32::new(sample_rate).ok_or_else(|| invalid_param_error!(sample_rate))?;

        Ok(Self::from_channel_layout(format, samples, sample_rate, channel_layout))
    }

    pub fn channels(&self) -> NonZeroU8 {
        self.channel_layout.channels
    }

    pub fn duration_equal(&self, other: &AudioFrameDescriptor) -> bool {
        let duration1 = self.samples.get() as i64 * time::MSEC_PER_SEC / self.sample_rate.get() as i64;
        let duration2 = other.samples.get() as i64 * time::MSEC_PER_SEC / other.sample_rate.get() as i64;
        duration1 == duration2
    }
}

impl From<AudioFrameDescriptor> for FrameDescriptor {
    fn from(desc: AudioFrameDescriptor) -> Self {
        FrameDescriptor::Audio(desc)
    }
}

impl TryFrom<FrameDescriptor> for AudioFrameDescriptor {
    type Error = Error;

    fn try_from(value: FrameDescriptor) -> Result<Self> {
        match value {
            FrameDescriptor::Audio(desc) => Ok(desc),
            _ => Err(invalid_param_error!(value)),
        }
    }
}

impl FrameDescriptorSpec for AudioFrameDescriptor {
    fn media_type(&self) -> crate::MediaType {
        MediaType::Audio
    }

    fn create_frame(&self) -> Result<Frame<'static, Self>> {
        AudioFrame::new_with_descriptor(self.clone())
    }

    fn as_audio(&self) -> Option<&AudioFrameDescriptor> {
        Some(self)
    }
}
