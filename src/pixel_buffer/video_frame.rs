use std::{borrow::Cow, num::NonZeroU32, sync::OnceLock};

use core_foundation::{base::*, boolean::*, dictionary::*, number::CFNumber, string::*};
use core_video::{
    buffer::{kCVAttachmentMode_ShouldPropagate, TCVBuffer},
    image_buffer::{
        color_primaries_get_integer_code_point_for_string, color_primaries_get_string_for_integer_code_point,
        transfer_function_get_integer_code_point_for_string, transfer_function_get_string_for_integer_code_point,
        ycbcr_matrix_get_integer_code_point_for_string, ycbcr_matrix_get_string_for_integer_code_point, CVImageBufferColorPrimaries,
        CVImageBufferKeys, CVImageBufferTransferFunction, CVImageBufferYCbCrMatrix,
    },
    pixel_buffer::*,
    r#return::kCVReturnSuccess,
};
use os_ver::if_greater_than;
use smallvec::SmallVec;

use crate::{
    error::MediaError,
    invalid_param_error,
    media::MediaFrameType,
    media_frame::*,
    none_param_error,
    video::{ColorMatrix, ColorPrimaries, ColorRange, ColorTransferCharacteristics, PixelFormat, VideoFrameDescription},
};

const ITU_R_601_4: &str = "ITU_R_601_4";
const ITU_R_709_2: &str = "ITU_R_709_2";
const ITU_R_2020: &str = "ITU_R_2020";
const ITU_R_2100_HLG: &str = "ITU_R_2100_HLG";
const SMPTE_240M_1995: &str = "SMPTE_240M_1995";
const SMPTE_C: &str = "SMPTE_C";
const SMPTE_ST_428_1: &str = "SMPTE_ST_428_1";
const SMPTE_ST_2084_PQ: &str = "SMPTE_ST_2084_PQ";
const USE_GAMMA: &str = "UseGamma";
const IEC_SRGB: &str = "IEC_sRGB";
const DCI_P3: &str = "DCI_P3";
const P3_D65: &str = "P3_D65";
const EBU_3213: &str = "EBU_3213";
const P22: &str = "P22";
const LINEAR: &str = "Linear";

static PIXEL_FORMATS: OnceLock<[[u32; ColorRange::MAX as usize]; PixelFormat::MAX as usize]> = OnceLock::new();

fn pixel_formats() -> &'static [[u32; ColorRange::MAX as usize]; PixelFormat::MAX as usize] {
    PIXEL_FORMATS.get_or_init(|| {
        let mut formats = [[0; ColorRange::MAX as usize]; PixelFormat::MAX as usize];
        formats[PixelFormat::ARGB32 as usize][ColorRange::Unspecified as usize] = kCVPixelFormatType_32BGRA;
        formats[PixelFormat::BGRA32 as usize][ColorRange::Unspecified as usize] = kCVPixelFormatType_32ARGB;
        formats[PixelFormat::BGR24 as usize][ColorRange::Unspecified as usize] = kCVPixelFormatType_24RGB;
        formats[PixelFormat::I420 as usize][ColorRange::Video as usize] = kCVPixelFormatType_420YpCbCr8Planar;
        formats[PixelFormat::I420 as usize][ColorRange::Full as usize] = kCVPixelFormatType_420YpCbCr8PlanarFullRange;
        formats[PixelFormat::NV12 as usize][ColorRange::Video as usize] = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
        formats[PixelFormat::NV12 as usize][ColorRange::Full as usize] = kCVPixelFormatType_420YpCbCr8BiPlanarFullRange;
        formats[PixelFormat::YUYV as usize][ColorRange::Video as usize] = kCVPixelFormatType_422YpCbCr8_yuvs;
        formats[PixelFormat::UYVY as usize][ColorRange::Full as usize] = kCVPixelFormatType_422YpCbCr8;
        formats
    })
}

fn into_cv_format(format: PixelFormat, color_range: ColorRange) -> u32 {
    pixel_formats()[format as usize][color_range as usize]
}

fn from_cv_format(format: u32) -> (Option<PixelFormat>, ColorRange) {
    for (i, formats) in pixel_formats().iter().enumerate() {
        for (j, &f) in formats.iter().enumerate() {
            if f == format {
                return (PixelFormat::try_from(i).ok(), ColorRange::from(j));
            }
        }
    }
    (None, ColorRange::Unspecified)
}

fn into_cv_color_matrix(color_matrix: ColorMatrix) -> Option<CFString> {
    match color_matrix {
        ColorMatrix::BT470BG | ColorMatrix::SMPTE170M => return Some(CVImageBufferYCbCrMatrix::ITU_R_601_4.into()),
        ColorMatrix::BT709 => return Some(CVImageBufferYCbCrMatrix::ITU_R_709_2.into()),
        ColorMatrix::BT2020CL | ColorMatrix::BT2020NCL => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 11) => {
                    return Some(CVImageBufferYCbCrMatrix::ITU_R_2020.into());
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(9) => {
                    return Some(CVImageBufferYCbCrMatrix::ITU_R_2020.into());
                }}
            }

            return Some(CFString::from_static_string(ITU_R_2020));
        }
        ColorMatrix::SMPTE240M => return Some(CVImageBufferYCbCrMatrix::SMPTE_240M_1995.into()),
        _ => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    return Some(ycbcr_matrix_get_string_for_integer_code_point(color_matrix as i32));
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    return Some(ycbcr_matrix_get_string_for_integer_code_point(color_matrix as i32));
                }}
            }

            return None;
        }
    }
}

fn from_cv_color_matrix(color_matrix: &CFString) -> ColorMatrix {
    match Cow::from(color_matrix).as_ref() {
        ITU_R_709_2 => return ColorMatrix::BT709,
        ITU_R_601_4 => return ColorMatrix::BT470BG,
        SMPTE_240M_1995 => return ColorMatrix::SMPTE240M,
        ITU_R_2020 => return ColorMatrix::BT2020NCL,
        _ => {
            let mut code_point = 0;

            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    code_point = ycbcr_matrix_get_integer_code_point_for_string(color_matrix);
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    code_point = ycbcr_matrix_get_integer_code_point_for_string(color_matrix);
                }}
            }

            if code_point != 0 {
                ColorMatrix::try_from(code_point as usize).ok().unwrap_or(ColorMatrix::Identity)
            } else {
                ColorMatrix::Identity
            }
        }
    }
}

fn into_cv_color_primaries(color_primaries: ColorPrimaries) -> Option<CFString> {
    match color_primaries {
        ColorPrimaries::BT709 => return Some(CVImageBufferColorPrimaries::ITU_R_709_2.into()),
        ColorPrimaries::BT470BG => return Some(CVImageBufferColorPrimaries::EBU_3213.into()),
        ColorPrimaries::SMPTE170M => return Some(CVImageBufferColorPrimaries::SMPTE_C.into()),
        ColorPrimaries::BT2020 => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 11) => {
                    return Some(CVImageBufferColorPrimaries::ITU_R_2020.into());
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(9) => {
                    return Some(CVImageBufferColorPrimaries::ITU_R_2020.into());
                }}
            }

            return Some(CFString::from_static_string(ITU_R_2020));
        }
        ColorPrimaries::Unspecified => return None,
        _ => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    return Some(color_primaries_get_string_for_integer_code_point(color_primaries as i32));
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    return Some(color_primaries_get_string_for_integer_code_point(color_primaries as i32));
                }}
            }

            return None;
        }
    };
}

fn from_cv_color_primaries(color_primaries: &CFString) -> ColorPrimaries {
    match Cow::from(color_primaries).as_ref() {
        ITU_R_709_2 => return ColorPrimaries::BT709,
        EBU_3213 => return ColorPrimaries::BT470BG,
        SMPTE_C => return ColorPrimaries::SMPTE170M,
        P22 => return ColorPrimaries::JEDEC_P22,
        DCI_P3 => return ColorPrimaries::SMPTE431,
        P3_D65 => return ColorPrimaries::SMPTE432,
        ITU_R_2020 => return ColorPrimaries::BT2020,
        _ => {
            let mut code_point = 0;

            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    code_point = color_primaries_get_integer_code_point_for_string(color_primaries);
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    code_point = color_primaries_get_integer_code_point_for_string(color_primaries);
                }}
            }

            if code_point != 0 {
                ColorPrimaries::try_from(code_point as usize).ok().unwrap_or(ColorPrimaries::Unspecified)
            } else {
                ColorPrimaries::Unspecified
            }
        }
    }
}

fn into_cv_color_transfer_function(color_transfer_characteristics: ColorTransferCharacteristics) -> Option<CFString> {
    match color_transfer_characteristics {
        ColorTransferCharacteristics::BT709 => return Some(CVImageBufferTransferFunction::ITU_R_709_2.into()),
        ColorTransferCharacteristics::BT470M | ColorTransferCharacteristics::BT470BG => return Some(CVImageBufferTransferFunction::UseGamma.into()),
        ColorTransferCharacteristics::SMPTE240M => return Some(CVImageBufferTransferFunction::SMPTE_240M_1995.into()),
        ColorTransferCharacteristics::BT2020_10 | ColorTransferCharacteristics::BT2020_12 => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 11) => {
                    return Some(CVImageBufferTransferFunction::ITU_R_2020.into());
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(9) => {
                    return Some(CVImageBufferTransferFunction::ITU_R_2020.into());
                }}
            }

            return Some(CFString::from_static_string(ITU_R_2020));
        }
        ColorTransferCharacteristics::SMPTE2084 => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    return Some(CVImageBufferTransferFunction::SMPTE_ST_2084_PQ.into());
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    return Some(CVImageBufferTransferFunction::SMPTE_ST_2084_PQ.into());
                }}
            }

            return Some(CFString::from_static_string(SMPTE_ST_2084_PQ));
        }
        ColorTransferCharacteristics::SMPTE428 => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 12) => {
                    return Some(CVImageBufferTransferFunction::SMPTE_ST_428_1.into());
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(10) => {
                    return Some(CVImageBufferTransferFunction::SMPTE_ST_428_1.into());
                }}
            }

            return Some(CFString::from_static_string(SMPTE_ST_428_1));
        }
        ColorTransferCharacteristics::ARIB_STD_B67 => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    return Some(CVImageBufferTransferFunction::ITU_R_2100_HLG.into());
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    return Some(CVImageBufferTransferFunction::ITU_R_2100_HLG.into());
                }}
            }

            return Some(CFString::from_static_string(ITU_R_2100_HLG));
        }
        ColorTransferCharacteristics::Unspecified => return None,
        _ => {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    return Some(transfer_function_get_string_for_integer_code_point(color_transfer_characteristics as i32));
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    return Some(transfer_function_get_string_for_integer_code_point(color_transfer_characteristics as i32));
                }}
            }

            return None;
        }
    }
}

const GAMMA_22: f32 = 2.2;
const GAMMA_28: f32 = 2.8;

fn from_cv_color_transfer_function(color_transfer_function: &CFString, gamma: Option<&CFNumber>, bits: u8) -> ColorTransferCharacteristics {
    match Cow::from(color_transfer_function).as_ref() {
        ITU_R_709_2 => return ColorTransferCharacteristics::BT709,
        SMPTE_240M_1995 => return ColorTransferCharacteristics::SMPTE240M,
        USE_GAMMA => {
            if let Some(gamma) = gamma {
                if let Some(gamma) = gamma.to_f32() {
                    if (gamma - GAMMA_22).abs() < f32::EPSILON {
                        return ColorTransferCharacteristics::BT470M;
                    } else if (gamma - GAMMA_28).abs() < f32::EPSILON {
                        return ColorTransferCharacteristics::BT470BG;
                    }
                }
            }

            return ColorTransferCharacteristics::Unspecified;
        }
        IEC_SRGB => return ColorTransferCharacteristics::IEC61966_2_1,
        ITU_R_2020 => match bits {
            10 => return ColorTransferCharacteristics::BT2020_10,
            12 => return ColorTransferCharacteristics::BT2020_12,
            _ => return ColorTransferCharacteristics::Unspecified,
        },
        SMPTE_ST_428_1 => return ColorTransferCharacteristics::SMPTE428,
        SMPTE_ST_2084_PQ => return ColorTransferCharacteristics::SMPTE2084,
        ITU_R_2100_HLG => return ColorTransferCharacteristics::ARIB_STD_B67,
        LINEAR => return ColorTransferCharacteristics::Linear,
        _ => {
            let mut code_point = 0;

            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(10, 13) => {
                    code_point = transfer_function_get_integer_code_point_for_string(color_transfer_function);
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(11) => {
                    code_point = transfer_function_get_integer_code_point_for_string(color_transfer_function);
                }}
            }

            if code_point != 0 {
                ColorTransferCharacteristics::try_from(code_point as usize).ok().unwrap_or(ColorTransferCharacteristics::Unspecified)
            } else {
                ColorTransferCharacteristics::Unspecified
            }
        }
    }
}

impl<'a> MediaFrame<'a> {
    pub fn new_video_frame_with_pixel_buffer(desc: VideoFrameDescription) -> Result<Self, MediaError> {
        let pixel_format = into_cv_format(desc.format, desc.color_range);
        #[cfg(target_os = "macos")]
        let compatibility_key: CFString = {
            if_greater_than! {(10, 11) => {
                CVPixelBufferKeys::MetalCompatibility.into()
            } else {
                CVPixelBufferKeys::OpenGLCompatibility.into()
            }}
        };

        #[cfg(target_os = "ios")]
        let compatibility_key: CFString = {
            if_greater_than! {(8) => {
                CVPixelBufferKeys::MetalCompatibility.into()
            } else {
                CVPixelBufferKeys::OpenGLESCompatibility.into()
            }}
        };

        let options = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[
            (CVPixelBufferKeys::IOSurfaceProperties.into(), CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]).as_CFType()),
            (compatibility_key, CFBoolean::true_value().as_CFType()),
        ]);

        let width = desc.width.get() - desc.crop_left - desc.crop_right;
        let height = desc.height.get() - desc.crop_top - desc.crop_bottom;
        let pixel_buffer = CVPixelBuffer::new(pixel_format, width as usize, height as usize, Some(&options))
            .map_err(|_| MediaError::CreationFailed(stringify!(CVPixelBuffer).to_string()))?;

        let buffer = pixel_buffer.as_buffer();

        if let Some(color_matrix) = into_cv_color_matrix(desc.color_matrix) {
            buffer.set_attachment(&CVImageBufferKeys::YCbCrMatrix.into(), &color_matrix.as_CFType(), kCVAttachmentMode_ShouldPropagate);
        }

        if let Some(color_primaries) = into_cv_color_primaries(desc.color_primaries) {
            buffer.set_attachment(&CVImageBufferKeys::ColorPrimaries.into(), &color_primaries.as_CFType(), kCVAttachmentMode_ShouldPropagate);
        }

        if let Some(color_transfer_function) = into_cv_color_transfer_function(desc.color_transfer_characteristics) {
            buffer.set_attachment(
                &CVImageBufferKeys::TransferFunction.into(),
                &color_transfer_function.as_CFType(),
                kCVAttachmentMode_ShouldPropagate,
            );
        }

        let gamma = match desc.color_transfer_characteristics {
            ColorTransferCharacteristics::BT470M => Some(CFNumber::from(GAMMA_22)),
            ColorTransferCharacteristics::BT470BG => Some(CFNumber::from(GAMMA_28)),
            _ => None,
        };

        if let Some(gamma) = gamma {
            buffer.set_attachment(&CVImageBufferKeys::GammaLevel.into(), &gamma.as_CFType(), kCVAttachmentMode_ShouldPropagate);
        }

        let video_frame = Self {
            media_type: MediaFrameType::Video,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescription::Video(desc),
            metadata: None,
            data: MediaFrameData::PixelBuffer(pixel_buffer),
        };

        Ok(video_frame)
    }

    pub fn from_pixel_buffer(pixel_buffer: &CVPixelBuffer) -> Result<Self, MediaError> {
        let width = pixel_buffer.get_width() as u32;
        let width = NonZeroU32::new(width).ok_or(invalid_param_error!(width))?;
        let height = pixel_buffer.get_height() as u32;
        let height = NonZeroU32::new(height).ok_or(invalid_param_error!(height))?;
        let format = pixel_buffer.get_pixel_format();
        let (format, color_range) = from_cv_format(format);
        let format = format.ok_or(none_param_error!(format))?;
        let mut desc = VideoFrameDescription::new(format, width, height);
        desc.color_range = color_range;

        let buffer = pixel_buffer.as_buffer();
        let (color_matrix, color_primaries, color_transfer_function, gamma) = {
            #[cfg(target_os = "macos")]
            {
                if_greater_than! {(12, 1) => {
                    (
                        buffer.copy_attachment(&CVImageBufferKeys::YCbCrMatrix.into(), None),
                        buffer.copy_attachment(&CVImageBufferKeys::ColorPrimaries.into(), None),
                        buffer.copy_attachment(&CVImageBufferKeys::TransferFunction.into(), None),
                        buffer.copy_attachment(&CVImageBufferKeys::GammaLevel.into(), None),
                    )
                } else {
                    (
                        buffer.get_attachment(&CVImageBufferKeys::YCbCrMatrix.into(), None),
                        buffer.get_attachment(&CVImageBufferKeys::ColorPrimaries.into(), None),
                        buffer.get_attachment(&CVImageBufferKeys::TransferFunction.into(), None),
                        buffer.get_attachment(&CVImageBufferKeys::GammaLevel.into(), None),
                    )
                }}
            }

            #[cfg(target_os = "ios")]
            {
                if_greater_than! {(15) => {
                    (
                        buffer.copy_attachment(&CVImageBufferKeys::YCbCrMatrix.into(), None),
                        buffer.copy_attachment(&CVImageBufferKeys::ColorPrimaries.into(), None),
                        buffer.copy_attachment(&CVImageBufferKeys::TransferFunction.into(), None),
                        buffer.copy_attachment(&CVImageBufferKeys::GammaLevel.into(), None),
                    )
                } else {
                    (
                        buffer.get_attachment(&CVImageBufferKeys::YCbCrMatrix.into(), None),
                        buffer.get_attachment(&CVImageBufferKeys::ColorPrimaries.into(), None),
                        buffer.get_attachment(&CVImageBufferKeys::TransferFunction.into(), None),
                        buffer.get_attachment(&CVImageBufferKeys::GammaLevel.into(), None),
                    )
                }}
            }
        };

        if let Some(color_matrix) = color_matrix {
            if let Some(color_matrix) = color_matrix.downcast_into::<CFString>() {
                desc.color_matrix = from_cv_color_matrix(&color_matrix);
            }
        }

        if let Some(color_primaries) = color_primaries {
            if let Some(color_primaries) = color_primaries.downcast_into::<CFString>() {
                desc.color_primaries = from_cv_color_primaries(&color_primaries);
            }
        }

        if let Some(color_transfer_function) = color_transfer_function {
            let color_transfer_function = color_transfer_function.downcast_into::<CFString>();
            if let Some(color_transfer_function) = color_transfer_function {
                let gamma = gamma.and_then(|gamma| gamma.downcast_into::<CFNumber>());
                let depth = format.depth();
                desc.color_transfer_characteristics = from_cv_color_transfer_function(&color_transfer_function, gamma.as_ref(), depth);
            }
        }

        Ok(Self {
            media_type: MediaFrameType::Video,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescription::Video(desc),
            metadata: None,
            data: MediaFrameData::PixelBuffer(pixel_buffer.clone()),
        })
    }
}

impl<'a> DataMappable<'a> for CVPixelBuffer {
    fn map(&self) -> Result<MappedGuard, MediaError> {
        if self.lock_base_address(kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess {
            return Err(MediaError::Failed("lock base address".to_string()).into());
        }

        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard, MediaError> {
        if self.lock_base_address(0) != kCVReturnSuccess {
            return Err(MediaError::Failed("lock base address".to_string()).into());
        }

        Ok(MappedGuard {
            data_ref: DataRef::Mutable(self),
        })
    }

    fn unmap(&self) -> Result<(), MediaError> {
        if self.unlock_base_address(kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess {
            return Err(MediaError::Failed("unlock base address".to_string()).into());
        }
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<(), MediaError> {
        if self.unlock_base_address(0) != kCVReturnSuccess {
            return Err(MediaError::Failed("unlock base address".to_string()).into());
        }
        Ok(())
    }

    fn planes(&'a self) -> Option<MappedPlanes<'a>> {
        let mut planes = SmallVec::<[MappedPlane<'a>; MEDIA_FRAME_MAX_PLANES]>::new();

        if self.is_planar() {
            let plane_count = self.get_plane_count();
            for i in 0..plane_count {
                let base_address = unsafe { self.get_base_address_of_plane(i) as *const u8 };
                let bytes_per_row = self.get_bytes_per_row_of_plane(i);
                let height = self.get_height_of_plane(i);
                let slice = unsafe { std::slice::from_raw_parts(base_address, bytes_per_row * height) };
                planes.push(MappedPlane::Video {
                    data: MappedData::Ref(slice),
                    stride: bytes_per_row as u32,
                    height: height as u32,
                });
            }
        } else {
            let base_address = unsafe { self.get_base_address() as *const u8 };
            let bytes_per_row = self.get_bytes_per_row();
            let height = self.get_height();
            let slice = unsafe { std::slice::from_raw_parts(base_address, bytes_per_row * height) };
            planes.push(MappedPlane::Video {
                data: MappedData::Ref(slice),
                stride: bytes_per_row as u32,
                height: height as u32,
            });
        }

        Some(MappedPlanes {
            planes,
        })
    }

    fn planes_mut(&'a mut self) -> Option<MappedPlanes<'a>> {
        let mut planes = SmallVec::<[MappedPlane<'a>; MEDIA_FRAME_MAX_PLANES]>::new();

        if self.is_planar() {
            let plane_count = self.get_plane_count();
            for i in 0..plane_count {
                let base_address = unsafe { self.get_base_address_of_plane(i) as *mut u8 };
                let bytes_per_row = self.get_bytes_per_row_of_plane(i);
                let height = self.get_height_of_plane(i);
                let slice = unsafe { std::slice::from_raw_parts_mut(base_address, bytes_per_row * height) };
                planes[i] = MappedPlane::Video {
                    data: MappedData::RefMut(slice),
                    stride: bytes_per_row as u32,
                    height: height as u32,
                };
            }
        } else {
            let base_address = unsafe { self.get_base_address() as *mut u8 };
            let bytes_per_row = self.get_bytes_per_row();
            let height = self.get_height();
            let slice = unsafe { std::slice::from_raw_parts_mut(base_address, bytes_per_row * height) };
            planes[0] = MappedPlane::Video {
                data: MappedData::RefMut(slice),
                stride: bytes_per_row as u32,
                height: height as u32,
            };
        }

        Some(MappedPlanes {
            planes,
        })
    }
}
