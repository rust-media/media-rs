use std::{
    iter,
    num::{NonZeroU32, NonZeroU8},
};

use bitflags::bitflags;

use super::{
    media_frame::{PlaneInformation, PlaneInformationVec},
    time,
};
use crate::{error::MediaError, invalid_param_error, media::MediaFrameDescriptor};

pub const SAMPLE_RATE_TELEPHONE: u32 = 8000;
pub const SAMPLE_RATE_VOIP: u32 = 16000;
pub const SAMPLE_RATE_CD: u32 = 44100;
pub const SAMPLE_RATE_DVD: u32 = 48000;
pub const SAMPLE_RATE_HIGH: u32 = 96000;
pub const SAMPLE_RATE_ULTRA_HIGH: u32 = 192000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SampleFormat {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChannelFormat {
    FrontLeft,
    FrontRight,
    FrontCenter,
    LowFrequency,
    BackLeft,
    BackRight,
    FrontLeftOfCenter,
    FrontRightOfCenter,
    BackCenter,
    SideLeft,
    SideRight,
    TopCenter,
    TopFrontLeft,
    TopFrontCenter,
    TopFrontRight,
    TopBackLeft,
    TopBackCenter,
    TopBackRight,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ChannelFormatMasks: u64 {
        const FrontLeft             = 1u64 << ChannelFormat::FrontLeft as u32;
        const FrontRight            = 1u64 << ChannelFormat::FrontRight as u32;
        const FrontCenter           = 1u64 << ChannelFormat::FrontCenter as u32;
        const LowFrequency          = 1u64 << ChannelFormat::LowFrequency as u32;
        const BackLeft              = 1u64 << ChannelFormat::BackLeft as u32;
        const BackRight             = 1u64 << ChannelFormat::BackRight as u32;
        const FrontLeftOfCenter     = 1u64 << ChannelFormat::FrontLeftOfCenter as u32;
        const FrontRightOfCenter    = 1u64 << ChannelFormat::FrontRightOfCenter as u32;
        const BackCenter            = 1u64 << ChannelFormat::BackCenter as u32;
        const SideLeft              = 1u64 << ChannelFormat::SideLeft as u32;
        const SideRight             = 1u64 << ChannelFormat::SideRight as u32;
        const TopCenter             = 1u64 << ChannelFormat::TopCenter as u32;
        const TopFrontLeft          = 1u64 << ChannelFormat::TopFrontLeft as u32;
        const TopFrontCenter        = 1u64 << ChannelFormat::TopFrontCenter as u32;
        const TopFrontRight         = 1u64 << ChannelFormat::TopFrontRight as u32;
        const TopBackLeft           = 1u64 << ChannelFormat::TopBackLeft as u32;
        const TopBackCenter         = 1u64 << ChannelFormat::TopBackCenter as u32;
        const TopBackRight          = 1u64 << ChannelFormat::TopBackRight as u32;

        const Mono      = Self::FrontCenter.bits();
        const Stereo    = Self::FrontLeft.bits() | Self::FrontRight.bits();
    }
}



#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChannelOrder {
    Unspecified,
    Native,
    Custom,
    MAX,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelLayoutSpec {
    Mask(ChannelFormatMasks),
    Map(Option<Vec<ChannelFormat>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelLayout {
    pub order: ChannelOrder,
    pub channels: NonZeroU8,
    pub spec: ChannelLayoutSpec,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioFrameDescriptor {
    pub format: SampleFormat,
    pub channels: NonZeroU8,
    pub samples: NonZeroU32,
    pub sample_rate: NonZeroU32,
    pub channel_layout: Option<ChannelLayout>,
}

struct SampleFormatDescriptor {
    pub bits: u8,
    pub is_planar: bool,
}

static SAMPLE_FORMAT_DESC: [SampleFormatDescriptor; SampleFormat::MAX as usize] = [
    // U8
    SampleFormatDescriptor {
        bits: 8,
        is_planar: false,
    },
    // S16
    SampleFormatDescriptor {
        bits: 16,
        is_planar: false,
    },
    // S32
    SampleFormatDescriptor {
        bits: 32,
        is_planar: false,
    },
    // F32
    SampleFormatDescriptor {
        bits: 32,
        is_planar: false,
    },
    // F64
    SampleFormatDescriptor {
        bits: 64,
        is_planar: false,
    },
    // U8P
    SampleFormatDescriptor {
        bits: 8,
        is_planar: true,
    },
    // S16P
    SampleFormatDescriptor {
        bits: 16,
        is_planar: true,
    },
    // S32P
    SampleFormatDescriptor {
        bits: 32,
        is_planar: true,
    },
    // F32P
    SampleFormatDescriptor {
        bits: 32,
        is_planar: true,
    },
    // F64P
    SampleFormatDescriptor {
        bits: 64,
        is_planar: true,
    },
];

impl SampleFormat {
    pub fn bits(&self) -> u8 {
        SAMPLE_FORMAT_DESC[*self as usize].bits
    }

    pub fn bytes(&self) -> u8 {
        self.bits() >> 3
    }

    pub fn is_planar(&self) -> bool {
        SAMPLE_FORMAT_DESC[*self as usize].is_planar
    }

    pub fn stride(&self, channels: u8, samples: u32) -> u32 {
        if self.is_planar() {
            self.bytes() as u32 * samples
        } else {
            self.bytes() as u32 * channels as u32 * samples
        }
    }

    pub(super) fn calc_data(&self, channels: u8, samples: u32) -> (u32, PlaneInformationVec) {
        let mut size = 0;
        let mut planes = PlaneInformationVec::new();
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

impl From<ChannelFormat> for u32 {
    fn from(format: ChannelFormat) -> Self {
        format as u32
    }
}

impl From<ChannelFormat> for ChannelFormatMasks {
    fn from(format: ChannelFormat) -> Self {
        ChannelFormatMasks::from_bits_truncate(1u64 << format as u32)
    }
}

impl ChannelLayout {
    pub fn from_mask(mask: ChannelFormatMasks) -> Result<Self, MediaError> {
        let channels = mask.bits().count_ones() as u8;
        let spec = ChannelLayoutSpec::Mask(mask);

        NonZeroU8::new(channels)
            .map(|channels| Self {
                order: ChannelOrder::Native,
                channels,
                spec,
            })
            .ok_or_else(|| invalid_param_error!("channel mask cannot be empty"))
    }
}

impl AudioFrameDescriptor {
    pub fn new(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32, sample_rate: NonZeroU32) -> Self {
        Self {
            format,
            channels,
            samples,
            sample_rate,
            channel_layout: None,
        }
    }

    pub fn try_new(format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Self, MediaError> {
        let channels = NonZeroU8::new(channels).ok_or(invalid_param_error!(channels))?;
        let samples = NonZeroU32::new(samples).ok_or(invalid_param_error!(samples))?;
        let sample_rate = NonZeroU32::new(sample_rate).ok_or(invalid_param_error!(sample_rate))?;

        Ok(Self::new(format, channels, samples, sample_rate))
    }

    pub fn duration_equal(&self, other: &AudioFrameDescriptor) -> bool {
        let duration1 = self.samples.get() as u64 * time::MSEC_PER_SEC / self.sample_rate.get() as u64;
        let duration2 = other.samples.get() as u64 * time::MSEC_PER_SEC / other.sample_rate.get() as u64;
        duration1 == duration2
    }
}

impl From<AudioFrameDescriptor> for MediaFrameDescriptor {
    fn from(desc: AudioFrameDescriptor) -> Self {
        MediaFrameDescriptor::Audio(desc)
    }
}
