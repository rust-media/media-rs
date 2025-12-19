use crate::{audio::SampleFormat, circular_buffer::CircularBuffer, frame::Frame, Error, FrameDescriptorSpec, Result};

pub struct AudioCircularBuffer {
    buffers: Vec<CircularBuffer>,
    format: SampleFormat,
    channels: u8,
    len: u32,
    capacity: u32,
}

impl AudioCircularBuffer {
    pub fn new(format: SampleFormat, channels: u8, samples: u32) -> Self {
        let num_buffers = if format.is_planar() {
            channels as usize
        } else {
            1
        };

        let buffer_size = format.calc_plane_size(channels, samples);

        let mut buffers = Vec::with_capacity(num_buffers);
        for _ in 0..num_buffers {
            buffers.push(CircularBuffer::new(buffer_size));
        }

        Self {
            buffers,
            format,
            channels,
            len: 0,
            capacity: samples,
        }
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline]
    pub fn available(&self) -> u32 {
        self.capacity - self.len
    }

    pub fn grow(&mut self, samples: u32) -> crate::Result<()> {
        if self.capacity >= samples {
            return Ok(());
        }
        let buffer_size = self.format.calc_plane_size(self.channels, samples);

        for buffer in &mut self.buffers {
            buffer.grow(buffer_size)?;
        }

        self.capacity = samples;

        Ok(())
    }

    fn validate_frame<D>(&self, frame: &Frame<D>) -> Result<u32>
    where
        D: FrameDescriptorSpec,
    {
        let desc = frame.descriptor().as_audio().ok_or_else(|| Error::Invalid("not audio frame".to_string()))?;

        if desc.format != self.format {
            return Err(Error::Invalid("sample format mismatch".to_string()));
        }

        if desc.channels().get() != self.channels {
            return Err(Error::Invalid("channel count mismatch".to_string()));
        }

        Ok(desc.samples.get())
    }

    pub fn write<D>(&mut self, frame: &Frame<D>) -> Result<usize>
    where
        D: FrameDescriptorSpec,
    {
        let samples = self.validate_frame(frame)?;

        if self.available() < samples {
            self.grow(self.len + samples)?;
        }

        let guard = frame.map().map_err(|_| Error::Invalid("cannot read source frame".into()))?;
        let planes = guard.planes().unwrap();

        if planes.len() != self.buffers.len() {
            return Err(Error::Invalid("plane count mismatch".to_string()));
        }

        planes.iter().enumerate().try_for_each(|(i, plane)| {
            let buffer = &mut self.buffers[i];
            buffer.write(plane.data().unwrap())?;
            Ok(())
        })?;

        self.len += samples;

        Ok(samples as usize)
    }

    pub fn read<D>(&mut self, frame: &mut Frame<D>) -> Result<usize>
    where
        D: FrameDescriptorSpec,
    {
        let samples = self.validate_frame(frame)?.min(self.len);

        let mut guard = frame.map_mut().map_err(|_| Error::Invalid("cannot write destination frame".into()))?;
        let mut planes = guard.planes_mut().unwrap();

        if planes.len() != self.buffers.len() {
            return Err(Error::Invalid("plane count mismatch".to_string()));
        }

        planes.iter_mut().enumerate().try_for_each(|(i, plane)| {
            let buffer = &mut self.buffers[i];
            buffer.read(plane.data_mut().unwrap())?;
            Ok(())
        })?;

        self.len -= samples;

        Ok(samples as usize)
    }

    pub fn clear(&mut self) {
        self.buffers.iter_mut().for_each(|buffer| buffer.clear());
        self.len = 0;
    }
}
