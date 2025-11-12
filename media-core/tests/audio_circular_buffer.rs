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

    let mut input_frame = Frame::audio_creator().create(SampleFormat::F32P, 2, 960, 48000).unwrap();

    if let Ok(mut guard) = input_frame.map_mut() {
        if let Some(planes) = guard.planes_mut() {
            for mut plane in planes {
                let data = plane.as_mut_slice_of::<f32>().unwrap();
                data.fill(0.5);
            }
        }
    }

    audio_buf.write(&input_frame).unwrap();
    assert_eq!(audio_buf.len(), 960);
    assert_eq!(audio_buf.available(), 0);

    let mut output_frame = Frame::audio_creator().create(SampleFormat::F32P, 2, 240, 48000).unwrap();
    for i in 0..4 {
        assert_eq!(audio_buf.read(&mut output_frame).unwrap(), 240);
        assert_eq!(audio_buf.len(), 960 - (i + 1) * 240);
        assert_eq!(audio_buf.available(), (i + 1) * 240);

        if let Ok(guard) = output_frame.map() {
            if let Some(planes) = guard.planes() {
                for plane in planes {
                    let data = plane.as_slice_of::<f32>().unwrap();
                    for &sample in data {
                        assert_eq!(sample, 0.5);
                    }
                }
            }
        }
    }

    assert_eq!(audio_buf.len(), 0);
    assert_eq!(audio_buf.available(), 960);
}
