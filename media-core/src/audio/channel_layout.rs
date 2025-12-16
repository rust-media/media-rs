use std::{mem, num::NonZeroU8, sync::LazyLock};

use bitflags::bitflags;
use smallvec::SmallVec;
use strum::EnumCount;

use crate::{error::Error, invalid_param_error, Result};

#[derive(Clone, Copy, Debug, EnumCount, Eq, PartialEq)]
#[repr(u8)]
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
    fn from(chn: Channel) -> Self {
        chn as u32
    }
}

impl From<Channel> for ChannelMasks {
    fn from(chn: Channel) -> Self {
        ChannelMasks::from_bits_truncate(1u32 << chn as u32)
    }
}

impl TryFrom<u8> for Channel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        if value < Channel::COUNT as u8 {
            Ok(unsafe { mem::transmute::<u8, Channel>(value) })
        } else {
            Err(invalid_param_error!(value))
        }
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
    map: Vec<Option<Vec<(&'static str, ChannelLayout)>>>,
}

macro_rules! define_channel_layouts {
    ( $(
        $const_name:ident: [$name:literal, $mask:ident($channel_count:literal)]
        ),* $(,)?
    ) => {
        impl ChannelLayout {
            $(
                pub const $const_name: Self = Self {
                    order: ChannelOrder::Native,
                    channels: NonZeroU8::new($channel_count).unwrap(),
                    spec: ChannelLayoutSpec::Mask(ChannelMasks::$mask),
                };
            )*
        }

        static CHANNEL_LAYOUT_MAP: LazyLock<ChannelLayoutMap> = LazyLock::new(|| {
            let mut map = vec![None; DEFAULT_MAX_CHANNELS];
            $(
                let entry = map[($channel_count - 1) as usize].get_or_insert_with(Vec::new);
                entry.push((
                    $name,
                    ChannelLayout::$const_name,
                ));
            )*
            ChannelLayoutMap { map }
        });
    };
}

define_channel_layouts! {
    MONO: ["mono", Mono(1)],
    STEREO: ["stereo", Stereo(2)],
    SURROUND_2_1: ["2.1", Surround_2_1(3)],
    SURROUND_3_0: ["3.0", Surround_3_0(3)],
    SURROUND_3_0_BACK: ["3.0(back)", Surround_3_0_BACK(3)],
    SURROUND_4_0: ["4.0", Surround_4_0(4)],
    QUAD: ["quad", Quad(4)],
    SURROUND_2_2: ["quad(side)", Surround_2_2(4)],
    SURROUND_3_1: ["3.1", Surround_3_1(4)],
    SURROUND_5_0_BACK: ["5.0", Surround_5_0_BACK(5)],
    SURROUND_5_0: ["5.0(side)", Surround_5_0(5)],
    SURROUND_4_1: ["4.1", Surround_4_1(5)],
    SURROUND_5_1_BACK: ["5.1", Surround_5_1_BACK(6)],
    SURROUND_5_1: ["5.1(side)", Surround_5_1(6)],
    SURROUND_6_0: ["6.0", Surround_6_0(6)],
    SURROUND_6_0_FRONT: ["6.0(front)", Surround_6_0_FRONT(6)],
    SURROUND_3_1_2: ["3.1.2", Surround_3_1_2(6)],
    HEXAGONAL: ["hexagonal", Hexagonal(6)],
    SURROUND_6_1: ["6.1", Surround_6_1(7)],
    SURROUND_6_1_BACK: ["6.1(back)", Surround_6_1_BACK(7)],
    SURROUND_6_1_FRONT: ["6.1(front)", Surround_6_1_FRONT(7)],
    SURROUND_7_0: ["7.0", Surround_7_0(7)],
    SURROUND_7_0_FRONT: ["7.0(front)", Surround_7_0_FRONT(7)],
    SURROUND_7_1: ["7.1", Surround_7_1(8)],
    SURROUND_7_1_WIDE_BACK: ["7.1(wide)", Surround_7_1_WIDE_BACK(8)],
    SURROUND_7_1_WIDE: ["7.1(wide-side)", Surround_7_1_WIDE(8)],
    SURROUND_5_1_2: ["5.1.2", Surround_5_1_2(8)],
    SURROUND_5_1_2_BACK: ["5.1.2(back)", Surround_5_1_2_BACK(8)],
    OCTAGONAL: ["octagonal", Octagonal(8)],
    CUBE: ["cube", Cube(8)],
    SURROUND_5_1_4_BACK: ["5.1.4", Surround_5_1_4_BACK(10)],
    SURROUND_7_1_2: ["7.1.2", Surround_7_1_2(10)],
    SURROUND_7_1_4_BACK: ["7.1.4", Surround_7_1_4_BACK(12)],
    SURROUND_9_1_4_BACK: ["9.1.4", Surround_9_1_4_BACK(14)],
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
            .and_then(|opt| opt.as_ref()?.first())
            .map(|(_, layout)| layout.clone())
            .unwrap_or_else(|| Self {
                order: ChannelOrder::Unspecified,
                channels,
                spec: ChannelLayoutSpec::Mask(ChannelMasks::from_bits_truncate(0)),
            }))
    }

    pub fn get_channel_from_index(&self, index: usize) -> Option<Channel> {
        if index >= self.channels.get() as usize {
            return None;
        }

        match (&self.order, &self.spec) {
            (ChannelOrder::Native, ChannelLayoutSpec::Mask(mask)) => {
                let mut remaining = index;
                for chn in 0..Channel::COUNT {
                    let channel = Channel::try_from(chn as u8).ok()?;
                    if mask.contains(ChannelMasks::from(channel)) {
                        if remaining == 0 {
                            return Some(channel);
                        }
                        remaining -= 1;
                    }
                }
                None
            }
            (ChannelOrder::Custom, ChannelLayoutSpec::Map(Some(map))) => map.get(index).copied(),
            _ => None,
        }
    }

    pub fn get_index_from_channel(&self, channel: Channel) -> Option<usize> {
        match (&self.order, &self.spec) {
            (ChannelOrder::Native, ChannelLayoutSpec::Mask(mask)) => {
                let channel_mask = ChannelMasks::from(channel);
                mask.contains(channel_mask).then(|| {
                    let lower_bits = channel_mask.bits() - 1;
                    (mask.bits() & lower_bits).count_ones() as usize
                })
            }
            (ChannelOrder::Custom, ChannelLayoutSpec::Map(Some(map))) => map.iter().position(|&c| c == channel),
            _ => None,
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.channels.get() == 0 {
            return false;
        }

        match (&self.order, &self.spec) {
            (ChannelOrder::Unspecified, _) => true,
            (ChannelOrder::Native, ChannelLayoutSpec::Mask(mask)) => mask.bits().count_ones() as u8 == self.channels.get(),
            (ChannelOrder::Custom, ChannelLayoutSpec::Map(Some(map))) => map.len() == self.channels.get() as usize,
            _ => false,
        }
    }

    pub fn subset(&self, mask: ChannelMasks) -> ChannelMasks {
        match (&self.order, &self.spec) {
            (ChannelOrder::Native, ChannelLayoutSpec::Mask(channel_mask)) => *channel_mask & mask,
            (ChannelOrder::Custom, ChannelLayoutSpec::Map(Some(map))) => {
                let mut subset_mask = ChannelMasks::empty();
                for &channel in map.iter() {
                    let channel_mask = ChannelMasks::from(channel);
                    if mask.contains(channel_mask) {
                        subset_mask |= channel_mask;
                    }
                }
                subset_mask
            }
            _ => ChannelMasks::from_bits_truncate(0),
        }
    }
}
