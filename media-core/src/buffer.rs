use std::{
    mem,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Weak,
    },
};

use crossbeam_queue::SegQueue;

pub struct Buffer {
    data: Box<[u8]>,
    pool: Weak<BufferPool>,
}

impl Buffer {
    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            pool.recycle_buffer(Arc::new(Buffer {
                data: mem::take(&mut self.data),
                pool: Arc::downgrade(&pool),
            }));
        }
    }
}

pub struct BufferPool {
    queue: SegQueue<Arc<Buffer>>,
    buffer_size: AtomicUsize,
}

impl BufferPool {
    pub fn new(buffer_size: usize) -> Arc<Self> {
        Arc::new(Self {
            queue: SegQueue::new(),
            buffer_size: AtomicUsize::new(buffer_size),
        })
    }

    pub fn available(&self) -> usize {
        self.queue.len()
    }

    pub fn get_buffer(self: &Arc<Self>) -> Arc<Buffer> {
        let buffer_size = self.buffer_size.load(Ordering::Relaxed);
        if let Some(mut buffer) = self.queue.pop() {
            if buffer_size == buffer.size() {
                if let Some(buffer_mut) = Arc::get_mut(&mut buffer) {
                    buffer_mut.data_mut().fill(0);
                    return buffer;
                }
            }
        }

        Arc::new(Buffer {
            data: vec![0u8; buffer_size].into_boxed_slice(),
            pool: Arc::downgrade(self),
        })
    }

    pub fn recycle_buffer(&self, buffer: Arc<Buffer>) {
        if buffer.size() == self.buffer_size.load(Ordering::Relaxed) {
            self.queue.push(buffer);
        }
    }

    pub fn get_buffer_size(&self) -> usize {
        self.buffer_size.load(Ordering::Relaxed)
    }

    pub fn set_buffer_size(&self, buffer_size: usize) {
        self.buffer_size.store(buffer_size, Ordering::Relaxed);
        while self.queue.pop().is_some() {}
    }
}
