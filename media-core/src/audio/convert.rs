use bytemuck::Pod;
use strum::EnumCount;

use super::audio::SampleFormat;
use crate::{
    error::Error,
    frame::{Frame, MappedPlanes},
    Result,
};

macro_rules! impl_convert {
    ($func_name:ident, $src_type:ty, $dst_type:ty, $convert_expr:expr) => {
        #[allow(clippy::too_many_arguments)]
        fn $func_name(
            src_planes: &MappedPlanes,
            dst_planes: &mut MappedPlanes,
            src_plane_index_step: usize,
            dst_plane_index_step: usize,
            src_data_step: usize,
            dst_data_step: usize,
            channels: u8,
            samples: u32,
        ) -> Result<()> {
            convert_samples::<$src_type, $dst_type>(
                src_planes,
                dst_planes,
                src_plane_index_step,
                dst_plane_index_step,
                src_data_step,
                dst_data_step,
                channels,
                samples,
                |src_val| $convert_expr(src_val),
            )
        }
    };
}

impl_convert!(u8_to_u8, u8, u8, |x: u8| x);
impl_convert!(u8_to_s16, u8, i16, |x: u8| (x as i16 - 0x80i16) << 8);
impl_convert!(u8_to_s32, u8, i32, |x: u8| (x as i32 - 0x80i32) << 24);
impl_convert!(u8_to_s64, u8, i64, |x: u8| (x as i64 - 0x80i64) << 56);
impl_convert!(u8_to_f32, u8, f32, |x: u8| (x as i32 - 0x80i32) as f32 * (1.0f32 / (1 << 7) as f32));
impl_convert!(u8_to_f64, u8, f64, |x: u8| (x as i32 - 0x80i32) as f64 * (1.0f64 / (1 << 7) as f64));
impl_convert!(s16_to_u8, i16, u8, |x: i16| ((x >> 8) + 0x80) as u8);
impl_convert!(s16_to_s16, i16, i16, |x: i16| x);
impl_convert!(s16_to_s32, i16, i32, |x: i16| (x as i32) << 16);
impl_convert!(s16_to_s64, i16, i64, |x: i16| (x as i64) << 48);
impl_convert!(s16_to_f32, i16, f32, |x: i16| (x as f32) * (1.0f32 / (1 << 15) as f32));
impl_convert!(s16_to_f64, i16, f64, |x: i16| (x as f64) * (1.0f64 / (1 << 15) as f64));
impl_convert!(s32_to_u8, i32, u8, |x: i32| ((x >> 24) + 0x80) as u8);
impl_convert!(s32_to_s16, i32, i16, |x: i32| (x >> 16) as i16);
impl_convert!(s32_to_s32, i32, i32, |x: i32| x);
impl_convert!(s32_to_s64, i32, i64, |x: i32| (x as i64) << 32);
impl_convert!(s32_to_f32, i32, f32, |x: i32| (x as f32) * (1.0f32 / (1 << 31) as f32));
impl_convert!(s32_to_f64, i32, f64, |x: i32| (x as f64) * (1.0f64 / (1 << 31) as f64));
impl_convert!(s64_to_u8, i64, u8, |x: i64| ((x >> 56) + 0x80) as u8);
impl_convert!(s64_to_s16, i64, i16, |x: i64| (x >> 48) as i16);
impl_convert!(s64_to_s32, i64, i32, |x: i64| (x >> 32) as i32);
impl_convert!(s64_to_s64, i64, i64, |x: i64| x);
impl_convert!(s64_to_f32, i64, f32, |x: i64| (x as f32) * (1.0f32 / (1u64 << 63) as f32));
impl_convert!(s64_to_f64, i64, f64, |x: i64| (x as f64) * (1.0f64 / (1u64 << 63) as f64));
impl_convert!(f32_to_u8, f32, u8, |x: f32| ((x * (1 << 7) as f32).round() as i32 + 0x80).clamp(0, 255) as u8);
impl_convert!(f32_to_s16, f32, i16, |x: f32| ((x * (1 << 15) as f32).round() as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16);
impl_convert!(f32_to_s32, f32, i32, |x: f32| ((x * (1 << 31) as f32).round() as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
impl_convert!(f32_to_s64, f32, i64, |x: f32| (x * (1u64 << 63) as f32).round() as i64);
impl_convert!(f32_to_f32, f32, f32, |x: f32| x);
impl_convert!(f32_to_f64, f32, f64, |x: f32| x as f64);
impl_convert!(f64_to_u8, f64, u8, |x: f64| ((x * (1 << 7) as f64).round() as i32 + 0x80).clamp(0, 255) as u8);
impl_convert!(f64_to_s16, f64, i16, |x: f64| ((x * (1 << 15) as f64).round() as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16);
impl_convert!(f64_to_s32, f64, i32, |x: f64| ((x * (1 << 31) as f64).round() as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
impl_convert!(f64_to_s64, f64, i64, |x: f64| (x * (1u64 << 63) as f64).round() as i64);
impl_convert!(f64_to_f32, f64, f32, |x: f64| x as f32);
impl_convert!(f64_to_f64, f64, f64, |x: f64| x);

type SampleFormatConvertFunc = fn(&MappedPlanes, &mut MappedPlanes, usize, usize, usize, usize, u8, u32) -> Result<()>;

const AUDIO_SAMPLE_FORMAT_MAX: usize = SampleFormat::COUNT / 2; // Only handle packed formats

static AUDIO_SAMPLE_CONVERT_FUNCS: [[SampleFormatConvertFunc; AUDIO_SAMPLE_FORMAT_MAX]; AUDIO_SAMPLE_FORMAT_MAX] = [
    [u8_to_u8, u8_to_s16, u8_to_s32, u8_to_s64, u8_to_f32, u8_to_f64],
    [s16_to_u8, s16_to_s16, s16_to_s32, s16_to_s64, s16_to_f32, s16_to_f64],
    [s32_to_u8, s32_to_s16, s32_to_s32, s32_to_s64, s32_to_f32, s32_to_f64],
    [s64_to_u8, s64_to_s16, s64_to_s32, s64_to_s64, s64_to_f32, s64_to_f64],
    [f32_to_u8, f32_to_s16, f32_to_s32, f32_to_s64, f32_to_f32, f32_to_f64],
    [f64_to_u8, f64_to_s16, f64_to_s32, f64_to_s64, f64_to_f32, f64_to_f64],
];

#[allow(clippy::too_many_arguments)]
fn convert_samples<S: Pod, D: Pod>(
    src_planes: &MappedPlanes,
    dst_planes: &mut MappedPlanes,
    src_plane_index_step: usize,
    dst_plane_index_step: usize,
    src_data_step: usize,
    dst_data_step: usize,
    channels: u8,
    samples: u32,
    convert: impl Fn(S) -> D,
) -> Result<()> {
    for ch in 0..channels as usize {
        let src_i = ch * src_plane_index_step;
        let dst_i = ch * dst_plane_index_step;
        let src_data = src_planes.plane_data(src_i).ok_or_else(|| Error::Invalid("out of range: src".to_string()))?;
        let dst_data = dst_planes.plane_data_mut(dst_i).ok_or_else(|| Error::Invalid("out of range: dst".to_string()))?;

        let src_data: &[S] = bytemuck::cast_slice(src_data);
        let dst_data: &mut [D] = bytemuck::cast_slice_mut(dst_data);

        for i in 0..samples as usize {
            dst_data[i * dst_data_step] = convert(src_data[i * src_data_step]);
        }
    }

    Ok(())
}

fn data_copy(src_planes: &MappedPlanes, dst_planes: &mut MappedPlanes) -> Result<()> {
    for (src_plane, dst_plane) in src_planes.iter().zip(dst_planes.iter_mut()) {
        if let (Some(src), Some(dst)) = (src_plane.data(), dst_plane.data_mut()) {
            if src.len() != dst.len() {
                return Err(Error::Invalid("plane size mismatch".to_string()));
            }
            dst.copy_from_slice(src);
        }
    }

    Ok(())
}

fn data_convert(
    src_planes: &MappedPlanes,
    dst_planes: &mut MappedPlanes,
    src_format: SampleFormat,
    dst_format: SampleFormat,
    channels: u8,
    samples: u32,
) -> Result<()> {
    // Get conversion function from table
    let convert = AUDIO_SAMPLE_CONVERT_FUNCS[src_format.packed_sample_format() as usize][dst_format.packed_sample_format() as usize];

    let (src_plane_index_step, src_data_step) = if src_format.is_planar() {
        (1, 1)
    } else {
        (0, channels as usize)
    };

    let (dst_plane_index_step, dst_data_step) = if dst_format.is_planar() {
        (1, 1)
    } else {
        (0, channels as usize)
    };

    convert(src_planes, dst_planes, src_plane_index_step, src_data_step, dst_plane_index_step, dst_data_step, channels, samples)
}

impl Frame<'_> {
    pub fn convert_audio_to(&self, dst: &mut Frame) -> Result<()> {
        if self.media_type() != dst.media_type() || !self.is_audio() {
            return Err(Error::Unsupported("media type mismatch".to_string()));
        }

        let src_desc = self.audio_descriptor().ok_or_else(|| Error::Invalid("not audio frame".to_string()))?;
        let dst_desc = dst.audio_descriptor().cloned().ok_or_else(|| Error::Invalid("not audio frame".to_string()))?;

        if src_desc.samples != dst_desc.samples {
            return Err(Error::Unsupported("samples mismatch".to_string()));
        }

        let src_channels = src_desc.channels().get();
        let dst_channels = dst_desc.channels().get();

        if src_channels != dst_channels {
            return Err(Error::Unsupported("channels mismatch".to_string()));
        }

        let guard = self.map().map_err(|_| Error::Invalid("cannot read source frame".into()))?;
        let mut dst_guard = dst.map_mut().map_err(|_| Error::Invalid("cannot write destination frame".into()))?;
        let src_planes = guard.planes().unwrap();
        let mut dst_planes = dst_guard.planes_mut().unwrap();

        let (src_format, dst_format) = if src_channels == 1 {
            (src_desc.format.planar_sample_format(), dst_desc.format.planar_sample_format())
        } else {
            (src_desc.format, dst_desc.format)
        };

        if src_format == dst_format {
            data_copy(&src_planes, &mut dst_planes)
        } else {
            data_convert(&src_planes, &mut dst_planes, src_format, dst_format, src_channels, src_desc.samples.get())
        }
    }
}
