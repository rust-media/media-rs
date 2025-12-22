use crate::{Error, Result};

pub struct CircularBuffer<T = u8> {
    buffer: Vec<T>,
    read_pos: usize,
    write_pos: usize,
    len: usize,
}

impl<T: Default + Copy> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![T::default(); capacity],
            read_pos: 0,
            write_pos: 0,
            len: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn available(&self) -> usize {
        self.capacity() - self.len
    }

    pub fn grow(&mut self, capacity: usize) -> Result<()> {
        if self.capacity() >= capacity {
            return Ok(());
        }

        let new_capacity = capacity.max(self.capacity() * 2);

        if self.read_pos + self.len <= self.capacity() {
            if self.write_pos == 0 && self.len > 0 {
                let pos = self.capacity();
                self.buffer.resize(new_capacity, T::default());
                self.write_pos = pos;
            } else {
                self.buffer.resize(new_capacity, T::default());
            }
        } else {
            let mut new_buffer = vec![T::default(); new_capacity];
            let len = self.len;

            if len > 0 {
                self.read_to_slice(&mut new_buffer[..len]);
            }

            self.buffer = new_buffer;
            self.read_pos = 0;
            self.write_pos = len;
            self.len = len;
        }

        Ok(())
    }

    pub fn write(&mut self, buf: &[T]) -> Result<usize> {
        if buf.is_empty() {
            return Err(Error::WriteFailed("input buffer is empty".into()));
        }

        if self.available() < buf.len() {
            self.grow(self.len + buf.len())?;
        }

        let write_len = buf.len().min(self.available());
        let end_pos = self.write_pos + write_len;

        if end_pos <= self.capacity() {
            self.buffer[self.write_pos..end_pos].copy_from_slice(&buf[..write_len]);
            self.write_pos = end_pos % self.capacity();
        } else {
            let chunk_len = self.capacity() - self.write_pos;
            self.buffer[self.write_pos..].copy_from_slice(&buf[..chunk_len]);
            self.buffer[..write_len - chunk_len].copy_from_slice(&buf[chunk_len..write_len]);
            self.write_pos = (write_len - chunk_len) % self.capacity();
        }

        self.len += write_len;

        Ok(write_len)
    }

    pub fn read(&mut self, buf: &mut [T]) -> Result<usize> {
        if buf.is_empty() {
            return Err(Error::ReadFailed("output buffer is empty".into()));
        }

        let read_len = buf.len().min(self.len);
        if read_len == 0 {
            return Ok(0);
        }

        self.read_to_slice(&mut buf[..read_len]);
        Ok(read_len)
    }

    fn read_to_slice(&mut self, buf: &mut [T]) {
        let read_len = buf.len();
        let end_pos = self.read_pos + read_len;

        if end_pos <= self.capacity() {
            buf.copy_from_slice(&self.buffer[self.read_pos..end_pos]);
            self.read_pos = end_pos % self.capacity();
        } else {
            let chunk_len = self.capacity() - self.read_pos;
            buf[..chunk_len].copy_from_slice(&self.buffer[self.read_pos..]);
            buf[chunk_len..].copy_from_slice(&self.buffer[..read_len - chunk_len]);
            self.read_pos = (read_len - chunk_len) % self.capacity();
        }

        self.len -= read_len;
    }

    pub fn peek(&self, buf: &mut [T]) -> Result<usize> {
        if buf.is_empty() {
            return Err(Error::ReadFailed("output buffer is empty".into()));
        }

        let peek_len = buf.len().min(self.len);
        if peek_len == 0 {
            return Ok(0);
        }

        let end_pos = self.read_pos + peek_len;

        if end_pos <= self.capacity() {
            buf.copy_from_slice(&self.buffer[self.read_pos..end_pos]);
        } else {
            let chunk_len = self.capacity() - self.read_pos;
            buf[..chunk_len].copy_from_slice(&self.buffer[self.read_pos..]);
            buf[chunk_len..].copy_from_slice(&self.buffer[..peek_len - chunk_len]);
        }

        Ok(peek_len)
    }

    pub fn consume(&mut self, len: usize) -> usize {
        let consume_len = len.min(self.len);
        self.read_pos = (self.read_pos + consume_len) % self.capacity();
        self.len -= consume_len;
        consume_len
    }

    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.len = 0;
    }
}
