use std::{
    cmp,
    fmt::{Display, Formatter},
    iter,
    num::NonZeroU32,
};

use bitflags::bitflags;
use num_enum::TryFromPrimitive;

use super::{
    align_to, ceil_rshift,
    media_frame::{MemoryPlanes, PlaneInformation},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

pub const RESOLUTION_SQCIF: Resolution = Resolution {
    width: 128,
    height: 96,
};
pub const RESOLUTION_QCIF: Resolution = Resolution {
    width: 176,
    height: 144,
};

pub const RESOLUTION_CIF: Resolution = Resolution {
    width: 352,
    height: 288,
};
pub const RESOLUTION_QQVGA: Resolution = Resolution {
    width: 160,
    height: 120,
};
pub const RESOLUTION_QVGA: Resolution = Resolution {
    width: 320,
    height: 240,
};
pub const RESOLUTION_VGA: Resolution = Resolution {
    width: 640,
    height: 480,
};
pub const RESOLUTION_SVGA: Resolution = Resolution {
    width: 800,
    height: 600,
};
pub const RESOLUTION_XGA: Resolution = Resolution {
    width: 1024,
    height: 768,
};
pub const RESOLUTION_SXGA: Resolution = Resolution {
    width: 1280,
    height: 1024,
};
pub const RESOLUTION_UXGA: Resolution = Resolution {
    width: 1600,
    height: 1200,
};
pub const RESOLUTION_QXGA: Resolution = Resolution {
    width: 2048,
    height: 1536,
};
pub const RESOLUTION_SD: Resolution = Resolution {
    width: 720,
    height: 480,
};
pub const RESOLUTION_HD: Resolution = Resolution {
    width: 1280,
    height: 720,
};
pub const RESOLUTION_FHD: Resolution = Resolution {
    width: 1920,
    height: 1080,
};
pub const RESOLUTION_QHD: Resolution = Resolution {
    width: 2560,
    height: 1440,
};
pub const RESOLUTION_UHD_4K: Resolution = Resolution {
    width: 3840,
    height: 2160,
};
pub const RESOLUTION_UHD_8K: Resolution = Resolution {
    width: 7680,
    height: 4320,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(u8)]
pub enum ColorRange {
    #[default]
    Unspecified,
    Video,
    Full,
    MAX,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(u8)]
pub enum ColorMatrix {
    #[default]
    Identity = 0, // The identity matrix
    BT709,            // BT.709
    Unspecified,      // Unspecified
    FCC      = 4,     // FCC Title 47 Code of Federal Regulations 73.682(a)(20)
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

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

#[allow(non_camel_case_types)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, TryFromPrimitive)]
pub enum PixelFormat {
    #[default]
    ARGB32 = 0, // packed ARGB, 32 bits, little-endian, BGRA in memory
    BGRA32, // packed BGRA, 32 bits, little-endian, ARGB in memory
    ABGR32, // packed ABGR, 32 bits, little-endian, RGBA in memory
    RGBA32, // packed RGBA, 32 bits, little-endian, ABGR in memory
    RGB24,  // packed RGB, 24 bits, little-endian, BGR in memory
    BGR24,  // packed BGR, 24 bits, little-endian, RGB in memory
    I420,   // planar YUV 4:2:0, 12 bits
    I422,   // planar YUV 4:2:2, 16 bits
    I444,   // planar YUV 4:4:4, 24 bits
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
    RGB30,  // packed RGB, 30 bits, 10 bits per channel, little-endian
    BGR30,  // packed BGR, 30 bits, 10 bits per channel, little-endian
    ARGB64, // packed ARGB, 64 bits, 16 bits per channel, little-endian
    ABGR64, // packed ABGR, 64 bits, 16 bits per channel, little-endian
    I010,   // planar YUV 4:2:0, 10 bits per channel
    I210,   // planar YUV 4:2:2, 10 bits per channel
    I410,   // planar YUV 4:4:4, 10 bits per channel
    P010,   // biplanar YUV 4:2:0, 10 bits per channel
    P210,   // biplanar YUV 4:2:2, 10 bits per channel
    P410,   // biplanar YUV 4:4:4, 10 bits per channel
    I012,   // planar YUV 4:2:2, 12 bits per channel
    I212,   // planar YUV 4:2:2, 12 bits per channel
    I412,   // planar YUV 4:4:4, 12 bits per channel
    P012,   // biplanar YUV 4:2:0, 12 bits per channel
    P212,   // biplanar YUV 4:2:2, 12 bits per channel
    P412,   // biplanar YUV 4:4:4, 12 bits per channel
    I016,   // planar YUV 4:2:0, 16 bits per channel
    I216,   // planar YUV 4:2:2, 16 bits per channel
    I416,   // planar YUV 4:4:4, 16 bits per channel
    P016,   // biplanar YUV 4:2:0, 16 bits per channel
    P216,   // biplanar YUV 4:2:2, 16 bits per channel
    P416,   // biplanar YUV 4:4:4, 16 bits per channel
    MAX,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, TryFromPrimitive)]
pub enum CompressionFormat {
    #[default]
    MJPEG,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VideoFormat {
    Pixel(PixelFormat),
    Compression(CompressionFormat),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChromaSubsampling {
    YUV420,
    YUV422,
    YUV444,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Rotation {
    #[default]
    None,
    Rotation90,
    Rotation180,
    Rotation270,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Origin {
    #[default]
    TopDown,
    BottomUp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoFrameDescription {
    pub format: PixelFormat,
    pub color_range: ColorRange,
    pub color_matrix: ColorMatrix,
    pub color_primaries: ColorPrimaries,
    pub color_transfer_characteristics: ColorTransferCharacteristics,
    pub width: NonZeroU32,
    pub height: NonZeroU32,
    pub rotation: Rotation,
    pub origin: Origin,
    pub transparent: bool,
    pub extra_alpha: bool,
    pub crop_left: u32,
    pub crop_top: u32,
    pub crop_right: u32,
    pub crop_bottom: u32,
}

impl VideoFrameDescription {
    pub fn new(format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Self {
        Self {
            format,
            color_range: ColorRange::default(),
            color_matrix: ColorMatrix::default(),
            color_primaries: ColorPrimaries::default(),
            color_transfer_characteristics: ColorTransferCharacteristics::default(),
            width,
            height,
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
}

bitflags! {
    struct ColorInfo: u32 {
        const Alpha    = 1 << 0;
        const RGB      = 1 << 1;
        const YUV      = 1 << 2;
        const Planar   = 1 << 3;
        const Packed   = 1 << 4;
        const BiPlanar = 1 << 5;
    }
}

struct PixelDescription {
    pub components: u8,
    pub chroma_shift_x: u8,
    pub chroma_shift_y: u8,
    pub depth: u8,
    pub color_info: u32,
    pub component_bytes: [u8; 4],
}

static PIXEL_DESC: [PixelDescription; PixelFormat::MAX as usize] = [
    // ARGB32
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // BGRA32
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // ABGR32
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // RGBA32
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // RGB24
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [3, 0, 0, 0],
    },
    // BGR24
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [3, 0, 0, 0],
    },
    // I420
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 1, 0],
    },
    // I422
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 1, 0],
    },
    // I444
    PixelDescription {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 1, 0],
    },
    // NV12
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [1, 2, 0, 0],
    },
    // NV21
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [1, 2, 0, 0],
    },
    // NV16
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [1, 2, 0, 0],
    },
    // NV61
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [1, 2, 0, 0],
    },
    // NV24
    PixelDescription {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [1, 2, 0, 0],
    },
    // NV42
    PixelDescription {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [1, 2, 0, 0],
    },
    // YV12
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 1, 0],
    },
    // YV16
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 1, 0],
    },
    // YV24
    PixelDescription {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 1, 0],
    },
    // YUYV
    PixelDescription {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // YVYU
    PixelDescription {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // UYVY
    PixelDescription {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // VYUY
    PixelDescription {
        components: 1,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // AYUV
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::YUV.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // Y8
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Planar.bits(),
        component_bytes: [1, 0, 0, 0],
    },
    // YA8
    PixelDescription {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 8,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::Planar.bits(),
        component_bytes: [1, 1, 0, 0],
    },
    // RGB30
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        color_info: ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // BGR30
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        color_info: ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [4, 0, 0, 0],
    },
    // ARGB64
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [8, 0, 0, 0],
    },
    // ABGR64
    PixelDescription {
        components: 1,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        color_info: ColorInfo::Alpha.bits() | ColorInfo::RGB.bits() | ColorInfo::Packed.bits(),
        component_bytes: [8, 0, 0, 0],
    },
    // I010
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 10,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // I210
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 10,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // I410
    PixelDescription {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // P010
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 10,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // P210
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 10,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // P410
    PixelDescription {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 10,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // I012
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 12,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // I212
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 12,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // I412
    PixelDescription {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 12,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // P012
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 12,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // P212
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 12,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // P412
    PixelDescription {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 12,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // I016
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 16,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // I216
    PixelDescription {
        components: 3,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 16,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // I416
    PixelDescription {
        components: 3,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        color_info: ColorInfo::YUV.bits() | ColorInfo::Planar.bits(),
        component_bytes: [2, 2, 2, 0],
    },
    // P016
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 1,
        depth: 16,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // P216
    PixelDescription {
        components: 2,
        chroma_shift_x: 1,
        chroma_shift_y: 0,
        depth: 16,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
    // P416
    PixelDescription {
        components: 2,
        chroma_shift_x: 0,
        chroma_shift_y: 0,
        depth: 16,
        color_info: ColorInfo::YUV.bits() | ColorInfo::BiPlanar.bits(),
        component_bytes: [2, 4, 0, 0],
    },
];

impl PixelFormat {
    pub fn components(&self) -> u8 {
        PIXEL_DESC[*self as usize].components
    }

    pub fn component_bytes(&self, component: u8) -> u8 {
        PIXEL_DESC[*self as usize].component_bytes[component as usize]
    }

    pub fn chroma_subsampling(&self) -> Option<ChromaSubsampling> {
        if !self.is_yuv() {
            return None;
        }

        let desc = &PIXEL_DESC[*self as usize];

        match (desc.chroma_shift_x, desc.chroma_shift_y) {
            (1, 1) => Some(ChromaSubsampling::YUV420),
            (1, 0) => Some(ChromaSubsampling::YUV422),
            (0, 0) => Some(ChromaSubsampling::YUV444),
            _ => None,
        }
    }

    pub fn depth(&self) -> u8 {
        PIXEL_DESC[*self as usize].depth
    }

    pub fn is_rgb(&self) -> bool {
        PIXEL_DESC[*self as usize].color_info & ColorInfo::RGB.bits() != 0
    }

    pub fn is_yuv(&self) -> bool {
        PIXEL_DESC[*self as usize].color_info & ColorInfo::YUV.bits() != 0
    }

    pub fn is_planar(&self) -> bool {
        PIXEL_DESC[*self as usize].color_info & ColorInfo::Planar.bits() != 0
    }

    pub fn is_packed(&self) -> bool {
        PIXEL_DESC[*self as usize].color_info & ColorInfo::Packed.bits() != 0
    }

    pub fn is_biplanar(&self) -> bool {
        PIXEL_DESC[*self as usize].color_info & ColorInfo::BiPlanar.bits() != 0
    }

    pub(super) fn calc_data(&self, width: u32, height: u32, alignment: u32) -> (u32, MemoryPlanes) {
        let desc = &PIXEL_DESC[*self as usize];
        let mut size;
        let mut planes = MemoryPlanes::with_capacity(desc.components as usize);

        match self {
            PixelFormat::RGB24 | PixelFormat::BGR24 | PixelFormat::Y8 => {
                let stride = align_to(width * desc.component_bytes[0] as u32, cmp::max(alignment, 4));
                planes.push(PlaneInformation::Video(stride, height));
                size = stride * height;
            }
            PixelFormat::YA8 => {
                let stride = align_to(width * desc.component_bytes[0] as u32, cmp::max(alignment, 4));
                planes.extend(iter::repeat(PlaneInformation::Video(stride, height)).take(2));
                size = stride * height * 2;
            }
            PixelFormat::YUYV | PixelFormat::YVYU | PixelFormat::UYVY | PixelFormat::VYUY | PixelFormat::AYUV => {
                let stride = align_to(ceil_rshift(width, desc.chroma_shift_x as u32) * 4, alignment);
                planes.push(PlaneInformation::Video(stride, height));
                size = stride * height;
            }
            _ => {
                let stride = align_to(width * desc.component_bytes[0] as u32, alignment);
                planes.push(PlaneInformation::Video(stride, height));
                size = stride * height;
                for i in 1..desc.components as usize {
                    let stride = align_to(ceil_rshift(width, desc.chroma_shift_x as u32) * desc.component_bytes[i as usize] as u32, alignment);
                    let height = ceil_rshift(height, desc.chroma_shift_y as u32);
                    planes.push(PlaneInformation::Video(stride, height));
                    size += stride * height;
                }
            }
        }

        (size, planes)
    }

    pub(super) fn calc_data_with_stride(&self, height: u32, stride: u32) -> (u32, MemoryPlanes) {
        let desc = &PIXEL_DESC[*self as usize];
        let mut size;
        let mut planes = MemoryPlanes::with_capacity(desc.components as usize);

        planes.push(PlaneInformation::Video(stride, height));
        size = stride * height;
        for i in 1..desc.components as usize {
            let plane_stride = ceil_rshift(stride, desc.chroma_shift_x as u32) * desc.component_bytes[i as usize] as u32;
            let plane_height = ceil_rshift(height, desc.chroma_shift_y as u32);
            planes.push(PlaneInformation::Video(plane_stride, plane_height));
            size = size + plane_stride * plane_height;
        }

        (size, planes)
    }
}

impl TryFrom<usize> for PixelFormat {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value <= PixelFormat::MAX as usize {
            Ok(unsafe { std::mem::transmute(value as u8) })
        } else {
            Err(())
        }
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

impl TryFrom<usize> for ColorMatrix {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
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
            _ => Err(()),
        }
    }
}

impl TryFrom<usize> for ColorPrimaries {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
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
            _ => Err(()),
        }
    }
}

impl TryFrom<usize> for ColorTransferCharacteristics {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
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
            _ => Err(()),
        }
    }
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

impl Into<u32> for VideoFormat {
    fn into(self) -> u32 {
        match self {
            VideoFormat::Pixel(format) => format as u32,
            VideoFormat::Compression(format) => format as u32 | COMPRESSION_MASK,
        }
    }
}

impl TryFrom<u32> for VideoFormat {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value & COMPRESSION_MASK != 0 {
            let format_value = value & !COMPRESSION_MASK;
            CompressionFormat::try_from(format_value as u8).map(VideoFormat::Compression).map_err(|_| ())
        } else {
            PixelFormat::try_from(value as u8).map(VideoFormat::Pixel).map_err(|_| ())
        }
    }
}
