use media_core::{frame::*, video::*};

#[test]
fn test_pixel_format() {
    assert_eq!(PixelFormat::I420.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::I422.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::I444.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::NV12.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::NV16.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::NV24.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::NV21.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::NV61.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::NV42.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::YUYV.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::YVYU.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::UYVY.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::VYUY.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::I010.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::I210.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::I410.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::I012.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::I212.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::I412.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::I016.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::I216.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::I416.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::P010.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::P210.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::P410.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::P012.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::P212.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::P412.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
    assert_eq!(PixelFormat::P016.chroma_subsampling(), Some(ChromaSubsampling::YUV420));
    assert_eq!(PixelFormat::P216.chroma_subsampling(), Some(ChromaSubsampling::YUV422));
    assert_eq!(PixelFormat::P416.chroma_subsampling(), Some(ChromaSubsampling::YUV444));
}

#[test]
fn test_video_frame() {
    let frame = Frame::video_creator().create(PixelFormat::I420, 640, 480);

    assert!(frame.is_ok());

    let frame = frame.unwrap();
    let desc = frame.video_descriptor().unwrap();
    assert_eq!(desc.format, PixelFormat::I420);
    assert_eq!(desc.width().get(), 640);
    assert_eq!(desc.height().get(), 480);
}

#[test]
fn test_video_frame_from_buffer() {
    let pixel_format = PixelFormat::ARGB32;
    let width = 640;
    let height = 480;
    let data_size = width * height * 4;
    let buffer = vec![0u8; data_size as usize];

    let frame = Frame::video_creator().create_from_packed_buffer(pixel_format, width, height, width * 4, buffer.as_slice());

    assert!(frame.is_ok());

    let frame = frame.unwrap();
    let desc = frame.video_descriptor().unwrap();
    assert_eq!(desc.format, PixelFormat::ARGB32);
    assert_eq!(desc.width().get(), 640);
    assert_eq!(desc.height().get(), 480);
}
