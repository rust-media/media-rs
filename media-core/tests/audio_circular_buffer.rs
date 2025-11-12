use media_core::{
    audio::{circular_buffer::AudioCircularBuffer, *},
    frame::Frame,
};

#[test]
fn test_read_write() {
    let mut audio_buf = AudioCircularBuffer::new(SampleFormat::F32P, 2, 960);

    assert_eq!(audio_buf.capacity(), 960);
    assert_eq!(audio_buf.len(), 0);
    assert_eq!(audio_buf.available(), 960);

    let input_frame = Frame::audio_creator().create(SampleFormat::F32P, 2, 960, 48000).unwrap();
    audio_buf.write(&input_frame).unwrap();
    assert_eq!(audio_buf.len(), 960);
    assert_eq!(audio_buf.available(), 0);

    let mut output_frame = Frame::audio_creator().create(SampleFormat::F32P, 2, 240, 48000).unwrap();
    for i in 0..4 {
        assert_eq!(audio_buf.read(&mut output_frame).unwrap(), 240);
        assert_eq!(audio_buf.len(), 960 - (i + 1) * 240);
        assert_eq!(audio_buf.available(), (i + 1) * 240);
    }

    assert_eq!(audio_buf.len(), 0);
    assert_eq!(audio_buf.available(), 960);
}
