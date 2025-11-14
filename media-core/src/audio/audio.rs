use std::{
    iter, mem,
    num::{NonZeroU32, NonZeroU8},
    sync::LazyLock,
};

use bitflags::bitflags;
use smallvec::SmallVec;
use strum::EnumCount;

use crate::{
    align_to,
    error::Error,
    frame::{PlaneDescriptor, PlaneVec},
    invalid_param_error,
    media::FrameDescriptor,
    time, Result,
};

pub const SAMPLE_RATE_TELEPHONE: u32 = 8000;
pub const SAMPLE_RATE_VOIP: u32 = 16000;
pub const SAMPLE_RATE_CD: u32 = 44100;
pub const SAMPLE_RATE_DVD: u32 = 48000;
pub const SAMPLE_RATE_HIGH: u32 = 96000;
pub const SAMPLE_RATE_ULTRA_HIGH: u32 = 192000;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

macro_rules! chn_fmt_masks {
    ($($mask:ident)|+) => {
        0 $(| ChannelFormatMasks::$mask.bits())+
    };
    ($mask:ident) => {
        ChannelFormatMasks::$mask.bits()
    };
}

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

        const Mono                      = chn_fmt_masks!(FrontCenter);
        const Stereo                    = chn_fmt_masks!(FrontLeft | FrontRight);
        const Surround_2_1              = chn_fmt_masks!(Stereo | LowFrequency);
        const Surround                  = chn_fmt_masks!(Stereo | FrontCenter);
        const Surround_3_0              = chn_fmt_masks!(Surround);
        const Surround_3_0_FRONT        = chn_fmt_masks!(Surround);
        const Surround_3_0_BACK         = chn_fmt_masks!(Stereo | BackCenter);
        const Surround_3_1              = chn_fmt_masks!(Surround_3_0 | LowFrequency);
        const Surround_3_1_2            = chn_fmt_masks!(Surround_3_1 | TopFrontLeft | TopFrontRight);
        const Surround_4_0              = chn_fmt_masks!(Surround_3_0 | BackCenter);
        const Surround_4_1              = chn_fmt_masks!(Surround_4_0 | LowFrequency);
        const Surround_2_2              = chn_fmt_masks!(Stereo | SideLeft | SideRight);
        const Quad                      = chn_fmt_masks!(Stereo | BackLeft | BackRight);
        const Surround_5_0              = chn_fmt_masks!(Surround_3_0 | SideLeft | SideRight);
        const Surround_5_1              = chn_fmt_masks!(Surround_5_0 | LowFrequency);
        const Surround_5_0_BACK         = chn_fmt_masks!(Surround_3_0 | BackLeft | BackRight);
        const Surround_5_1_BACK         = chn_fmt_masks!(Surround_5_0_BACK | LowFrequency);
        const Surround_6_0              = chn_fmt_masks!(Surround_5_0 | BackCenter);
        const Hexagonal                 = chn_fmt_masks!(Surround_5_0_BACK | BackCenter);
        const Surround_6_1              = chn_fmt_masks!(Surround_6_0 | LowFrequency);
        const Surround_6_0_FRONT        = chn_fmt_masks!(Surround_2_2 | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_6_1_FRONT        = chn_fmt_masks!(Surround_6_0_FRONT | LowFrequency);
        const Surround_6_1_BACK         = chn_fmt_masks!(Surround_5_1_BACK | BackCenter);
        const Surround_7_0              = chn_fmt_masks!(Surround_5_0 | BackLeft | BackRight);
        const Surround_7_1              = chn_fmt_masks!(Surround_7_0 | LowFrequency);
        const Surround_7_0_FRONT        = chn_fmt_masks!(Surround_5_0 | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_7_1_WIDE         = chn_fmt_masks!(Surround_5_1 | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_7_1_WIDE_BACK    = chn_fmt_masks!(Surround_5_1_BACK | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_5_1_2            = chn_fmt_masks!(Surround_5_1 | TopFrontLeft | TopFrontRight);
        const Surround_5_1_2_BACK       = chn_fmt_masks!(Surround_5_1_BACK | TopFrontLeft | TopFrontRight);
        const Octagonal                 = chn_fmt_masks!(Surround_5_0 | BackLeft | BackCenter | BackRight);
        const Cube                      = chn_fmt_masks!(Quad | TopFrontLeft | TopFrontRight | TopBackLeft | TopBackRight);
        const Surround_5_1_4_BACK       = chn_fmt_masks!(Surround_5_1_2 | TopBackLeft | TopBackRight);
        const Surround_7_1_2            = chn_fmt_masks!(Surround_7_1 | TopFrontLeft | TopFrontRight);
        const Surround_7_1_4_BACK       = chn_fmt_masks!(Surround_7_1_2 | TopBackLeft | TopBackRight);
        const Surround_9_1_4_BACK       = chn_fmt_masks!(Surround_7_1_4_BACK | FrontLeftOfCenter | FrontRightOfCenter);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelOrder {
    Unspecified,
    Native,
    Custom,
    MAX,
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

const DEFAULT_MAX_CHANNELS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelLayoutSpec {
    Mask(ChannelFormatMasks),
    Map(Option<SmallVec<[ChannelFormat; DEFAULT_MAX_CHANNELS]>>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelLayout {
    pub order: ChannelOrder,
    pub channels: NonZeroU8,
    pub spec: ChannelLayoutSpec,
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

impl TryFrom<ChannelFormatMasks> for ChannelLayout {
    type Error = Error;

    fn try_from(mask: ChannelFormatMasks) -> std::result::Result<Self, Self::Error> {
        Self::from_mask(mask)
    }
}

impl TryFrom<u8> for ChannelLayout {
    type Error = Error;

    fn try_from(channels: u8) -> std::result::Result<Self, Self::Error> {
        Self::default_from_channels(channels)
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

    pub fn default_from_channels(channels: u8) -> Result<Self> {
        let channels = NonZeroU8::new(channels).ok_or_else(|| invalid_param_error!(channels))?;

        Ok(CHANNEL_LAYOUT_MAP
            .map
            .get((channels.get() - 1) as usize)
            .and_then(|opt| opt.as_ref())
            .and_then(|layouts| layouts.first())
            .cloned()
            .unwrap_or_else(|| Self {
                order: ChannelOrder::Unspecified,
                channels,
                spec: ChannelLayoutSpec::Mask(ChannelFormatMasks::from_bits_truncate(0)),
            }))
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
        let duration1 = self.samples.get() as u64 * time::MSEC_PER_SEC / self.sample_rate.get() as u64;
        let duration2 = other.samples.get() as u64 * time::MSEC_PER_SEC / other.sample_rate.get() as u64;
        duration1 == duration2
    }
}

impl From<AudioFrameDescriptor> for FrameDescriptor {
    fn from(desc: AudioFrameDescriptor) -> Self {
        FrameDescriptor::Audio(desc)
    }
}
