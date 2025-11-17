use std::{fmt::Debug, u8};

use bytemuck::Pod;
use media_core::{audio::SampleFormat, frame::Frame};

fn test_conversion<I, O>(src_format: SampleFormat, dst_format: SampleFormat, channels: u8, input_value: I, output_value: O)
where
    I: Pod + Copy,
    O: Pod + Copy + Debug + PartialEq,
{
    let samples = 960;
    let sample_rate = 48000;

    let mut input_frame = Frame::audio_creator().create(src_format, channels, samples, sample_rate).unwrap();
    let mut output_frame = Frame::audio_creator().create(dst_format, channels, samples, sample_rate).unwrap();

    if let Ok(mut guard) = input_frame.map_mut() {
        if let Some(planes) = guard.planes_mut() {
            for mut plane in planes {
                let data = plane.as_mut_slice_of::<I>().unwrap();
                data.fill(input_value);
            }
        }
    }

    let result = input_frame.convert_audio_to(&mut output_frame);
    assert!(result.is_ok(), "convert failed: {:?}", result);

    let map_result = output_frame.map();
    if let Ok(guard) = map_result {
        if let Some(planes) = guard.planes() {
            for plane in planes {
                let data = plane.as_slice_of::<O>().unwrap();
                for &sample in data {
                    assert_eq!(sample, output_value);
                }
            }
        }
    }
}

#[test]
fn test_u8_to_s16() {
    test_conversion(SampleFormat::U8, SampleFormat::S16, 1, u8::MIN, i16::MIN);
    test_conversion(SampleFormat::U8P, SampleFormat::S16, 2, 0x80u8, 0i16);
}

#[test]
fn test_u8_to_s32() {
    test_conversion(SampleFormat::U8, SampleFormat::S32, 1, u8::MIN, i32::MIN);
    test_conversion(SampleFormat::U8P, SampleFormat::S32, 2, 0x80u8, 0i32);
}

#[test]
fn test_u8_to_s64() {
    test_conversion(SampleFormat::U8, SampleFormat::S64, 1, u8::MIN, i64::MIN);
    test_conversion(SampleFormat::U8P, SampleFormat::S64, 2, 0x80u8, 0i64);
}

#[test]
fn test_u8_to_f32() {
    test_conversion(SampleFormat::U8, SampleFormat::F32, 1, u8::MIN, -1.0f32);
    test_conversion(SampleFormat::U8, SampleFormat::F32P, 2, 0x80u8, 0.0f32);
}

#[test]
fn test_u8_to_f64() {
    test_conversion(SampleFormat::U8, SampleFormat::F64, 1, u8::MIN, -1.0f64);
    test_conversion(SampleFormat::U8, SampleFormat::F64P, 2, 0x80u8, 0.0f64);
}

#[test]
fn test_s16_to_u8() {
    test_conversion(SampleFormat::S16, SampleFormat::U8, 1, i16::MIN, u8::MIN);
    test_conversion(SampleFormat::S16P, SampleFormat::U8, 2, 0i16, 0x80u8);
    test_conversion(SampleFormat::S16, SampleFormat::U8P, 2, i16::MAX, u8::MAX);
}

#[test]
fn test_s16_to_s32() {
    test_conversion(SampleFormat::S16, SampleFormat::S32, 1, i16::MIN, i32::MIN);
    test_conversion(SampleFormat::S16P, SampleFormat::S32, 2, 0i16, 0i32);
}

#[test]
fn test_s16_to_s64() {
    test_conversion(SampleFormat::S16, SampleFormat::S64, 1, i16::MIN, i64::MIN);
    test_conversion(SampleFormat::S16P, SampleFormat::S64, 2, 0i16, 0i64);
}

#[test]
fn test_s16_to_f32() {
    test_conversion(SampleFormat::S16, SampleFormat::F32, 1, i16::MIN, -1.0f32);
    test_conversion(SampleFormat::S16P, SampleFormat::F32, 2, 0i16, 0.0f32);
}

#[test]
fn test_s16_to_f64() {
    test_conversion(SampleFormat::S16, SampleFormat::F64, 1, i16::MIN, -1.0f64);
    test_conversion(SampleFormat::S16P, SampleFormat::F64, 2, 0i16, 0.0f64);
}

#[test]
fn test_s32_to_u8() {
    test_conversion(SampleFormat::S32, SampleFormat::U8, 1, i32::MIN, u8::MIN);
    test_conversion(SampleFormat::S32P, SampleFormat::U8, 2, 0i32, 0x80u8);
    test_conversion(SampleFormat::S32, SampleFormat::U8P, 2, i32::MAX, u8::MAX);
}

#[test]
fn test_s32_to_s16() {
    test_conversion(SampleFormat::S32, SampleFormat::S16, 1, i32::MIN, i16::MIN);
    test_conversion(SampleFormat::S32P, SampleFormat::S16, 2, 0i32, 0i16);
    test_conversion(SampleFormat::S32, SampleFormat::S16P, 2, i32::MAX, i16::MAX);
}

#[test]
fn test_s32_to_s64() {
    test_conversion(SampleFormat::S32, SampleFormat::S64, 1, i32::MIN, i64::MIN);
    test_conversion(SampleFormat::S32P, SampleFormat::S64, 2, 0i32, 0i64);
}

#[test]
fn test_s32_to_f32() {
    test_conversion(SampleFormat::S32, SampleFormat::F32, 1, i32::MIN, -1.0f32);
    test_conversion(SampleFormat::S32P, SampleFormat::F32, 2, 0i32, 0.0f32);
}

#[test]
fn test_s32_to_f64() {
    test_conversion(SampleFormat::S32, SampleFormat::F64, 1, i32::MIN, -1.0f64);
    test_conversion(SampleFormat::S32P, SampleFormat::F64, 2, 0i32, 0.0f64);
}

#[test]
fn test_s64_to_u8() {
    test_conversion(SampleFormat::S64, SampleFormat::U8, 1, i64::MIN, u8::MIN);
    test_conversion(SampleFormat::S64P, SampleFormat::U8, 2, 0i64, 0x80u8);
    test_conversion(SampleFormat::S64, SampleFormat::U8P, 2, i64::MAX, u8::MAX);
}

#[test]
fn test_s64_to_s16() {
    test_conversion(SampleFormat::S64, SampleFormat::S16, 1, i64::MIN, i16::MIN);
    test_conversion(SampleFormat::S64P, SampleFormat::S16, 2, 0i64, 0i16);
    test_conversion(SampleFormat::S64, SampleFormat::S16P, 2, i64::MAX, i16::MAX);
}

#[test]
fn test_s64_to_s32() {
    test_conversion(SampleFormat::S64, SampleFormat::S32, 1, i64::MIN, i32::MIN);
    test_conversion(SampleFormat::S64P, SampleFormat::S32, 2, 0i64, 0i32);
    test_conversion(SampleFormat::S64, SampleFormat::S32P, 2, i64::MAX, i32::MAX);
}

#[test]
fn test_s64_to_f32() {
    test_conversion(SampleFormat::S64, SampleFormat::F32, 1, i64::MIN, -1.0f32);
    test_conversion(SampleFormat::S64P, SampleFormat::F32, 2, 0i64, 0.0f32);
}

#[test]
fn test_s64_to_f64() {
    test_conversion(SampleFormat::S64, SampleFormat::F64, 1, i64::MIN, -1.0f64);
    test_conversion(SampleFormat::S64P, SampleFormat::F64, 2, 0i64, 0.0f64);
}

#[test]
fn test_f32_to_u8() {
    test_conversion(SampleFormat::F32, SampleFormat::U8, 1, -1.0f32, u8::MIN);
    test_conversion(SampleFormat::F32P, SampleFormat::U8, 2, 0.0f32, 0x80u8);
    test_conversion(SampleFormat::F32, SampleFormat::U8P, 2, 1.0f32, u8::MAX);
}

#[test]
fn test_f32_to_s16() {
    test_conversion(SampleFormat::F32, SampleFormat::S16, 1, -1.0f32, i16::MIN);
    test_conversion(SampleFormat::F32P, SampleFormat::S16, 2, 0.0f32, 0i16);
    test_conversion(SampleFormat::F32, SampleFormat::S16P, 2, 1.0f32, i16::MAX);
}

#[test]
fn test_f32_to_s32() {
    test_conversion(SampleFormat::F32, SampleFormat::S32, 1, -1.0f32, i32::MIN);
    test_conversion(SampleFormat::F32P, SampleFormat::S32, 2, 0.0f32, 0i32);
    test_conversion(SampleFormat::F32, SampleFormat::S32P, 2, 1.0f32, i32::MAX);
}

#[test]
fn test_f32_to_s64() {
    test_conversion(SampleFormat::F32, SampleFormat::S64, 1, -1.0f32, i64::MIN);
    test_conversion(SampleFormat::F32P, SampleFormat::S64, 2, 0.0f32, 0i64);
    test_conversion(SampleFormat::F32, SampleFormat::S64P, 2, 1.0f32, i64::MAX);
}

#[test]
fn test_f32_to_f64() {
    test_conversion(SampleFormat::F32, SampleFormat::F64, 1, -1.0f32, -1.0f64);
    test_conversion(SampleFormat::F32P, SampleFormat::F64, 2, 0.0f32, 0.0f64);
    test_conversion(SampleFormat::F32, SampleFormat::F64P, 2, 1.0f32, 1.0f64);
}

#[test]
fn test_f64_to_u8() {
    test_conversion(SampleFormat::F64, SampleFormat::U8, 1, -1.0f64, u8::MIN);
    test_conversion(SampleFormat::F64P, SampleFormat::U8, 2, 0.0f64, 0x80u8);
    test_conversion(SampleFormat::F64, SampleFormat::U8P, 2, 1.0f64, u8::MAX);
}

#[test]
fn test_f64_to_s16() {
    test_conversion(SampleFormat::F64, SampleFormat::S16, 1, -1.0f64, i16::MIN);
    test_conversion(SampleFormat::F64P, SampleFormat::S16, 2, 0.0f64, 0i16);
    test_conversion(SampleFormat::F64, SampleFormat::S16P, 2, 1.0f64, i16::MAX);
}

#[test]
fn test_f64_to_s32() {
    test_conversion(SampleFormat::F64, SampleFormat::S32, 1, -1.0f64, i32::MIN);
    test_conversion(SampleFormat::F64P, SampleFormat::S32, 2, 0.0f64, 0i32);
    test_conversion(SampleFormat::F64, SampleFormat::S32P, 2, 1.0f64, i32::MAX);
}

#[test]
fn test_f64_to_s64() {
    test_conversion(SampleFormat::F64, SampleFormat::S64, 1, -1.0f64, i64::MIN);
    test_conversion(SampleFormat::F64P, SampleFormat::S64, 2, 0.0f64, 0i64);
    test_conversion(SampleFormat::F64, SampleFormat::S64P, 2, 1.0f64, i64::MAX);
}

#[test]
fn test_f64_to_f32() {
    test_conversion(SampleFormat::F64, SampleFormat::F32, 1, -1.0f64, -1.0f32);
    test_conversion(SampleFormat::F64P, SampleFormat::F32, 2, 0.0f64, 0.0f32);
    test_conversion(SampleFormat::F64, SampleFormat::F32P, 2, 1.0f64, 1.0f32);
}
