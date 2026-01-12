use std::{
    mem,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Weak,
    },
};

use crossbeam_queue::SegQueue;

const DEFAULT_BUFFER_CAPACITY: usize = 1024;

pub struct Buffer {
    data: Box<[u8]>,
    available: usize,
    pool: Weak<BufferPool>,
}

impl Buffer {
    fn new(data: Box<[u8]>, pool: &Arc<BufferPool>) -> Self {
        let available = data.len();

        Self {
            data,
            available,
            pool: Arc::downgrade(pool),
        }
    }

    fn new_with_available(data: Box<[u8]>, pool: &Arc<BufferPool>, available: usize) -> Self {
        let available = available.min(data.len());

        Self {
            data,
            available,
            pool: Arc::downgrade(pool),
        }
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    pub fn len(&self) -> usize {
        self.available
    }

    pub fn is_empty(&self) -> bool {
        self.available == 0
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.available]
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data[..self.available]
    }

    // Resize the buffer to the specified length, not exceeding its capacity
    pub fn resize(&mut self, len: usize) {
        self.available = len.min(self.capacity());
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            pool.recycle_buffer(Arc::new(Buffer::new(mem::take(&mut self.data), &pool)));
        }
    }
}

pub struct BufferPool {
    queue: SegQueue<Arc<Buffer>>,
    buffer_capacity: AtomicUsize,
}

impl BufferPool {
    pub fn new(buffer_capacity: usize) -> Arc<Self> {
        let buffer_capacity = if buffer_capacity == 0 {
            DEFAULT_BUFFER_CAPACITY
        } else {
            buffer_capacity
        };

        Arc::new(Self {
            queue: SegQueue::new(),
            buffer_capacity: AtomicUsize::new(buffer_capacity),
        })
    }

    pub fn available(&self) -> usize {
        self.queue.len()
    }

    pub fn get_buffer(self: &Arc<Self>) -> Arc<Buffer> {
        let buffer_capacity = self.buffer_capacity.load(Ordering::Acquire);
        if let Some(mut buffer) = self.queue.pop() {
            if buffer_capacity == buffer.capacity() {
                if let Some(buffer_mut) = Arc::get_mut(&mut buffer) {
                    buffer_mut.resize(buffer_capacity);
                    buffer_mut.data_mut().fill(0);
                    return buffer;
                }
            }
        }

        Arc::new(Buffer::new(vec![0u8; buffer_capacity].into_boxed_slice(), self))
    }

    pub fn get_buffer_with_length(self: &Arc<Self>, len: usize) -> Arc<Buffer> {
        let mut buffer_capacity = self.buffer_capacity.load(Ordering::Acquire);

        if len > buffer_capacity {
            self.set_buffer_capacity(len);
            buffer_capacity = len;
        }

        if let Some(mut buffer) = self.queue.pop() {
            if buffer_capacity == buffer.capacity() {
                if let Some(buffer_mut) = Arc::get_mut(&mut buffer) {
                    buffer_mut.resize(len);
                    buffer_mut.data_mut().fill(0);
                    return buffer;
                }
            }
        }

        Arc::new(Buffer::new_with_available(vec![0u8; buffer_capacity].into_boxed_slice(), self, len))
    }

    pub fn recycle_buffer(&self, buffer: Arc<Buffer>) {
        if buffer.capacity() == self.buffer_capacity.load(Ordering::Acquire) {
            self.queue.push(buffer);
        }
    }

    pub fn get_buffer_capacity(&self) -> usize {
        self.buffer_capacity.load(Ordering::Relaxed)
    }

    pub fn set_buffer_capacity(&self, buffer_capacity: usize) {
        self.buffer_capacity.store(buffer_capacity, Ordering::Release);
        while self.queue.pop().is_some() {}
    }
}
