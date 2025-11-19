use media_core::{frame::Frame, video::*};

fn test_video_convert(src_fmt: PixelFormat, dst_fmt: PixelFormat, width: u32, height: u32) {
    let input_frame = Frame::video_creator().create(src_fmt, width, height).unwrap();
    let mut output_frame = Frame::video_creator().create(dst_fmt, width, height).unwrap();

    let result = input_frame.convert_video_to(&mut output_frame);
    assert!(result.is_ok(), "convert from {:?} to {:?} failed: {:?}", src_fmt, dst_fmt, result);
}

#[test]
fn test_yuv_to_rgb() {
    test_video_convert(PixelFormat::I420, PixelFormat::RGBA32, 640, 480);
    test_video_convert(PixelFormat::NV12, PixelFormat::RGBA32, 640, 480);
    test_video_convert(PixelFormat::YUYV, PixelFormat::RGBA32, 640, 480);
}

#[test]
fn test_rgb_to_yuv() {
    test_video_convert(PixelFormat::RGBA32, PixelFormat::I420, 640, 480);
    test_video_convert(PixelFormat::RGBA32, PixelFormat::NV12, 640, 480);
}

#[test]
fn test_yuv_to_yuv() {
    test_video_convert(PixelFormat::I420, PixelFormat::YUYV, 640, 480);
    test_video_convert(PixelFormat::YUYV, PixelFormat::I420, 640, 480);
}

#[test]
fn test_rgb_to_rgb() {
    test_video_convert(PixelFormat::RGBA32, PixelFormat::BGRA32, 640, 480);
    test_video_convert(PixelFormat::BGRA32, PixelFormat::RGBA32, 640, 480);
}

#[test]
fn test_same_format() {
    test_video_convert(PixelFormat::I420, PixelFormat::I420, 640, 480);
    test_video_convert(PixelFormat::NV12, PixelFormat::NV12, 640, 480);
    test_video_convert(PixelFormat::RGBA32, PixelFormat::RGBA32, 640, 480);
}
