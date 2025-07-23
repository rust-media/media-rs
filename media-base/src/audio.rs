use std::{
    iter,
    num::{NonZeroU32, NonZeroU8},
    sync::LazyLock,
};

use bitflags::bitflags;
use smallvec::SmallVec;

use crate::{
    error::MediaError,
    invalid_param_error,
    media::MediaFrameDescriptor,
    media_frame::{PlaneInformation, PlaneInformationVec},
    time, Result,
};

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
    pub struct ChannelFormatMasks: u32 {
        const FrontLeft             = 1u32 << ChannelFormat::FrontLeft as u32;
        const FrontRight            = 1u32 << ChannelFormat::FrontRight as u32;
        const FrontCenter           = 1u32 << ChannelFormat::FrontCenter as u32;
        const LowFrequency          = 1u32 << ChannelFormat::LowFrequency as u32;
        const BackLeft              = 1u32 << ChannelFormat::BackLeft as u32;
        const BackRight             = 1u32 << ChannelFormat::BackRight as u32;
        const FrontLeftOfCenter     = 1u32 << ChannelFormat::FrontLeftOfCenter as u32;
        const FrontRightOfCenter    = 1u32 << ChannelFormat::FrontRightOfCenter as u32;
        const BackCenter            = 1u32 << ChannelFormat::BackCenter as u32;
        const SideLeft              = 1u32 << ChannelFormat::SideLeft as u32;
        const SideRight             = 1u32 << ChannelFormat::SideRight as u32;
        const TopCenter             = 1u32 << ChannelFormat::TopCenter as u32;
        const TopFrontLeft          = 1u32 << ChannelFormat::TopFrontLeft as u32;
        const TopFrontCenter        = 1u32 << ChannelFormat::TopFrontCenter as u32;
        const TopFrontRight         = 1u32 << ChannelFormat::TopFrontRight as u32;
        const TopBackLeft           = 1u32 << ChannelFormat::TopBackLeft as u32;
        const TopBackCenter         = 1u32 << ChannelFormat::TopBackCenter as u32;
        const TopBackRight          = 1u32 << ChannelFormat::TopBackRight as u32;

        const Mono                      = Self::FrontCenter.bits();
        const Stereo                    = Self::FrontLeft.bits() | Self::FrontRight.bits();
        const Surround_2_1              = Self::Stereo.bits() | Self::LowFrequency.bits();
        const Surround                  = Self::Stereo.bits() | Self::FrontCenter.bits();
        const Surround_3_0              = Self::Surround.bits();
        const Surround_3_0_FRONT        = Self::Surround.bits();
        const Surround_3_0_BACK         = Self::Stereo.bits() | Self::BackCenter.bits();
        const Surround_3_1              = Self::Surround_3_0.bits() | Self::LowFrequency.bits();
        const Surround_3_1_2            = Self::Surround_3_1.bits() | Self::TopFrontLeft.bits() | Self::TopFrontRight.bits();
        const Surround_4_0              = Self::Surround_3_0.bits() | Self::BackCenter.bits();
        const Surround_4_1              = Self::Surround_4_0.bits() | Self::LowFrequency.bits();
        const Surround_2_2              = Self::Stereo.bits() | Self::SideLeft.bits() | Self::SideRight.bits();
        const Quad                      = Self::Stereo.bits() | Self::BackLeft.bits() | Self::BackRight.bits();
        const Surround_5_0              = Self::Surround_3_0.bits() | Self::SideLeft.bits() | Self::SideRight.bits();
        const Surround_5_1              = Self::Surround_5_0.bits() | Self::LowFrequency.bits();
        const Surround_5_0_BACK         = Self::Surround_3_0.bits() | Self::BackLeft.bits() | Self::BackRight.bits();
        const Surround_5_1_BACK         = Self::Surround_5_0_BACK.bits() | Self::LowFrequency.bits();
        const Surround_6_0              = Self::Surround_5_0.bits() | Self::BackCenter.bits();
        const Hexagonal                 = Self::Surround_5_0_BACK.bits() | Self::BackCenter.bits();
        const Surround_6_1              = Self::Surround_6_0.bits() | Self::LowFrequency.bits();
        const Surround_6_0_FRONT        = Self::Surround_2_2.bits() | Self::FrontLeftOfCenter.bits() | Self::FrontRightOfCenter.bits();
        const Surround_6_1_FRONT        = Self::Surround_6_0_FRONT.bits() | Self::LowFrequency.bits();
        const Surround_6_1_BACK         = Self::Surround_5_1_BACK.bits() | Self::BackCenter.bits();
        const Surround_7_0              = Self::Surround_5_0.bits() | Self::BackLeft.bits() | Self::BackRight.bits();
        const Surround_7_1              = Self::Surround_7_0.bits() | Self::LowFrequency.bits();
        const Surround_7_0_FRONT        = Self::Surround_5_0.bits() | Self::FrontLeftOfCenter.bits() | Self::FrontRightOfCenter.bits();
        const Surround_7_1_WIDE         = Self::Surround_5_1.bits() | Self::FrontLeftOfCenter.bits() | Self::FrontRightOfCenter.bits();
        const Surround_7_1_WIDE_BACK    = Self::Surround_5_1_BACK.bits() | Self::FrontLeftOfCenter.bits() | Self::FrontRightOfCenter.bits();
        const Surround_5_1_2            = Self::Surround_5_1.bits() | Self::TopFrontLeft.bits() | Self::TopFrontRight.bits();
        const Surround_5_1_2_BACK       = Self::Surround_5_1_BACK.bits() | Self::TopFrontLeft.bits() | Self::TopFrontRight.bits();
        const Octagonal                 = Self::Surround_5_0.bits() | Self::BackLeft.bits() | Self::BackCenter.bits() | Self::BackRight.bits();
        const Cube                      = Self::Quad.bits() | Self::TopFrontLeft.bits() | Self::TopFrontRight.bits() | Self::TopBackLeft.bits() | Self::TopBackRight.bits();
        const Surround_5_1_4_BACK       = Self::Surround_5_1_2.bits() | Self::TopBackLeft.bits() | Self::TopBackRight.bits();
        const Surround_7_1_2            = Self::Surround_7_1.bits() | Self::TopFrontLeft.bits() | Self::TopFrontRight.bits();
        const Surround_7_1_4_BACK       = Self::Surround_7_1_2.bits() | Self::TopBackLeft.bits() | Self::TopBackRight.bits();
        const Surround_9_1_4_BACK       = Self::Surround_7_1_4_BACK.bits() | Self::FrontLeftOfCenter.bits() | Self::FrontRightOfCenter.bits();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChannelOrder {
    Unspecified,
    Native,
    Custom,
    MAX,
}

const DEFAULT_MAX_CHANNELS: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelLayoutSpec {
    Mask(ChannelFormatMasks),
    Map(Option<SmallVec<[ChannelFormat; DEFAULT_MAX_CHANNELS]>>),
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
    pub samples: NonZeroU32,
    pub sample_rate: NonZeroU32,
    pub channel_layout: ChannelLayout,
}

struct ChannelLayoutMap {
    map: Vec<Option<Vec<ChannelLayout>>>,
}

macro_rules! define_channel_layout_map {
    ( $($channel_count:literal => [$($mask:ident),*]),* $(,)? ) => {
        static CHANNEL_LAYOUT_MAP: LazyLock<ChannelLayoutMap> = LazyLock::new(|| {
            let mut map = vec![None; DEFAULT_MAX_CHANNELS];
            $(
                let channels = NonZeroU8::new($channel_count).unwrap();
                map[($channel_count - 1) as usize] = Some(vec![
                    $(
                        ChannelLayout {
                            order: ChannelOrder::Native,
                            channels,
                            spec: ChannelLayoutSpec::Mask(ChannelFormatMasks::$mask),
                        }
                    ),*
                ]);
            )*
            ChannelLayoutMap { map }
        });
    };
}

define_channel_layout_map! {
    1 => [Mono],
    2 => [Stereo],
    3 => [Surround_2_1, Surround_3_0, Surround_3_0_BACK],
    4 => [Surround_3_1, Surround_4_0, Surround_2_2, Quad],
    5 => [Surround_4_1, Surround_5_0, Surround_5_0_BACK],
    6 => [Surround_5_1, Surround_5_1_BACK, Surround_6_0, Surround_6_0_FRONT, Surround_3_1_2, Hexagonal],
    7 => [Surround_6_1, Surround_6_1_FRONT, Surround_6_1_BACK, Surround_7_0, Surround_7_0_FRONT],
    8 => [Surround_7_1, Surround_7_1_WIDE, Surround_7_1_WIDE_BACK, Surround_5_1_2, Surround_5_1_2_BACK, Octagonal, Cube],
   10 => [Surround_5_1_4_BACK, Surround_7_1_2],
   12 => [Surround_7_1_4_BACK],
   14 => [Surround_9_1_4_BACK],
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

    pub(crate) fn calc_data(&self, channels: u8, samples: u32) -> (u32, PlaneInformationVec) {
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
        ChannelFormatMasks::from_bits_truncate(1u32 << format as u32)
    }
}

impl Default for ChannelLayout {
    fn default() -> Self {
        Self {
            order: ChannelOrder::Unspecified,
            channels: NonZeroU8::new(1).unwrap(),
            spec: ChannelLayoutSpec::Mask(ChannelFormatMasks::from_bits_truncate(0)),
        }
    }
}

impl ChannelLayout {
    pub fn from_mask(mask: ChannelFormatMasks) -> Result<Self> {
        let channels = mask.bits().count_ones() as u8;
        let spec = ChannelLayoutSpec::Mask(mask);

        NonZeroU8::new(channels)
            .map(|channels| Self {
                order: ChannelOrder::Native,
                channels,
                spec,
            })
            .ok_or_else(|| invalid_param_error!("channel mask is empty"))
    }

    pub fn default(channels: u8) -> Result<Self> {
        let channels = NonZeroU8::new(channels).ok_or(invalid_param_error!(channels))?;

        Ok(CHANNEL_LAYOUT_MAP
            .map
            .get((channels.get() - 1) as usize)
            .and_then(|opt| opt.as_ref())
            .and_then(|layouts| layouts.first())
            .cloned()
            .unwrap_or(Self {
                order: ChannelOrder::Unspecified,
                channels,
                spec: ChannelLayoutSpec::Mask(ChannelFormatMasks::from_bits_truncate(0)),
            }))
    }
}

impl AudioFrameDescriptor {
    pub fn new(format: SampleFormat, channels: NonZeroU8, samples: NonZeroU32, sample_rate: NonZeroU32) -> Self {
        Self {
            format,
            samples,
            sample_rate,
            channel_layout: ChannelLayout::default(channels.get()).unwrap_or_default(),
        }
    }

    pub fn try_new(format: SampleFormat, channels: u8, samples: u32, sample_rate: u32) -> Result<Self> {
        let channels = NonZeroU8::new(channels).ok_or(invalid_param_error!(channels))?;
        let samples = NonZeroU32::new(samples).ok_or(invalid_param_error!(samples))?;
        let sample_rate = NonZeroU32::new(sample_rate).ok_or(invalid_param_error!(sample_rate))?;

        Ok(Self::new(format, channels, samples, sample_rate))
    }

    pub fn channels(&self) -> NonZeroU8 {
        self.channel_layout.channels
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
