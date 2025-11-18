use std::{num::NonZeroU8, sync::LazyLock};

use bitflags::bitflags;
use smallvec::SmallVec;
use strum::EnumCount;

use crate::{error::Error, invalid_param_error, Result};

#[derive(Clone, Copy, Debug, EnumCount, Eq, PartialEq)]
pub enum Channel {
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

impl From<Channel> for u32 {
    fn from(format: Channel) -> Self {
        format as u32
    }
}

impl From<Channel> for ChannelMasks {
    fn from(format: Channel) -> Self {
        ChannelMasks::from_bits_truncate(1u32 << format as u32)
    }
}

macro_rules! channel_masks {
    ($($mask:ident)|+) => {
        0 $(| ChannelMasks::$mask.bits())+
    };
    ($mask:ident) => {
        ChannelMasks::$mask.bits()
    };
}

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ChannelMasks: u32 {
        const FrontLeft             = 1u32 << Channel::FrontLeft as u32;
        const FrontRight            = 1u32 << Channel::FrontRight as u32;
        const FrontCenter           = 1u32 << Channel::FrontCenter as u32;
        const LowFrequency          = 1u32 << Channel::LowFrequency as u32;
        const BackLeft              = 1u32 << Channel::BackLeft as u32;
        const BackRight             = 1u32 << Channel::BackRight as u32;
        const FrontLeftOfCenter     = 1u32 << Channel::FrontLeftOfCenter as u32;
        const FrontRightOfCenter    = 1u32 << Channel::FrontRightOfCenter as u32;
        const BackCenter            = 1u32 << Channel::BackCenter as u32;
        const SideLeft              = 1u32 << Channel::SideLeft as u32;
        const SideRight             = 1u32 << Channel::SideRight as u32;
        const TopCenter             = 1u32 << Channel::TopCenter as u32;
        const TopFrontLeft          = 1u32 << Channel::TopFrontLeft as u32;
        const TopFrontCenter        = 1u32 << Channel::TopFrontCenter as u32;
        const TopFrontRight         = 1u32 << Channel::TopFrontRight as u32;
        const TopBackLeft           = 1u32 << Channel::TopBackLeft as u32;
        const TopBackCenter         = 1u32 << Channel::TopBackCenter as u32;
        const TopBackRight          = 1u32 << Channel::TopBackRight as u32;

        const Mono                      = channel_masks!(FrontCenter);
        const Stereo                    = channel_masks!(FrontLeft | FrontRight);
        const Surround_2_1              = channel_masks!(Stereo | LowFrequency);
        const Surround                  = channel_masks!(Stereo | FrontCenter);
        const Surround_3_0              = channel_masks!(Surround);
        const Surround_3_0_FRONT        = channel_masks!(Surround);
        const Surround_3_0_BACK         = channel_masks!(Stereo | BackCenter);
        const Surround_3_1              = channel_masks!(Surround_3_0 | LowFrequency);
        const Surround_3_1_2            = channel_masks!(Surround_3_1 | TopFrontLeft | TopFrontRight);
        const Surround_4_0              = channel_masks!(Surround_3_0 | BackCenter);
        const Surround_4_1              = channel_masks!(Surround_4_0 | LowFrequency);
        const Surround_2_2              = channel_masks!(Stereo | SideLeft | SideRight);
        const Quad                      = channel_masks!(Stereo | BackLeft | BackRight);
        const Surround_5_0              = channel_masks!(Surround_3_0 | SideLeft | SideRight);
        const Surround_5_1              = channel_masks!(Surround_5_0 | LowFrequency);
        const Surround_5_0_BACK         = channel_masks!(Surround_3_0 | BackLeft | BackRight);
        const Surround_5_1_BACK         = channel_masks!(Surround_5_0_BACK | LowFrequency);
        const Surround_6_0              = channel_masks!(Surround_5_0 | BackCenter);
        const Hexagonal                 = channel_masks!(Surround_5_0_BACK | BackCenter);
        const Surround_6_1              = channel_masks!(Surround_6_0 | LowFrequency);
        const Surround_6_0_FRONT        = channel_masks!(Surround_2_2 | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_6_1_FRONT        = channel_masks!(Surround_6_0_FRONT | LowFrequency);
        const Surround_6_1_BACK         = channel_masks!(Surround_5_1_BACK | BackCenter);
        const Surround_7_0              = channel_masks!(Surround_5_0 | BackLeft | BackRight);
        const Surround_7_1              = channel_masks!(Surround_7_0 | LowFrequency);
        const Surround_7_0_FRONT        = channel_masks!(Surround_5_0 | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_7_1_WIDE         = channel_masks!(Surround_5_1 | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_7_1_WIDE_BACK    = channel_masks!(Surround_5_1_BACK | FrontLeftOfCenter | FrontRightOfCenter);
        const Surround_5_1_2            = channel_masks!(Surround_5_1 | TopFrontLeft | TopFrontRight);
        const Surround_5_1_2_BACK       = channel_masks!(Surround_5_1_BACK | TopFrontLeft | TopFrontRight);
        const Octagonal                 = channel_masks!(Surround_5_0 | BackLeft | BackCenter | BackRight);
        const Cube                      = channel_masks!(Quad | TopFrontLeft | TopFrontRight | TopBackLeft | TopBackRight);
        const Surround_5_1_4_BACK       = channel_masks!(Surround_5_1_2 | TopBackLeft | TopBackRight);
        const Surround_7_1_2            = channel_masks!(Surround_7_1 | TopFrontLeft | TopFrontRight);
        const Surround_7_1_4_BACK       = channel_masks!(Surround_7_1_2 | TopBackLeft | TopBackRight);
        const Surround_9_1_4_BACK       = channel_masks!(Surround_7_1_4_BACK | FrontLeftOfCenter | FrontRightOfCenter);
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
                            spec: ChannelLayoutSpec::Mask(ChannelMasks::$mask),
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
    Mask(ChannelMasks),
    Map(Option<SmallVec<[Channel; DEFAULT_MAX_CHANNELS]>>),
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
            spec: ChannelLayoutSpec::Mask(ChannelMasks::from_bits_truncate(0)),
        }
    }
}

impl TryFrom<ChannelMasks> for ChannelLayout {
    type Error = Error;

    fn try_from(mask: ChannelMasks) -> std::result::Result<Self, Self::Error> {
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
    pub fn from_mask(mask: ChannelMasks) -> Result<Self> {
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
                spec: ChannelLayoutSpec::Mask(ChannelMasks::from_bits_truncate(0)),
            }))
    }
}
