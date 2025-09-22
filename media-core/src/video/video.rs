use std::{
    cmp,
    fmt::{Display, Formatter},
    iter, mem,
    num::NonZeroU32,
};

use bitflags::bitflags;
use num_enum::TryFromPrimitive;

use crate::{
    align_to, ceil_rshift,
    error::Error,
    frame::{PlaneInformation, PlaneInformationVec},
    invalid_param_error,
    media::FrameDescriptor,
    Result,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
        }
    }

    pub const SQCIF: Self = Self::new(128, 96);
    pub const QCIF: Self = Self::new(176, 144);
    pub const CIF: Self = Self::new(352, 288);
    pub const QQVGA: Self = Self::new(160, 120);
    pub const QVGA: Self = Self::new(320, 240);
    pub const VGA: Self = Self::new(640, 480);
    pub const SVGA: Self = Self::new(800, 600);
    pub const XGA: Self = Self::new(1024, 768);
    pub const SXGA: Self = Self::new(1280, 1024);
    pub const UXGA: Self = Self::new(1600, 1200);
    pub const QXGA: Self = Self::new(2048, 1536);
    pub const SD: Self = Self::new(720, 480);
    pub const HD: Self = Self::new(1280, 720);
    pub const FHD: Self = Self::new(1920, 1080);
    pub const QHD: Self = Self::new(2560, 1440);
    pub const UHD_4K: Self = Self::new(3840, 2160);
    pub const UHD_8K: Self = Self::new(7680, 4320);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ColorRange {
    #[default]
    Unspecified,
    Video,
    Full,
    MAX,
}

impl From<ColorRange> for usize {
    fn from(value: ColorRange) -> Self {
        value as usize
    }
}

impl From<usize> for ColorRange {
    fn from(value: usize) -> Self {
        match value {
            0 => ColorRange::Unspecified,
            1 => ColorRange::Video,
            2 => ColorRange::Full,
            _ => ColorRange::Unspecified,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ColorMatrix {
    #[default]
    Identity = 0, // The identity matrix
    BT709,            // BT.709
    Unspecified,      // Unspecified
    Reserved,         // Reserved
    FCC,              // FCC Title 47 Code of Federal Regulations 73.682(a)(20)
    BT470BG,          // BT.601 PAL & SECAM
    SMPTE170M,        // BT.601 NTSC
    SMPTE240M,        // SMPTE ST 240
    YCgCo,            // YCgCo
    BT2020NCL,        // BT.2020 non-constant luminance system
    BT2020CL,         // BT.2020 constant luminance system
    SMPTE2085,        // SMPTE ST 2085 Y'D'zD'x
    ChromaDerivedNCL, // Chromaticity-derived non-constant luminance system
    ChromaDerivedCL,  // Chromaticity-derived constant luminance system
    ICtCp,            // BT.2100 ICtCp
    SMPTE2128,        // SMPTE ST 2128
}

impl From<ColorMatrix> for usize {
    fn from(value: ColorMatrix) -> Self {
        value as usize
    }
}

impl TryFrom<usize> for ColorMatrix {
    type Error = Error;

    fn try_from(value: usize) -> Result<Self> {
        match value {
            0 => Ok(ColorMatrix::Identity),
            1 => Ok(ColorMatrix::BT709),
            2 => Ok(ColorMatrix::Unspecified),
            4 => Ok(ColorMatrix::FCC),
            5 => Ok(ColorMatrix::BT470BG),
            6 => Ok(ColorMatrix::SMPTE170M),
            7 => Ok(ColorMatrix::SMPTE240M),
            8 => Ok(ColorMatrix::YCgCo),
            9 => Ok(ColorMatrix::BT2020NCL),
            10 => Ok(ColorMatrix::BT2020CL),
            11 => Ok(ColorMatrix::SMPTE2085),
            12 => Ok(ColorMatrix::ChromaDerivedNCL),
            13 => Ok(ColorMatrix::ChromaDerivedCL),
            14 => Ok(ColorMatrix::ICtCp),
            15 => Ok(ColorMatrix::SMPTE2128),
            _ => Err(invalid_param_error!(value)),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ColorPrimaries {
    #[default]
    Reserved  = 0, // Reserved
    BT709,          // BT.709
    Unspecified,    // Unspecified
    BT470M    = 4,  // FCC Title 47 Code of Federal Regulations 73.682(a)(20)
    BT470BG,        // BT.601 PAL & SECAM
    SMPTE170M,      // BT.601 NTSC
    SMPTE240M,      // SMPTE ST 240 / SMPTE C
    Film,           // Generic film (color filters using illuminant C)
    BT2020,         // BT.2020
    SMPTE428,       // SMPTE ST 428-1 (CIE 1931 XYZ)
    SMPTE431,       // SMPTE ST 431-2 (DCI P3)
    SMPTE432,       // SMPTE ST 432-1 (P3 D65 / Display P3)
    JEDEC_P22 = 22, // JEDEC P22 phosphors
}

impl From<ColorPrimaries> for usize {
    fn from(value: ColorPrimaries) -> Self {
        value as usize
    }
}

impl TryFrom<usize> for ColorPrimaries {
    type Error = Error;

    fn try_from(value: usize) -> Result<Self> {
        match value {
            0 => Ok(ColorPrimaries::Reserved),
            1 => Ok(ColorPrimaries::BT709),
            2 => Ok(ColorPrimaries::Unspecified),
            4 => Ok(ColorPrimaries::BT470M),
            5 => Ok(ColorPrimaries::BT470BG),
            6 => Ok(ColorPrimaries::SMPTE170M),
            7 => Ok(ColorPrimaries::SMPTE240M),
            8 => Ok(ColorPrimaries::Film),
            9 => Ok(ColorPrimaries::BT2020),
            10 => Ok(ColorPrimaries::SMPTE428),
            11 => Ok(ColorPrimaries::SMPTE431),
            12 => Ok(ColorPrimaries::SMPTE432),
            22 => Ok(ColorPrimaries::JEDEC_P22),
            _ => Err(invalid_param_error!(value)),
        }
    }
}

#[allow(non_camel_case_types)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorTransferCharacteristics {
    #[default]
    Reserved = 0, // Reserved
    BT709,        // BT.709
    Unspecified,  // Unspecified
    BT470M   = 4, // gamma = 2.2 / FCC Title 47 Code of Federal Regulations 73.682(a)(20)
    BT470BG,      // gamma = 2.8 / BT.601 PAL & SECAM
    SMPTE170M,    // BT.601 NTSC
    SMPTE240M,    // SMPTE ST 240
    Linear,       // Linear transfer characteristics
    Log,          // Logarithmic transfer characteristic (100 : 1 range)
    LogSqrt,      // Logarithmic transfer characteristic (100 * Sqrt(10) : 1 range)
    IEC61966_2_4, // IEC 61966-2-4
    BT1361E,      // ITU-R BT1361 Extended Colour Gamut
    IEC61966_2_1, // IEC 61966-2-1 (sRGB or sYCC)
    BT2020_10,    // BT.2020 10-bit systems
    BT2020_12,    // BT.2020 12-bit systems
    SMPTE2084,    // SMPTE ST 2084 / BT.2100 perceptual quantization (PQ) system
    SMPTE428,     // SMPTE ST 428-1
    ARIB_STD_B67, // ARIB STD-B67 / BT.2100 hybrid log-gamma (HLG) system
}

impl From<ColorTransferCharacteristics> for usize {
    fn from(value: ColorTransferCharacteristics) -> Self {
        value as usize
    }
}

impl TryFrom<usize> for ColorTransferCharacteristics {
    type Error = Error;

    fn try_from(value: usize) -> Result<Self> {
        match value {
            0 => Ok(ColorTransferCharacteristics::Reserved),
            1 => Ok(ColorTransferCharacteristics::BT709),
            2 => Ok(ColorTransferCharacteristics::Unspecified),
            4 => Ok(ColorTransferCharacteristics::BT470M),
            5 => Ok(ColorTransferCharacteristics::BT470BG),
            6 => Ok(ColorTransferCharacteristics::SMPTE170M),
            7 => Ok(ColorTransferCharacteristics::SMPTE240M),
            8 => Ok(ColorTransferCharacteristics::Linear),
            9 => Ok(ColorTransferCharacteristics::Log),
            10 => Ok(ColorTransferCharacteristics::LogSqrt),
            11 => Ok(ColorTransferCharacteristics::IEC61966_2_4),
            12 => Ok(ColorTransferCharacteristics::BT1361E),
            13 => Ok(ColorTransferCharacteristics::IEC61966_2_1),
            14 => Ok(ColorTransferCharacteristics::BT2020_10),
            15 => Ok(ColorTransferCharacteristics::BT2020_12),
            16 => Ok(ColorTransferCharacteristics::SMPTE2084),
            17 => Ok(ColorTransferCharacteristics::SMPTE428),
            18 => Ok(ColorTransferCharacteristics::ARIB_STD_B67),
            _ => Err(invalid_param_error!(value)),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, TryFromPrimitive)]
pub enum PixelFormat {
    #[default]
    ARGB32 = 0, // packed ARGB, 32 bits
    BGRA32, // packed BGRA, 32 bits
    ABGR32, // packed ABGR, 32 bits
    RGBA32, // packed RGBA, 32 bits
    RGB24,  // packed RGB, 24 bits
    BGR24,  // packed BGR, 24 bits
    I420,   // planar YUV 4:2:0, 12 bits
    I422,   // planar YUV 4:2:2, 16 bits
    I444,   // planar YUV 4:4:4, 24 bits
    I440,   // planar YUV 4:4:0, 16 bits
    NV12,   // biplanar YUV 4:2:0, 12 bits
    NV21,   // biplanar YUV 4:2:0, 12 bits
    NV16,   // biplanar YUV 4:2:2, 16 bits
    NV61,   // biplanar YUV 4:2:2, 16 bits
    NV24,   // biplanar YUV 4:4:4, 24 bits
    NV42,   // biplanar YUV 4:4:4, 24 bits
    YV12,   // planar YVU 4:2:0, 12 bits
    YV16,   // planar YVU 4:2:2, 16 bits
    YV24,   // planar YVU 4:4:4, 24 bits
    YUYV,   // packed YUV 4:2:2, 16 bits, Y0 Cb Y1 Cr
    YVYU,   // packed YUV 4:2:2, 16 bits, Y0 Cr Y1 Cb
    UYVY,   // packed YUV 4:2:2, 16 bits, Cb Y0 Cr Y1
    VYUY,   // packed YUV 4:2:2, 16 bits, Cr Y0 Cb Y1
    AYUV,   // packed AYUV 4:4:4, 32 bits
    Y8,     // greyscale, 8 bits Y
    YA8,    // greyscale, 8 bits Y, 8 bits alpha
    RGB30,  // packed RGB, 30 bits, 10 bits per channel, 2 bits unused(LSB)
    BGR30,  // packed BGR, 30 bits, 10 bits per channel, 2 bits unused(LSB)
    ARGB64, // packed ARGB, 64 bits, 16 bits per channel, 16-bit big-endian
    BGRA64, // packed BGRA, 64 bits, 16 bits per channel, 16-bit big-endian
    ABGR64, // packed ABGR, 64 bits, 16 bits per channel, 16-bit big-endian
    RGBA64, // packed RGBA, 64 bits, 16 bits per channel, 16-bit big-endian
    I010,   // planar YUV 4:2:0, 10 bits per channel
    I210,   // planar YUV 4:2:2, 10 bits per channel
    I410,   // planar YUV 4:4:4, 10 bits per channel
    I44010, // planar YUV 4:4:0, 10 bits per channel
    P010,   // biplanar YUV 4:2:0, 10 bits per channel
    P210,   // biplanar YUV 4:2:2, 10 bits per channel
    P410,   // biplanar YUV 4:4:4, 10 bits per channel
    I012,   // planar YUV 4:2:2, 12 bits per channel
    I212,   // planar YUV 4:2:2, 12 bits per channel
    I412,   // planar YUV 4:4:4, 12 bits per channel
    I44012, // planar YUV 4:4:0, 12 bits per channel
    P012,   // biplanar YUV 4:2:0, 12 bits per channel
    P212,   // biplanar YUV 4:2:2, 12 bits per channel
    P412,   // biplanar YUV 4:4:4, 12 bits per channel
    I016,   // planar YUV 4:2:0, 16 bits per channel
    I216,   // planar YUV 4:2:2, 16 bits per channel
    I416,   // planar YUV 4:4:4, 16 bits per channel
    I44016, // planar YUV 4:4:0, 16 bits per channel
    P016,   // biplanar YUV 4:2:0, 16 bits per channel
    P216,   // biplanar YUV 4:2:2, 16 bits per channel
    P416,   // biplanar YUV 4:4:4, 16 bits per channel
    MAX,
}

impl From<PixelFormat> for usize {
    fn from(value: PixelFormat) -> Self {
        value as usize
    }
}

impl TryFrom<usize> for PixelFormat {
    type Error = Error;

    fn try_from(value: usize) -> Result<Self> {
        if value <= PixelFormat::MAX as usize {
            Ok(unsafe { mem::transmute::<u8, PixelFormat>(value as u8) })
        } else {
            Err(invalid_param_error!(value))
        }
    }
}

bitflags! {
    #[repr(transparent)]
    struct PixelFormatFlags: u32 {
        const Alpha    = 1 << 0;
        const RGB      = 1 << 1;
        const YUV      = 1 << 2;
        const Planar   = 1 << 3;
        const Packed   = 1 << 4;
        const BiPlanar = 1 << 5;
    }
}

struct PixelFormatDescriptor {
    components: u8,
    chroma_shift_x: u8,
    chroma_shift_y: u8,
    depth: u8,
    flags: PixelFormatFlags,
    component_bytes: [u8; 4],
}

macro_rules! pix_fmt_flags {
    ($($flag:ident)|+) => {
        PixelFormatFlags::from_bits_truncate(0 $(| PixelFormatFlags::$flag.bits())+)
    };
    ($flag:ident) => {
        PixelFormatFlags::from_bits_truncate(PixelFormatFlags::$flag.bits())
    };
}

static PIXEL_FORMAT_DESC: [PixelFormatDescriptor; PixelFormat::MAX as usize] = [
    // ARGB32
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // BGRA32
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // ABGR32
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // RGBA32
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // RGB24
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(RGB | Packed),
        component_bytes: [3, 0, 0, 0],
    },
    // BGR24
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(RGB | Packed),
        component_bytes: [3, 0, 0, 0],
    },
    // I420
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // I422
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // I444
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // I440
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 1,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // NV12
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [1, 2, 0, 0],
    },
    // NV21
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [1, 2, 0, 0],
    },
    // NV16
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [1, 2, 0, 0],
    },
    // NV61
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [1, 2, 0, 0],
    },
    // NV24
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [1, 2, 0, 0],
    },
    // NV42
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [1, 2, 0, 0],
    },
    // YV12
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // YV16
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // YV24
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [1, 1, 1, 0],
    },
    // YUYV
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // YVYU
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // UYVY
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // VYUY
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(YUV | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // AYUV
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(Alpha | YUV | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // Y8
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: PixelFormatFlags::Planar,
        component_bytes: [1, 0, 0, 0],
    },
    // YA8
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        flags: pix_fmt_flags!(Alpha | Planar),
        component_bytes: [1, 1, 0, 0],
    },
    // RGB30
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        flags: pix_fmt_flags!(RGB | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // BGR30
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        flags: pix_fmt_flags!(RGB | Packed),
        component_bytes: [4, 0, 0, 0],
    },
    // ARGB64
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [8, 0, 0, 0],
    },
    // BGRA64
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [8, 0, 0, 0],
    },
    // ABGR64
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [8, 0, 0, 0],
    },
    // RGBA64
    PixelFormatDescriptor {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(Alpha | RGB | Packed),
        component_bytes: [8, 0, 0, 0],
    },
    // I010
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 10,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I210
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 10,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I410
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I44010
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 1,
        depth: 10,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // P010
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 10,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // P210
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 10,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // P410
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // I012
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 12,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I212
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 12,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I412
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 12,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I44012
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 1,
        depth: 12,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // P012
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 12,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // P212
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 12,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // P412
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 12,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // I016
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 16,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I216
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I416
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // I44016
    PixelFormatDescriptor {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 1,
        depth: 16,
        flags: pix_fmt_flags!(YUV | Planar),
        component_bytes: [2, 2, 2, 0],
    },
    // P016
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 16,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // P216
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
    // P416
    PixelFormatDescriptor {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        flags: pix_fmt_flags!(YUV | BiPlanar),
        component_bytes: [2, 4, 0, 0],
    },
];

impl PixelFormat {
    pub fn components(&self) -> u8 {
        PIXEL_FORMAT_DESC[*self as usize].components
    }

    pub fn component_bytes(&self, component: u8) -> u8 {
        PIXEL_FORMAT_DESC[*self as usize].component_bytes[component as usize]
    }

    pub fn chroma_subsampling(&self) -> Option<ChromaSubsampling> {
        if !self.is_yuv() {
            return None;
        }

        let desc = &PIXEL_FORMAT_DESC[*self as usize];

        match (desc.chroma_shift_x, desc.chroma_shift_y) {
            (1, 1) => Some(ChromaSubsampling::YUV420),
            (1, 0) => Some(ChromaSubsampling::YUV422),
            (0, 0) => Some(ChromaSubsampling::YUV444),
            _ => None,
        }
    }

    pub fn depth(&self) -> u8 {
        PIXEL_FORMAT_DESC[*self as usize].depth
    }

    pub fn is_rgb(&self) -> bool {
        PIXEL_FORMAT_DESC[*self as usize].flags.contains(PixelFormatFlags::RGB)
    }

    pub fn is_yuv(&self) -> bool {
        PIXEL_FORMAT_DESC[*self as usize].flags.contains(PixelFormatFlags::YUV)
    }

    pub fn is_planar(&self) -> bool {
        PIXEL_FORMAT_DESC[*self as usize].flags.contains(PixelFormatFlags::Planar)
    }

    pub fn is_packed(&self) -> bool {
        PIXEL_FORMAT_DESC[*self as usize].flags.contains(PixelFormatFlags::Packed)
    }

    pub fn is_biplanar(&self) -> bool {
        PIXEL_FORMAT_DESC[*self as usize].flags.contains(PixelFormatFlags::BiPlanar)
    }

    pub fn calc_plane_row_bytes(&self, plane_index: usize, width: u32) -> u32 {
        let desc = &PIXEL_FORMAT_DESC[*self as usize];
        let component_bytes = desc.component_bytes[plane_index];

        if plane_index > 0 && (self.is_planar() || self.is_biplanar()) {
            ceil_rshift(width, desc.chroma_shift_x as u32) * component_bytes as u32
        } else {
            width * component_bytes as u32
        }
    }

    pub fn calc_plane_height(&self, plane_index: usize, height: u32) -> u32 {
        if plane_index > 0 && (self.is_planar() || self.is_biplanar()) {
            let desc = &PIXEL_FORMAT_DESC[*self as usize];
            ceil_rshift(height, desc.chroma_shift_y as u32)
        } else {
            height
        }
    }

    pub(crate) fn calc_data(&self, width: u32, height: u32, alignment: u32) -> (usize, PlaneInformationVec) {
        let desc = &PIXEL_FORMAT_DESC[*self as usize];
        let mut size;
        let mut planes = PlaneInformationVec::with_capacity(desc.components as usize);

        match self {
            PixelFormat::RGB24 | PixelFormat::BGR24 | PixelFormat::Y8 => {
                let stride = align_to(width * desc.component_bytes[0] as u32, cmp::max(alignment, 4)) as usize;
                planes.push(PlaneInformation::Video(stride, height));
                size = stride * height as usize;
            }
            PixelFormat::YA8 => {
                let stride = align_to(width * desc.component_bytes[0] as u32, cmp::max(alignment, 4)) as usize;
                planes.extend(iter::repeat_n(PlaneInformation::Video(stride, height), 2));
                size = stride * height as usize * 2;
            }
            PixelFormat::YUYV | PixelFormat::YVYU | PixelFormat::UYVY | PixelFormat::VYUY | PixelFormat::AYUV => {
                let stride = align_to(ceil_rshift(width, desc.chroma_shift_x as u32) * 4, alignment) as usize;
                planes.push(PlaneInformation::Video(stride, height));
                size = stride * height as usize;
            }
            _ => {
                let stride = align_to(width * desc.component_bytes[0] as u32, alignment) as usize;
                planes.push(PlaneInformation::Video(stride, height));
                size = stride * height as usize;
                for i in 1..desc.components as usize {
                    let stride = align_to(ceil_rshift(width, desc.chroma_shift_x as u32) * desc.component_bytes[i] as u32, alignment) as usize;
                    let height = ceil_rshift(height, desc.chroma_shift_y as u32);
                    planes.push(PlaneInformation::Video(stride, height));
                    size += stride * height as usize;
                }
            }
        }

        (size, planes)
    }

    pub(crate) fn calc_data_with_stride(&self, height: u32, stride: usize) -> (usize, PlaneInformationVec) {
        let desc = &PIXEL_FORMAT_DESC[*self as usize];
        let mut size;
        let mut planes = PlaneInformationVec::with_capacity(desc.components as usize);

        planes.push(PlaneInformation::Video(stride, height));
        size = stride * height as usize;
        for i in 1..desc.components as usize {
            let plane_stride = ceil_rshift(stride, desc.chroma_shift_x as usize) * desc.component_bytes[i] as usize;
            let plane_height = ceil_rshift(height, desc.chroma_shift_y as u32);
            planes.push(PlaneInformation::Video(plane_stride, plane_height));
            size += plane_stride * plane_height as usize;
        }

        (size, planes)
    }

    pub(crate) fn calc_chroma_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        let desc = &PIXEL_FORMAT_DESC[*self as usize];
        let chroma_width = ceil_rshift(width, desc.chroma_shift_x as u32);
        let chroma_height = ceil_rshift(height, desc.chroma_shift_y as u32);
        (chroma_width, chroma_height)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, TryFromPrimitive)]
pub enum CompressionFormat {
    #[default]
    MJPEG,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoFormat {
    Pixel(PixelFormat),
    Compression(CompressionFormat),
}

impl Display for VideoFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoFormat::Pixel(format) => write!(f, "{:?}", format),
            VideoFormat::Compression(format) => write!(f, "{:?}", format),
        }
    }
}

impl VideoFormat {
    pub fn is_compressed(&self) -> bool {
        matches!(self, VideoFormat::Compression(_))
    }

    pub fn is_yuv(&self) -> bool {
        match self {
            VideoFormat::Pixel(format) => format.is_yuv(),
            VideoFormat::Compression(CompressionFormat::MJPEG) => true,
        }
    }
}

const COMPRESSION_MASK: u32 = 0x8000;

impl From<VideoFormat> for u32 {
    fn from(value: VideoFormat) -> Self {
        match value {
            VideoFormat::Pixel(format) => format as u32,
            VideoFormat::Compression(format) => format as u32 | COMPRESSION_MASK,
        }
    }
}

impl TryFrom<u32> for VideoFormat {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self> {
        if value & COMPRESSION_MASK != 0 {
            let format_value = value & !COMPRESSION_MASK;
            CompressionFormat::try_from(format_value as u8).map(VideoFormat::Compression).map_err(|e| Error::Invalid(e.to_string()))
        } else {
            PixelFormat::try_from(value as u8).map(VideoFormat::Pixel).map_err(|e| Error::Invalid(e.to_string()))
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChromaLocation {
    #[default]
    Unspecified,
    Left,
    Center,
    TopLeft,
    Top,
    BottomLeft,
    Bottom,
}

impl From<ChromaLocation> for usize {
    fn from(value: ChromaLocation) -> Self {
        value as usize
    }
}

impl From<usize> for ChromaLocation {
    fn from(value: usize) -> Self {
        match value {
            0 => ChromaLocation::Unspecified,
            1 => ChromaLocation::Left,
            2 => ChromaLocation::Center,
            3 => ChromaLocation::TopLeft,
            4 => ChromaLocation::Top,
            5 => ChromaLocation::BottomLeft,
            6 => ChromaLocation::Bottom,
            _ => ChromaLocation::Unspecified,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromaSubsampling {
    YUV420,
    YUV422,
    YUV444,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Rotation {
    #[default]
    None,
    Rotation90,
    Rotation180,
    Rotation270,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Origin {
    #[default]
    TopDown,
    BottomUp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScaleFilter {
    Nearest,
    #[default]
    Bilinear,
    Bicubic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoFrameDescriptor {
    pub format: PixelFormat,
    pub width: NonZeroU32,
    pub height: NonZeroU32,
    pub color_range: ColorRange,
    pub color_matrix: ColorMatrix,
    pub color_primaries: ColorPrimaries,
    pub color_transfer_characteristics: ColorTransferCharacteristics,
    pub chroma_location: ChromaLocation,
    pub rotation: Rotation,
    pub origin: Origin,
    pub transparent: bool,
    pub extra_alpha: bool,
    pub crop_left: u32,
    pub crop_top: u32,
    pub crop_right: u32,
    pub crop_bottom: u32,
}

impl VideoFrameDescriptor {
    pub fn new(format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Self {
        Self {
            format,
            width,
            height,
            color_range: ColorRange::default(),
            color_matrix: ColorMatrix::default(),
            color_primaries: ColorPrimaries::default(),
            color_transfer_characteristics: ColorTransferCharacteristics::default(),
            chroma_location: ChromaLocation::default(),
            rotation: Rotation::default(),
            origin: Origin::default(),
            transparent: false,
            extra_alpha: false,
            crop_left: 0,
            crop_top: 0,
            crop_right: 0,
            crop_bottom: 0,
        }
    }

    pub fn try_new(format: PixelFormat, width: u32, height: u32) -> Result<Self> {
        let width = NonZeroU32::new(width).ok_or(invalid_param_error!(width))?;
        let height = NonZeroU32::new(height).ok_or(invalid_param_error!(height))?;

        Ok(Self::new(format, width, height))
    }
}

impl From<VideoFrameDescriptor> for FrameDescriptor {
    fn from(desc: VideoFrameDescriptor) -> Self {
        FrameDescriptor::Video(desc)
    }
}
