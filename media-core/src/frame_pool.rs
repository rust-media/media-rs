use std::sync::{Arc, RwLock};

use crossbeam_queue::SegQueue;

use crate::{
    error::Error,
    frame::{Frame, SharedFrame},
    FrameDescriptor, Result,
};

pub trait FrameCreator: Send + Sync {
    fn create_frame(&self, desc: FrameDescriptor) -> Result<Frame<'static>>;
}

pub struct DefaultFrameCreator;

impl FrameCreator for DefaultFrameCreator {
    fn create_frame(&self, desc: FrameDescriptor) -> Result<Frame<'static>> {
        Frame::with_descriptor(desc)
    }
}

impl From<DefaultFrameCreator> for Arc<dyn FrameCreator> {
    fn from(creator: DefaultFrameCreator) -> Self {
        Arc::new(creator)
    }
}

pub struct FramePoolConfig {
    pub desc: Option<FrameDescriptor>,
    pub creator: Arc<dyn FrameCreator>,
}

pub struct FramePool<T = RwLock<Frame<'static>>> {
    queue: SegQueue<SharedFrame<T>>,
    config: Arc<RwLock<FramePoolConfig>>,
}

impl<T> FramePool<T> {
    pub fn new(desc: Option<FrameDescriptor>, creator: Option<Box<dyn FrameCreator>>) -> Arc<Self> {
        Arc::new(Self {
            queue: SegQueue::new(),
            config: Arc::new(RwLock::new(FramePoolConfig {
                desc,
                creator: creator.map_or_else(|| DefaultFrameCreator.into(), Arc::from),
            })),
        })
    }

    pub fn available(&self) -> usize {
        self.queue.len()
    }

    pub fn configure(&self, desc: Option<FrameDescriptor>, creator: Option<Box<dyn FrameCreator>>) {
        let need_clear = {
            let mut config = self.config.write().unwrap();

            let desc_changed = desc.is_some_and(|desc| {
                if config.desc.as_ref() != Some(&desc) {
                    config.desc = Some(desc);
                    true
                } else {
                    false
                }
            });

            let creator_changed = creator.is_some_and(|creator| {
                config.creator = Arc::from(creator);
                true
            });

            desc_changed || creator_changed
        };

        if need_clear {
            self.clear();
        }
    }

    pub fn recycle_frame(&self, frame: SharedFrame<T>) {
        self.queue.push(frame);
    }

    fn clear(&self) {
        while self.queue.pop().is_some() {}
    }

    fn get_frame_internal<G, N>(self: &Arc<Self>, get_frame_desc: G, new_shared_frame: N) -> Result<SharedFrame<T>>
    where
        G: Fn(&SharedFrame<T>) -> FrameDescriptor,
        N: Fn(Frame<'static>) -> SharedFrame<T>,
    {
        let (desc, creator) = {
            let config = self.config.read().unwrap();

            if let Some(desc) = config.desc.clone() {
                (desc, config.creator.clone())
            } else {
                return Err(Error::Invalid("frame descriptor".to_string()));
            }
        };

        if let Some(mut frame) = self.queue.pop() {
            if get_frame_desc(&frame) == desc {
                frame.pool = Some(Arc::downgrade(self));
                return Ok(frame);
            }
        }

        let frame = creator.create_frame(desc)?;
        let mut shared_frame = new_shared_frame(frame);
        shared_frame.pool = Some(Arc::downgrade(self));

        Ok(shared_frame)
    }

    fn set_frame_descriptor(&self, desc: FrameDescriptor) {
        let need_update = { self.config.read().unwrap().desc.as_ref() != Some(&desc) };

        if need_update {
            self.config.write().unwrap().desc = Some(desc);
            self.clear();
        }
    }
}

impl FramePool<RwLock<Frame<'static>>> {
    pub fn get_frame(self: &Arc<Self>) -> Result<SharedFrame<RwLock<Frame<'static>>>> {
        self.get_frame_internal(|shared_frame| shared_frame.read().unwrap().desc.clone(), |frame| SharedFrame::<RwLock<Frame<'static>>>::new(frame))
    }

    pub fn get_frame_with_descriptor(self: &Arc<Self>, desc: FrameDescriptor) -> Result<SharedFrame<RwLock<Frame<'static>>>> {
        self.set_frame_descriptor(desc);
        self.get_frame()
    }
}

impl FramePool<Frame<'static>> {
    pub fn get_frame(self: &Arc<Self>) -> Result<SharedFrame<Frame<'static>>> {
        self.get_frame_internal(|shared_frame| shared_frame.read().desc.clone(), |frame| SharedFrame::<Frame<'static>>::new(frame))
    }

    pub fn get_frame_with_descriptor(self: &Arc<Self>, desc: FrameDescriptor) -> Result<SharedFrame<Frame<'static>>> {
        self.set_frame_descriptor(desc);
        self.get_frame()
    }
}
