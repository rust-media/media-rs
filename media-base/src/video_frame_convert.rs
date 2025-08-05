use std::{fmt::Debug, num::NonZeroU32, sync::LazyLock};

use bytemuck::{self, Pod};
use yuv::{
    self, BufferStoreMut, Rgb30ByteOrder::Network, YuvBiPlanarImage, YuvBiPlanarImageMut, YuvConversionMode, YuvConversionMode::Fast, YuvPackedImage,
    YuvPackedImageMut, YuvPlanarImage, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
};

use crate::{
    error::Error,
    frame::{Frame, MappedPlanes},
    media::FrameDescriptor,
    video::{ColorMatrix, ColorRange, PixelFormat},
    Result,
};

fn into_yuv_planar_image<'a, T>(src: &'a MappedPlanes, width: NonZeroU32, height: NonZeroU32) -> Result<YuvPlanarImage<'a, T>>
where
    T: Copy + Debug + Pod,
{
    if src.planes.len() != 3 {
        return Err(Error::Invalid("invalid plane count".to_string()));
    }

    let planes = &src.planes;
    let size = size_of::<T>() as u32;

    Ok(YuvPlanarImage::<T> {
        y_plane: bytemuck::cast_slice(planes[0].data().unwrap()),
        y_stride: planes[0].stride().unwrap() / size,
        u_plane: bytemuck::cast_slice(planes[1].data().unwrap()),
        u_stride: planes[1].stride().unwrap() / size,
        v_plane: bytemuck::cast_slice(planes[2].data().unwrap()),
        v_stride: planes[2].stride().unwrap() / size,
        width: width.get(),
        height: height.get(),
    })
}

fn into_yuv_planar_image_mut<'a, T>(dst: &'a mut MappedPlanes, width: NonZeroU32, height: NonZeroU32) -> Result<YuvPlanarImageMut<'a, T>>
where
    T: Copy + Debug + Pod,
{
    if dst.planes.len() != 3 {
        return Err(Error::Invalid("invalid plane count".to_string()));
    }

    let planes = dst.planes.as_mut_slice();
    let size = size_of::<T>() as u32;

    let y_stride = planes[0].stride().unwrap() / size;
    let u_stride = planes[1].stride().unwrap() / size;
    let v_stride = planes[2].stride().unwrap() / size;

    let (y_plane, rest) = planes.split_at_mut(1);
    let (u_plane, v_plane) = rest.split_at_mut(1);

    Ok(YuvPlanarImageMut::<T> {
        y_plane: BufferStoreMut::Borrowed(bytemuck::cast_slice_mut(y_plane[0].data_mut().unwrap())),
        y_stride,
        u_plane: BufferStoreMut::Borrowed(bytemuck::cast_slice_mut(u_plane[0].data_mut().unwrap())),
        u_stride,
        v_plane: BufferStoreMut::Borrowed(bytemuck::cast_slice_mut(v_plane[0].data_mut().unwrap())),
        v_stride,
        width: width.get(),
        height: height.get(),
    })
}

fn into_yuv_bi_planar_image<'a, T>(src: &'a MappedPlanes, width: NonZeroU32, height: NonZeroU32) -> Result<YuvBiPlanarImage<'a, T>>
where
    T: Copy + Debug + Pod,
{
    if src.planes.len() != 2 {
        return Err(Error::Invalid("invalid plane count".to_string()));
    }

    let planes = &src.planes;
    let size = size_of::<T>() as u32;

    Ok(YuvBiPlanarImage::<T> {
        y_plane: bytemuck::cast_slice(planes[0].data().unwrap()),
        y_stride: planes[0].stride().unwrap() / size,
        uv_plane: bytemuck::cast_slice(planes[1].data().unwrap()),
        uv_stride: planes[1].stride().unwrap() / size,
        width: width.get(),
        height: height.get(),
    })
}

fn into_yuv_bi_planar_image_mut<'a, T>(dst: &'a mut MappedPlanes, width: NonZeroU32, height: NonZeroU32) -> Result<YuvBiPlanarImageMut<'a, T>>
where
    T: Copy + Debug + Pod,
{
    if dst.planes.len() != 2 {
        return Err(Error::Invalid("invalid plane count".to_string()));
    }

    let planes = dst.planes.as_mut_slice();
    let size = size_of::<T>() as u32;

    let y_stride = planes[0].stride().unwrap() / size;
    let uv_stride = planes[1].stride().unwrap() / size;

    let (y_plane, uv_plane) = planes.split_at_mut(1);

    Ok(YuvBiPlanarImageMut::<T> {
        y_plane: BufferStoreMut::Borrowed(bytemuck::cast_slice_mut(y_plane[0].data_mut().unwrap())),
        y_stride,
        uv_plane: BufferStoreMut::Borrowed(bytemuck::cast_slice_mut(uv_plane[0].data_mut().unwrap())),
        uv_stride,
        width: width.get(),
        height: height.get(),
    })
}

fn into_yuv_packed_image<'a, T>(src: &'a MappedPlanes, width: NonZeroU32, height: NonZeroU32) -> Result<YuvPackedImage<'a, T>>
where
    T: Copy + Debug + Pod,
{
    if src.planes.len() != 1 {
        return Err(Error::Invalid("invalid plane count".to_string()));
    }

    let planes = &src.planes;
    let size = size_of::<T>() as u32;

    Ok(YuvPackedImage::<T> {
        yuy: bytemuck::cast_slice(planes[0].data().unwrap()),
        yuy_stride: planes[0].stride().unwrap() / size,
        width: width.get(),
        height: height.get(),
    })
}

fn into_yuv_packed_image_mut<'a, T>(dst: &'a mut MappedPlanes, width: NonZeroU32, height: NonZeroU32) -> Result<YuvPackedImageMut<'a, T>>
where
    T: Copy + Debug + Pod,
{
    if dst.planes.len() != 1 {
        return Err(Error::Invalid("invalid plane count".to_string()));
    }

    let planes = dst.planes.as_mut();
    let size = size_of::<T>() as u32;

    let yuy_stride = planes[0].stride().unwrap() / size;

    Ok(YuvPackedImageMut::<T> {
        yuy: BufferStoreMut::Borrowed(bytemuck::cast_slice_mut(planes[0].data_mut().unwrap())),
        yuy_stride,
        width: width.get(),
        height: height.get(),
    })
}

impl Into<YuvRange> for ColorRange {
    fn into(self) -> YuvRange {
        match self {
            ColorRange::Video => YuvRange::Limited,
            ColorRange::Full => YuvRange::Full,
            _ => YuvRange::Limited,
        }
    }
}

impl Into<YuvStandardMatrix> for ColorMatrix {
    fn into(self) -> YuvStandardMatrix {
        match self {
            ColorMatrix::BT709 => YuvStandardMatrix::Bt709,
            ColorMatrix::BT2020CL | ColorMatrix::BT2020NCL => YuvStandardMatrix::Bt2020,
            ColorMatrix::SMPTE240M => YuvStandardMatrix::Smpte240,
            ColorMatrix::BT470BG => YuvStandardMatrix::Bt470_6,
            ColorMatrix::FCC => YuvStandardMatrix::Fcc,
            _ => YuvStandardMatrix::Bt601,
        }
    }
}

macro_rules! impl_rgb_to_rgb {
    ($func_name:ident, $convert_func:ident) => {
        fn $func_name(
            src: &MappedPlanes,
            dst: &mut MappedPlanes,
            _color_range: ColorRange,
            _color_matrix: ColorMatrix,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let dst_stride = dst.plane_stride(0).unwrap();

            yuv::$convert_func(
                src.plane_data(0).unwrap(),
                src.plane_stride(0).unwrap(),
                dst.plane_data_mut(0).unwrap(),
                dst_stride,
                width.get(),
                height.get(),
            )
            .map_err(|e| Error::Invalid(e.to_string()))?;

            Ok(())
        }
    };
}

macro_rules! impl_rgb_to_yuv {
    ($func_name:ident, $convert_func:ident, $into_image_func:ident) => {
        fn $func_name(
            src: &MappedPlanes,
            dst: &mut MappedPlanes,
            color_range: ColorRange,
            color_matrix: ColorMatrix,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let mut yuv_image = $into_image_func(dst, width, height)?;

            yuv::$convert_func(
                &mut yuv_image,
                src.plane_data(0).unwrap(),
                src.plane_stride(0).unwrap(),
                color_range.into(),
                color_matrix.into(),
                YuvConversionMode::Fast,
            )
            .map_err(|e| Error::Invalid(e.to_string()))?;

            Ok(())
        }
    };
}

macro_rules! impl_yuv_to_rgb {
    ($func_name:ident, $convert_func:ident, $into_image_func:ident) => {
        fn $func_name(
            src: &MappedPlanes,
            dst: &mut MappedPlanes,
            color_range: ColorRange,
            color_matrix: ColorMatrix,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let yuv_image = $into_image_func(src, width, height)?;
            let dst_stride = dst.plane_stride(0).unwrap();

            yuv::$convert_func(&yuv_image, dst.plane_data_mut(0).unwrap(), dst_stride, color_range.into(), color_matrix.into())
                .map_err(|e| Error::Invalid(e.to_string()))?;

            Ok(())
        }
    };
}

macro_rules! impl_yuv_to_rgb_with_conversion_mode {
    ($func_name:ident, $convert_func:ident, $into_image_func:ident, $conversion_mode:ident) => {
        fn $func_name(
            src: &MappedPlanes,
            dst: &mut MappedPlanes,
            color_range: ColorRange,
            color_matrix: ColorMatrix,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let yuv_image = $into_image_func(src, width, height)?;
            let dst_stride = dst.plane_stride(0).unwrap();

            yuv::$convert_func(&yuv_image, dst.plane_data_mut(0).unwrap(), dst_stride, color_range.into(), color_matrix.into(), $conversion_mode)
                .map_err(|e| Error::Invalid(e.to_string()))?;

            Ok(())
        }
    };
}

macro_rules! impl_yuv_to_rgb_with_byte_order {
    ($func_name:ident, $convert_func:ident, $into_image_func:ident, $byte_order:ident) => {
        fn $func_name(
            src: &MappedPlanes,
            dst: &mut MappedPlanes,
            color_range: ColorRange,
            color_matrix: ColorMatrix,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let yuv_image = $into_image_func(src, width, height)?;
            let dst_stride = dst.plane_stride(0).unwrap();

            yuv::$convert_func(&yuv_image, dst.plane_data_mut(0).unwrap(), dst_stride, $byte_order, color_range.into(), color_matrix.into())
                .map_err(|e| Error::Invalid(e.to_string()))?;

            Ok(())
        }
    };
}

macro_rules! impl_yuv_to_yuv {
    ($func_name:ident, $convert_func:ident, $into_src_image_func:ident, $into_dst_image_func:ident) => {
        fn $func_name(
            src: &MappedPlanes,
            dst: &mut MappedPlanes,
            _color_range: ColorRange,
            _color_matrix: ColorMatrix,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let src_image = $into_src_image_func(src, width, height)?;
            let mut dst_image = $into_dst_image_func(dst, width, height)?;

            yuv::$convert_func(&mut dst_image, &src_image).map_err(|e| Error::Invalid(e.to_string()))?;

            Ok(())
        }
    };
}

impl_rgb_to_rgb!(bgra32_to_rgba32, bgra_to_rgba);

impl_rgb_to_yuv!(bgra32_to_i420, bgra_to_yuv420, into_yuv_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_i422, bgra_to_yuv422, into_yuv_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_i444, bgra_to_yuv444, into_yuv_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_nv12, bgra_to_yuv_nv12, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_nv16, bgra_to_yuv_nv16, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_nv24, bgra_to_yuv_nv24, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_nv21, bgra_to_yuv_nv21, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_nv61, bgra_to_yuv_nv61, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(bgra32_to_nv42, bgra_to_yuv_nv42, into_yuv_bi_planar_image_mut);

impl_rgb_to_rgb!(rgba32_to_bgra32, rgba_to_bgra);

impl_rgb_to_yuv!(rgba32_to_i420, rgba_to_yuv420, into_yuv_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_i422, rgba_to_yuv422, into_yuv_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_i444, rgba_to_yuv444, into_yuv_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_nv12, rgba_to_yuv_nv12, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_nv16, rgba_to_yuv_nv16, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_nv24, rgba_to_yuv_nv24, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_nv21, rgba_to_yuv_nv21, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_nv61, rgba_to_yuv_nv61, into_yuv_bi_planar_image_mut);
impl_rgb_to_yuv!(rgba32_to_nv42, rgba_to_yuv_nv42, into_yuv_bi_planar_image_mut);

impl_yuv_to_rgb!(i420_to_bgra32, yuv420_to_bgra, into_yuv_planar_image);
impl_yuv_to_rgb!(i420_to_rgba32, yuv420_to_rgba, into_yuv_planar_image);
impl_yuv_to_rgb!(i420_to_bgr24, yuv420_to_bgr, into_yuv_planar_image);
impl_yuv_to_rgb!(i420_to_rgb24, yuv420_to_rgb, into_yuv_planar_image);

impl_yuv_to_yuv!(i420_to_yuyv, yuv420_to_yuyv422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i420_to_yvyu, yuv420_to_yvyu422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i420_to_uyvy, yuv420_to_uyvy422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i420_to_vyuy, yuv420_to_vyuy422, into_yuv_planar_image, into_yuv_packed_image_mut);

impl_yuv_to_rgb!(i422_to_bgra32, yuv422_to_bgra, into_yuv_planar_image);
impl_yuv_to_rgb!(i422_to_rgba32, yuv422_to_rgba, into_yuv_planar_image);
impl_yuv_to_rgb!(i422_to_bgr24, yuv422_to_bgr, into_yuv_planar_image);
impl_yuv_to_rgb!(i422_to_rgb24, yuv422_to_rgb, into_yuv_planar_image);

impl_yuv_to_yuv!(i422_to_yuyv, yuv422_to_yuyv422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i422_to_yvyu, yuv422_to_yvyu422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i422_to_uyvy, yuv422_to_uyvy422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i422_to_vyuy, yuv422_to_vyuy422, into_yuv_planar_image, into_yuv_packed_image_mut);

impl_yuv_to_rgb!(i444_to_bgra32, yuv444_to_bgra, into_yuv_planar_image);
impl_yuv_to_rgb!(i444_to_rgba32, yuv444_to_rgba, into_yuv_planar_image);
impl_yuv_to_rgb!(i444_to_bgr24, yuv444_to_bgr, into_yuv_planar_image);
impl_yuv_to_rgb!(i444_to_rgb24, yuv444_to_rgb, into_yuv_planar_image);

impl_yuv_to_yuv!(i444_to_yuyv, yuv444_to_yuyv422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i444_to_yvyu, yuv444_to_yvyu422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i444_to_uyvy, yuv444_to_uyvy422, into_yuv_planar_image, into_yuv_packed_image_mut);
impl_yuv_to_yuv!(i444_to_vyuy, yuv444_to_vyuy422, into_yuv_planar_image, into_yuv_packed_image_mut);

impl_yuv_to_rgb_with_conversion_mode!(nv12_to_bgra32, yuv_nv12_to_bgra, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv12_to_rgba32, yuv_nv12_to_rgba, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv12_to_bgr24, yuv_nv12_to_bgr, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv12_to_rgb24, yuv_nv12_to_rgb, into_yuv_bi_planar_image, Fast);

impl_yuv_to_rgb_with_conversion_mode!(nv16_to_bgra32, yuv_nv16_to_bgra, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv16_to_rgba32, yuv_nv16_to_rgba, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv16_to_bgr24, yuv_nv16_to_bgr, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv16_to_rgb24, yuv_nv16_to_rgb, into_yuv_bi_planar_image, Fast);

impl_yuv_to_rgb_with_conversion_mode!(nv24_to_bgra32, yuv_nv24_to_bgra, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv24_to_rgba32, yuv_nv24_to_rgba, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv24_to_bgr24, yuv_nv24_to_bgr, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv24_to_rgb24, yuv_nv24_to_rgb, into_yuv_bi_planar_image, Fast);

impl_yuv_to_rgb_with_conversion_mode!(nv21_to_bgra32, yuv_nv21_to_bgra, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv21_to_rgba32, yuv_nv21_to_rgba, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv21_to_bgr24, yuv_nv21_to_bgr, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv21_to_rgb24, yuv_nv21_to_rgb, into_yuv_bi_planar_image, Fast);

impl_yuv_to_rgb_with_conversion_mode!(nv61_to_bgra32, yuv_nv61_to_bgra, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv61_to_rgba32, yuv_nv61_to_rgba, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv61_to_bgr24, yuv_nv61_to_bgr, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv61_to_rgb24, yuv_nv61_to_rgb, into_yuv_bi_planar_image, Fast);

impl_yuv_to_rgb_with_conversion_mode!(nv42_to_bgra32, yuv_nv42_to_bgra, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv42_to_rgba32, yuv_nv42_to_rgba, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv42_to_bgr24, yuv_nv42_to_bgr, into_yuv_bi_planar_image, Fast);
impl_yuv_to_rgb_with_conversion_mode!(nv42_to_rgb24, yuv_nv42_to_rgb, into_yuv_bi_planar_image, Fast);

impl_yuv_to_rgb!(yuyv_to_bgra32, yuyv422_to_bgra, into_yuv_packed_image);
impl_yuv_to_rgb!(yuyv_to_rgba32, yuyv422_to_rgba, into_yuv_packed_image);
impl_yuv_to_rgb!(yuyv_to_bgr24, yuyv422_to_bgr, into_yuv_packed_image);
impl_yuv_to_rgb!(yuyv_to_rgb24, yuyv422_to_rgb, into_yuv_packed_image);

impl_yuv_to_yuv!(yuyv_to_i420, yuyv422_to_yuv420, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(yuyv_to_i422, yuyv422_to_yuv422, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(yuyv_to_i444, yuyv422_to_yuv444, into_yuv_packed_image, into_yuv_planar_image_mut);

impl_yuv_to_rgb!(yvyu_to_bgra32, yvyu422_to_bgra, into_yuv_packed_image);
impl_yuv_to_rgb!(yvyu_to_rgba32, yvyu422_to_rgba, into_yuv_packed_image);
impl_yuv_to_rgb!(yvyu_to_bgr24, yvyu422_to_bgr, into_yuv_packed_image);
impl_yuv_to_rgb!(yvyu_to_rgb24, yvyu422_to_rgb, into_yuv_packed_image);

impl_yuv_to_yuv!(yvyu_to_i420, yvyu422_to_yuv420, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(yvyu_to_i422, yvyu422_to_yuv422, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(yvyu_to_i444, yvyu422_to_yuv444, into_yuv_packed_image, into_yuv_planar_image_mut);

impl_yuv_to_rgb!(uyvy_to_bgra32, uyvy422_to_bgra, into_yuv_packed_image);
impl_yuv_to_rgb!(uyvy_to_rgba32, uyvy422_to_rgba, into_yuv_packed_image);
impl_yuv_to_rgb!(uyvy_to_bgr24, uyvy422_to_bgr, into_yuv_packed_image);
impl_yuv_to_rgb!(uyvy_to_rgb24, uyvy422_to_rgb, into_yuv_packed_image);

impl_yuv_to_yuv!(uyvy_to_i420, uyvy422_to_yuv420, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(uyvy_to_i422, uyvy422_to_yuv422, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(uyvy_to_i444, uyvy422_to_yuv444, into_yuv_packed_image, into_yuv_planar_image_mut);

impl_yuv_to_rgb!(vyuy_to_bgra32, vyuy422_to_bgra, into_yuv_packed_image);
impl_yuv_to_rgb!(vyuy_to_rgba32, vyuy422_to_rgba, into_yuv_packed_image);
impl_yuv_to_rgb!(vyuy_to_bgr24, vyuy422_to_bgr, into_yuv_packed_image);
impl_yuv_to_rgb!(vyuy_to_rgb24, vyuy422_to_rgb, into_yuv_packed_image);

impl_yuv_to_yuv!(vyuy_to_i420, vyuy422_to_yuv420, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(vyuy_to_i422, vyuy422_to_yuv422, into_yuv_packed_image, into_yuv_planar_image_mut);
impl_yuv_to_yuv!(vyuy_to_i444, vyuy422_to_yuv444, into_yuv_packed_image, into_yuv_planar_image_mut);

impl_yuv_to_rgb_with_byte_order!(i010_to_rgb30, i010_to_ra30, into_yuv_planar_image, Network);
impl_yuv_to_rgb_with_byte_order!(i210_to_rgb30, i210_to_ra30, into_yuv_planar_image, Network);
impl_yuv_to_rgb_with_byte_order!(i410_to_rgb30, i410_to_ra30, into_yuv_planar_image, Network);

impl_yuv_to_rgb_with_byte_order!(p010_to_rgb30, p010_to_ra30, into_yuv_bi_planar_image, Network);
impl_yuv_to_rgb_with_byte_order!(p210_to_rgb30, p210_to_ra30, into_yuv_bi_planar_image, Network);

type VideoFormatConvertFunc = fn(&MappedPlanes, &mut MappedPlanes, ColorRange, ColorMatrix, NonZeroU32, NonZeroU32) -> Result<()>;

const PIXEL_FORMAT_MAX: usize = PixelFormat::MAX as usize;

static VIDEO_FORMAT_CONVERT_FUNCS: LazyLock<[[Option<VideoFormatConvertFunc>; PIXEL_FORMAT_MAX]; PIXEL_FORMAT_MAX]> = LazyLock::new(|| {
    let mut funcs: [[Option<VideoFormatConvertFunc>; PIXEL_FORMAT_MAX]; PIXEL_FORMAT_MAX] = [[None; PIXEL_FORMAT_MAX]; PIXEL_FORMAT_MAX];
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::RGBA32 as usize] = Some(bgra32_to_rgba32);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::I420 as usize] = Some(bgra32_to_i420);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::I422 as usize] = Some(bgra32_to_i422);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::I444 as usize] = Some(bgra32_to_i444);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::NV12 as usize] = Some(bgra32_to_nv12);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::NV16 as usize] = Some(bgra32_to_nv16);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::NV24 as usize] = Some(bgra32_to_nv24);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::NV21 as usize] = Some(bgra32_to_nv21);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::NV61 as usize] = Some(bgra32_to_nv61);
    funcs[PixelFormat::BGRA32 as usize][PixelFormat::NV42 as usize] = Some(bgra32_to_nv42);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::BGRA32 as usize] = Some(rgba32_to_bgra32);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::I420 as usize] = Some(rgba32_to_i420);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::I422 as usize] = Some(rgba32_to_i422);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::I444 as usize] = Some(rgba32_to_i444);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::NV12 as usize] = Some(rgba32_to_nv12);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::NV16 as usize] = Some(rgba32_to_nv16);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::NV24 as usize] = Some(rgba32_to_nv24);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::NV21 as usize] = Some(rgba32_to_nv21);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::NV61 as usize] = Some(rgba32_to_nv61);
    funcs[PixelFormat::RGBA32 as usize][PixelFormat::NV42 as usize] = Some(rgba32_to_nv42);
    funcs[PixelFormat::I420 as usize][PixelFormat::BGRA32 as usize] = Some(i420_to_bgra32);
    funcs[PixelFormat::I420 as usize][PixelFormat::RGBA32 as usize] = Some(i420_to_rgba32);
    funcs[PixelFormat::I420 as usize][PixelFormat::BGR24 as usize] = Some(i420_to_bgr24);
    funcs[PixelFormat::I420 as usize][PixelFormat::RGB24 as usize] = Some(i420_to_rgb24);
    funcs[PixelFormat::I420 as usize][PixelFormat::YUYV as usize] = Some(i420_to_yuyv);
    funcs[PixelFormat::I420 as usize][PixelFormat::YVYU as usize] = Some(i420_to_yvyu);
    funcs[PixelFormat::I420 as usize][PixelFormat::UYVY as usize] = Some(i420_to_uyvy);
    funcs[PixelFormat::I420 as usize][PixelFormat::VYUY as usize] = Some(i420_to_vyuy);
    funcs[PixelFormat::I422 as usize][PixelFormat::BGRA32 as usize] = Some(i422_to_bgra32);
    funcs[PixelFormat::I422 as usize][PixelFormat::RGBA32 as usize] = Some(i422_to_rgba32);
    funcs[PixelFormat::I422 as usize][PixelFormat::BGR24 as usize] = Some(i422_to_bgr24);
    funcs[PixelFormat::I422 as usize][PixelFormat::RGB24 as usize] = Some(i422_to_rgb24);
    funcs[PixelFormat::I422 as usize][PixelFormat::YUYV as usize] = Some(i422_to_yuyv);
    funcs[PixelFormat::I422 as usize][PixelFormat::YVYU as usize] = Some(i422_to_yvyu);
    funcs[PixelFormat::I422 as usize][PixelFormat::UYVY as usize] = Some(i422_to_uyvy);
    funcs[PixelFormat::I422 as usize][PixelFormat::VYUY as usize] = Some(i422_to_vyuy);
    funcs[PixelFormat::I444 as usize][PixelFormat::BGRA32 as usize] = Some(i444_to_bgra32);
    funcs[PixelFormat::I444 as usize][PixelFormat::RGBA32 as usize] = Some(i444_to_rgba32);
    funcs[PixelFormat::I444 as usize][PixelFormat::BGR24 as usize] = Some(i444_to_bgr24);
    funcs[PixelFormat::I444 as usize][PixelFormat::RGB24 as usize] = Some(i444_to_rgb24);
    funcs[PixelFormat::I444 as usize][PixelFormat::YUYV as usize] = Some(i444_to_yuyv);
    funcs[PixelFormat::I444 as usize][PixelFormat::YVYU as usize] = Some(i444_to_yvyu);
    funcs[PixelFormat::I444 as usize][PixelFormat::UYVY as usize] = Some(i444_to_uyvy);
    funcs[PixelFormat::I444 as usize][PixelFormat::VYUY as usize] = Some(i444_to_vyuy);
    funcs[PixelFormat::NV12 as usize][PixelFormat::BGRA32 as usize] = Some(nv12_to_bgra32);
    funcs[PixelFormat::NV12 as usize][PixelFormat::RGBA32 as usize] = Some(nv12_to_rgba32);
    funcs[PixelFormat::NV12 as usize][PixelFormat::BGR24 as usize] = Some(nv12_to_bgr24);
    funcs[PixelFormat::NV12 as usize][PixelFormat::RGB24 as usize] = Some(nv12_to_rgb24);
    funcs[PixelFormat::NV16 as usize][PixelFormat::BGRA32 as usize] = Some(nv16_to_bgra32);
    funcs[PixelFormat::NV16 as usize][PixelFormat::RGBA32 as usize] = Some(nv16_to_rgba32);
    funcs[PixelFormat::NV16 as usize][PixelFormat::BGR24 as usize] = Some(nv16_to_bgr24);
    funcs[PixelFormat::NV16 as usize][PixelFormat::RGB24 as usize] = Some(nv16_to_rgb24);
    funcs[PixelFormat::NV24 as usize][PixelFormat::BGRA32 as usize] = Some(nv24_to_bgra32);
    funcs[PixelFormat::NV24 as usize][PixelFormat::RGBA32 as usize] = Some(nv24_to_rgba32);
    funcs[PixelFormat::NV24 as usize][PixelFormat::BGR24 as usize] = Some(nv24_to_bgr24);
    funcs[PixelFormat::NV24 as usize][PixelFormat::RGB24 as usize] = Some(nv24_to_rgb24);
    funcs[PixelFormat::NV21 as usize][PixelFormat::BGRA32 as usize] = Some(nv21_to_bgra32);
    funcs[PixelFormat::NV21 as usize][PixelFormat::RGBA32 as usize] = Some(nv21_to_rgba32);
    funcs[PixelFormat::NV21 as usize][PixelFormat::BGR24 as usize] = Some(nv21_to_bgr24);
    funcs[PixelFormat::NV21 as usize][PixelFormat::RGB24 as usize] = Some(nv21_to_rgb24);
    funcs[PixelFormat::NV61 as usize][PixelFormat::BGRA32 as usize] = Some(nv61_to_bgra32);
    funcs[PixelFormat::NV61 as usize][PixelFormat::RGBA32 as usize] = Some(nv61_to_rgba32);
    funcs[PixelFormat::NV61 as usize][PixelFormat::BGR24 as usize] = Some(nv61_to_bgr24);
    funcs[PixelFormat::NV61 as usize][PixelFormat::RGB24 as usize] = Some(nv61_to_rgb24);
    funcs[PixelFormat::NV42 as usize][PixelFormat::BGRA32 as usize] = Some(nv42_to_bgra32);
    funcs[PixelFormat::NV42 as usize][PixelFormat::RGBA32 as usize] = Some(nv42_to_rgba32);
    funcs[PixelFormat::NV42 as usize][PixelFormat::BGR24 as usize] = Some(nv42_to_bgr24);
    funcs[PixelFormat::NV42 as usize][PixelFormat::RGB24 as usize] = Some(nv42_to_rgb24);
    funcs[PixelFormat::YUYV as usize][PixelFormat::BGRA32 as usize] = Some(yuyv_to_bgra32);
    funcs[PixelFormat::YUYV as usize][PixelFormat::RGBA32 as usize] = Some(yuyv_to_rgba32);
    funcs[PixelFormat::YUYV as usize][PixelFormat::BGR24 as usize] = Some(yuyv_to_bgr24);
    funcs[PixelFormat::YUYV as usize][PixelFormat::RGB24 as usize] = Some(yuyv_to_rgb24);
    funcs[PixelFormat::YUYV as usize][PixelFormat::I420 as usize] = Some(yuyv_to_i420);
    funcs[PixelFormat::YUYV as usize][PixelFormat::I422 as usize] = Some(yuyv_to_i422);
    funcs[PixelFormat::YUYV as usize][PixelFormat::I444 as usize] = Some(yuyv_to_i444);
    funcs[PixelFormat::YVYU as usize][PixelFormat::BGRA32 as usize] = Some(yvyu_to_bgra32);
    funcs[PixelFormat::YVYU as usize][PixelFormat::RGBA32 as usize] = Some(yvyu_to_rgba32);
    funcs[PixelFormat::YVYU as usize][PixelFormat::BGR24 as usize] = Some(yvyu_to_bgr24);
    funcs[PixelFormat::YVYU as usize][PixelFormat::RGB24 as usize] = Some(yvyu_to_rgb24);
    funcs[PixelFormat::YVYU as usize][PixelFormat::I420 as usize] = Some(yvyu_to_i420);
    funcs[PixelFormat::YVYU as usize][PixelFormat::I422 as usize] = Some(yvyu_to_i422);
    funcs[PixelFormat::YVYU as usize][PixelFormat::I444 as usize] = Some(yvyu_to_i444);
    funcs[PixelFormat::UYVY as usize][PixelFormat::BGRA32 as usize] = Some(uyvy_to_bgra32);
    funcs[PixelFormat::UYVY as usize][PixelFormat::RGBA32 as usize] = Some(uyvy_to_rgba32);
    funcs[PixelFormat::UYVY as usize][PixelFormat::BGR24 as usize] = Some(uyvy_to_bgr24);
    funcs[PixelFormat::UYVY as usize][PixelFormat::RGB24 as usize] = Some(uyvy_to_rgb24);
    funcs[PixelFormat::UYVY as usize][PixelFormat::I420 as usize] = Some(uyvy_to_i420);
    funcs[PixelFormat::UYVY as usize][PixelFormat::I422 as usize] = Some(uyvy_to_i422);
    funcs[PixelFormat::UYVY as usize][PixelFormat::I444 as usize] = Some(uyvy_to_i444);
    funcs[PixelFormat::VYUY as usize][PixelFormat::BGRA32 as usize] = Some(vyuy_to_bgra32);
    funcs[PixelFormat::VYUY as usize][PixelFormat::RGBA32 as usize] = Some(vyuy_to_rgba32);
    funcs[PixelFormat::VYUY as usize][PixelFormat::BGR24 as usize] = Some(vyuy_to_bgr24);
    funcs[PixelFormat::VYUY as usize][PixelFormat::RGB24 as usize] = Some(vyuy_to_rgb24);
    funcs[PixelFormat::VYUY as usize][PixelFormat::I420 as usize] = Some(vyuy_to_i420);
    funcs[PixelFormat::VYUY as usize][PixelFormat::I422 as usize] = Some(vyuy_to_i422);
    funcs[PixelFormat::VYUY as usize][PixelFormat::I444 as usize] = Some(vyuy_to_i444);
    funcs[PixelFormat::I010 as usize][PixelFormat::RGB30 as usize] = Some(i010_to_rgb30);
    funcs[PixelFormat::I210 as usize][PixelFormat::RGB30 as usize] = Some(i210_to_rgb30);
    funcs[PixelFormat::I410 as usize][PixelFormat::RGB30 as usize] = Some(i410_to_rgb30);
    funcs[PixelFormat::P010 as usize][PixelFormat::RGB30 as usize] = Some(p010_to_rgb30);
    funcs[PixelFormat::P210 as usize][PixelFormat::RGB30 as usize] = Some(p210_to_rgb30);
    funcs
});

fn data_copy(src: &MappedPlanes, dst: &mut MappedPlanes, format: PixelFormat, width: NonZeroU32, height: NonZeroU32) -> Result<()> {
    if src.planes.len() != dst.planes.len() {
        return Err(Error::Invalid("planes size mismatch".to_string()));
    }

    for (plane_index, (src_plane, dst_plane)) in src.planes.iter().zip(&mut dst.planes).enumerate() {
        let plane_row_bytes = format.calc_plane_row_bytes(plane_index, width.get()) as usize;
        let plane_height = format.calc_plane_height(plane_index, height.get());
        if let (Some(src_stride), Some(dst_stride)) = (src_plane.stride(), dst_plane.stride()) {
            if let (Some(src_data), Some(dst_data)) = (src_plane.data(), dst_plane.data_mut()) {
                for row in 0..plane_height {
                    let src_start = (row * src_stride) as usize;
                    let dst_start = (row * dst_stride) as usize;
                    dst_data[dst_start..dst_start + plane_row_bytes].copy_from_slice(&src_data[src_start..src_start + plane_row_bytes]);
                }
            }
        }
    }

    Ok(())
}

impl Frame<'_> {
    pub fn convert_to(&self, dst: &mut Frame) -> Result<()> {
        if self.media_type() != dst.media_type() || !self.is_video() {
            return Err(Error::Unsupported("media type".to_string()));
        }

        let dst_desc = dst.desc.clone();

        let guard = self.map().map_err(|_| Error::Invalid("not readable".into()))?;
        let mut dst_guard = dst.map_mut().map_err(|_| Error::Invalid("not writable".into()))?;

        if let (FrameDescriptor::Video(src_desc), FrameDescriptor::Video(dst_desc)) = (&self.desc, &dst_desc) {
            if src_desc.width == dst_desc.width && src_desc.height == dst_desc.height {
                let src_planes = guard.planes().unwrap();
                let mut dst_planes = dst_guard.planes_mut().unwrap();
                if src_desc.format == dst_desc.format {
                    return data_copy(&src_planes, &mut dst_planes, src_desc.format, src_desc.width, src_desc.height);
                } else {
                    let convert_func = VIDEO_FORMAT_CONVERT_FUNCS[src_desc.format as usize][dst_desc.format as usize];

                    if let Some(convert) = convert_func {
                        return convert(&src_planes, &mut dst_planes, src_desc.color_range, src_desc.color_matrix, src_desc.width, src_desc.height);
                    } else {
                        return Err(Error::Unsupported("video format conversion".to_string()));
                    }
                }
            } else {
                return Err(Error::Invalid("video frame size mismatch".to_string()));
            }
        }

        Ok(())
    }
}
