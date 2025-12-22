use std::{borrow::Cow, fmt::Debug};

use bytemuck::{self, Pod};
use pic_scale::{BufferStore, ImageStore, ImageStoreMut, LinearScaler, ResamplingFunction, Scaling, ScalingU16};

use super::{
    frame::VideoFrame,
    video::{PixelFormat, ScaleFilter, VideoFrameDescriptor},
};
use crate::{
    frame::{DataMappable, Frame, FrameData, MappedPlane},
    invalid_error, FrameDescriptor, Result,
};

impl From<ScaleFilter> for ResamplingFunction {
    fn from(filter: ScaleFilter) -> Self {
        match filter {
            ScaleFilter::Nearest => ResamplingFunction::Nearest,
            ScaleFilter::Bilinear => ResamplingFunction::Bilinear,
            ScaleFilter::Bicubic => ResamplingFunction::Bicubic,
        }
    }
}

fn into_image_store<'a, T, const N: usize>(src: &'a MappedPlane, width: u32, height: u32) -> Result<ImageStore<'a, T, N>>
where
    T: Debug + Pod,
{
    Ok(ImageStore::<T, N> {
        buffer: Cow::Borrowed(bytemuck::cast_slice(src.data().unwrap())),
        channels: N,
        width: width as usize,
        height: height as usize,
        stride: src.stride().unwrap() / size_of::<T>(),
        bit_depth: 0,
    })
}

fn into_image_store_mut<'a, T, const N: usize>(dst: &'a mut MappedPlane, width: u32, height: u32) -> Result<ImageStoreMut<'a, T, N>>
where
    T: Debug + Pod,
{
    let stride = dst.stride().unwrap() / size_of::<T>();

    Ok(ImageStoreMut::<T, N> {
        buffer: BufferStore::Borrowed(bytemuck::cast_slice_mut(dst.data_mut().unwrap())),
        channels: N,
        width: width as usize,
        height: height as usize,
        stride,
        bit_depth: 0,
    })
}

impl Frame<'_> {
    pub fn scale_to(&self, dst: &mut Frame<'_>, scale_filter: ScaleFilter) -> Result<()> {
        let (FrameDescriptor::Video(src_desc), FrameDescriptor::Video(dst_desc)) = (&self.desc, &dst.desc) else {
            return Err(invalid_error!("not video frame"));
        };

        VideoFrame::scale_to_internal(src_desc, &self.data, dst_desc, &mut dst.data, scale_filter)
    }
}

impl VideoFrame<'_> {
    fn scale_to_internal(
        src_desc: &VideoFrameDescriptor,
        src_data: &FrameData,
        dst_desc: &VideoFrameDescriptor,
        dst_data: &mut FrameData,
        scale_filter: ScaleFilter,
    ) -> Result<()> {
        if src_desc.format != dst_desc.format {
            return Err(invalid_error!("pixel format mismatch"));
        }

        let guard = src_data.map().map_err(|_| invalid_error!("not readable"))?;
        let mut dst_guard = dst_data.map_mut().map_err(|_| invalid_error!("not writable"))?;
        let src_planes = guard.planes().unwrap();
        let mut dst_planes = dst_guard.planes_mut().unwrap();

        let resampling_function: ResamplingFunction = scale_filter.into();
        let scaler = LinearScaler::new(resampling_function);

        let format = src_desc.format;
        match format {
            PixelFormat::ARGB32 | PixelFormat::BGRA32 | PixelFormat::ABGR32 | PixelFormat::RGBA32 => {
                let src = into_image_store::<u8, 4>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let mut dst = into_image_store_mut::<u8, 4>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_rgba(&src, &mut dst, true).map_err(|e| invalid_error!(e.to_string()))
            }
            PixelFormat::RGB24 | PixelFormat::BGR24 => {
                let src = into_image_store::<u8, 3>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let mut dst = into_image_store_mut::<u8, 3>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_rgb(&src, &mut dst).map_err(|e| invalid_error!(e.to_string()))
            }
            PixelFormat::I420 |
            PixelFormat::I422 |
            PixelFormat::I444 |
            PixelFormat::I440 |
            PixelFormat::YV12 |
            PixelFormat::YV16 |
            PixelFormat::YV24 => {
                let (src_chroma_width, src_chroma_height) = format.calc_chroma_dimensions(src_desc.width().get(), src_desc.height().get());
                let (dst_chroma_width, dst_chroma_height) = format.calc_chroma_dimensions(dst_desc.width().get(), dst_desc.height().get());
                let src_y = into_image_store::<u8, 1>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let src_u = into_image_store::<u8, 1>(&src_planes.planes[1], src_chroma_width, src_chroma_height)?;
                let src_v = into_image_store::<u8, 1>(&src_planes.planes[2], src_chroma_width, src_chroma_height)?;
                let mut dst_y = into_image_store_mut::<u8, 1>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_plane(&src_y, &mut dst_y).map_err(|e| invalid_error!(e.to_string()))?;
                let mut dst_u = into_image_store_mut::<u8, 1>(&mut dst_planes.planes[1], dst_chroma_width, dst_chroma_height)?;
                scaler.resize_plane(&src_u, &mut dst_u).map_err(|e| invalid_error!(e.to_string()))?;
                let mut dst_v = into_image_store_mut::<u8, 1>(&mut dst_planes.planes[2], dst_chroma_width, dst_chroma_height)?;
                scaler.resize_plane(&src_v, &mut dst_v).map_err(|e| invalid_error!(e.to_string()))?;
                Ok(())
            }
            PixelFormat::NV12 | PixelFormat::NV21 | PixelFormat::NV16 | PixelFormat::NV61 | PixelFormat::NV24 | PixelFormat::NV42 => {
                let (src_chroma_width, src_chroma_height) = format.calc_chroma_dimensions(src_desc.width().get(), src_desc.height().get());
                let (dst_chroma_width, dst_chroma_height) = format.calc_chroma_dimensions(dst_desc.width().get(), dst_desc.height().get());
                let src_y = into_image_store::<u8, 1>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let src_uv = into_image_store::<u8, 2>(&src_planes.planes[1], src_chroma_width, src_chroma_height)?;
                let mut dst_y = into_image_store_mut::<u8, 1>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_plane(&src_y, &mut dst_y).map_err(|e| invalid_error!(e.to_string()))?;
                let mut dst_uv = into_image_store_mut::<u8, 2>(&mut dst_planes.planes[1], dst_chroma_width, dst_chroma_height)?;
                scaler.resize_cbcr8(&src_uv, &mut dst_uv).map_err(|e| invalid_error!(e.to_string()))?;
                Ok(())
            }
            PixelFormat::ARGB64 | PixelFormat::BGRA64 | PixelFormat::ABGR64 | PixelFormat::RGBA64 => {
                let src = into_image_store::<u16, 4>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let mut dst = into_image_store_mut::<u16, 4>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_rgba_u16(&src, &mut dst, true).map_err(|e| invalid_error!(e.to_string()))
            }
            PixelFormat::I010 |
            PixelFormat::I210 |
            PixelFormat::I410 |
            PixelFormat::I44010 |
            PixelFormat::I012 |
            PixelFormat::I212 |
            PixelFormat::I412 |
            PixelFormat::I44012 |
            PixelFormat::I016 |
            PixelFormat::I216 |
            PixelFormat::I416 |
            PixelFormat::I44016 => {
                let (src_chroma_width, src_chroma_height) = format.calc_chroma_dimensions(src_desc.width().get(), src_desc.height().get());
                let (dst_chroma_width, dst_chroma_height) = format.calc_chroma_dimensions(dst_desc.width().get(), dst_desc.height().get());
                let src_y = into_image_store::<u16, 1>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let src_u = into_image_store::<u16, 1>(&src_planes.planes[1], src_chroma_width, src_chroma_height)?;
                let src_v = into_image_store::<u16, 1>(&src_planes.planes[2], src_chroma_width, src_chroma_height)?;
                let mut dst_y = into_image_store_mut::<u16, 1>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_plane_u16(&src_y, &mut dst_y).map_err(|e| invalid_error!(e.to_string()))?;
                let mut dst_u = into_image_store_mut::<u16, 1>(&mut dst_planes.planes[1], dst_chroma_width, dst_chroma_height)?;
                scaler.resize_plane_u16(&src_u, &mut dst_u).map_err(|e| invalid_error!(e.to_string()))?;
                let mut dst_v = into_image_store_mut::<u16, 1>(&mut dst_planes.planes[2], dst_chroma_width, dst_chroma_height)?;
                scaler.resize_plane_u16(&src_v, &mut dst_v).map_err(|e| invalid_error!(e.to_string()))?;
                Ok(())
            }
            PixelFormat::P010 |
            PixelFormat::P210 |
            PixelFormat::P410 |
            PixelFormat::P012 |
            PixelFormat::P212 |
            PixelFormat::P412 |
            PixelFormat::P016 |
            PixelFormat::P216 |
            PixelFormat::P416 => {
                let (src_chroma_width, src_chroma_height) = format.calc_chroma_dimensions(src_desc.width().get(), src_desc.height().get());
                let (dst_chroma_width, dst_chroma_height) = format.calc_chroma_dimensions(dst_desc.width().get(), dst_desc.height().get());
                let src_y = into_image_store::<u16, 1>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let src_uv = into_image_store::<u16, 2>(&src_planes.planes[1], src_chroma_width, src_chroma_height)?;
                let mut dst_y = into_image_store_mut::<u16, 1>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_plane_u16(&src_y, &mut dst_y).map_err(|e| invalid_error!(e.to_string()))?;
                let mut dst_uv = into_image_store_mut::<u16, 2>(&mut dst_planes.planes[1], dst_chroma_width, dst_chroma_height)?;
                scaler.resize_cbcr_u16(&src_uv, &mut dst_uv).map_err(|e| invalid_error!(e.to_string()))?;
                Ok(())
            }
            PixelFormat::Y8 => {
                let src = into_image_store::<u8, 1>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let mut dst = into_image_store_mut::<u8, 1>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                scaler.resize_plane(&src, &mut dst).map_err(|e| invalid_error!(e.to_string()))
            }
            PixelFormat::YA8 => {
                let src = into_image_store::<u8, 2>(&src_planes.planes[0], src_desc.width().get(), src_desc.height().get())?;
                let mut dst = into_image_store_mut::<u8, 2>(&mut dst_planes.planes[0], dst_desc.width().get(), dst_desc.height().get())?;
                // similar to UV component interleaving
                scaler.resize_cbcr8(&src, &mut dst).map_err(|e| invalid_error!(e.to_string()))
            }
            _ => Err(invalid_error!("unsupported pixel format".to_string())),
        }
    }

    pub fn scale_to(&self, dst: &mut VideoFrame<'_>, scale_filter: ScaleFilter) -> Result<()> {
        Self::scale_to_internal(&self.desc, &self.data, &dst.desc, &mut dst.data, scale_filter)
    }
}
