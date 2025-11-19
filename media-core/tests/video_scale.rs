use media_core::{frame::Frame, video::*};

fn test_scale(fmt: PixelFormat, src_width: u32, src_height: u32, dst_width: u32, dst_height: u32, filter: ScaleFilter) {
    let input_frame = Frame::video_creator().create(fmt, src_width, src_height).unwrap();
    let mut output_frame = Frame::video_creator().create(fmt, dst_width, dst_height).unwrap();

    let result = input_frame.scale_to(&mut output_frame, filter);
    assert!(result.is_ok(), "scaling from {}x{} to {}x{} failed: {:?}", src_width, src_height, dst_width, dst_height, result);
}

#[test]
fn test_scale_yuv() {
    test_scale(PixelFormat::I420, 640, 480, 320, 240, ScaleFilter::Nearest);
    test_scale(PixelFormat::I420, 640, 480, 320, 240, ScaleFilter::Bilinear);
    test_scale(PixelFormat::I420, 640, 480, 320, 240, ScaleFilter::Bicubic);
    test_scale(PixelFormat::NV12, 640, 480, 320, 240, ScaleFilter::Nearest);
    test_scale(PixelFormat::NV12, 640, 480, 320, 240, ScaleFilter::Bilinear);
    test_scale(PixelFormat::NV12, 640, 480, 320, 240, ScaleFilter::Bicubic);
}

#[test]
fn test_scale_rgb() {
    test_scale(PixelFormat::RGBA32, 640, 480, 320, 240, ScaleFilter::Nearest);
    test_scale(PixelFormat::RGBA32, 640, 480, 320, 240, ScaleFilter::Bilinear);
    test_scale(PixelFormat::RGBA32, 640, 480, 320, 240, ScaleFilter::Bicubic);
    test_scale(PixelFormat::RGB24, 640, 480, 320, 240, ScaleFilter::Nearest);
    test_scale(PixelFormat::RGB24, 640, 480, 320, 240, ScaleFilter::Bilinear);
    test_scale(PixelFormat::RGB24, 640, 480, 320, 240, ScaleFilter::Bicubic);
}
