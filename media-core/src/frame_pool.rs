use std::{
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use crossbeam_queue::SegQueue;

use crate::{
    error::Error,
    frame::{Frame, SharedFrame, SharedFrameInner},
    FrameDescriptorSpec, Result,
};

pub trait FrameCreator<D: FrameDescriptorSpec>: Send + Sync {
    fn create_frame(&self, desc: D) -> Result<Frame<'static, D>>;
}

pub type GenericFrameCreator<D> = dyn FrameCreator<D>;

pub struct DefaultFrameCreator<D: FrameDescriptorSpec> {
    _phantom: PhantomData<D>,
}

impl<D: FrameDescriptorSpec> Default for DefaultFrameCreator<D> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<D: FrameDescriptorSpec> FrameCreator<D> for DefaultFrameCreator<D> {
    fn create_frame(&self, desc: D) -> Result<Frame<'static, D>> {
        desc.create_frame()
    }
}

impl<D: FrameDescriptorSpec> From<DefaultFrameCreator<D>> for Arc<dyn FrameCreator<D>>
where
    D: FrameDescriptorSpec,
    DefaultFrameCreator<D>: FrameCreator<D>,
{
    fn from(creator: DefaultFrameCreator<D>) -> Self {
        Arc::new(creator)
    }
}

pub struct FramePoolConfig<D: FrameDescriptorSpec> {
    pub desc: Option<D>,
    pub creator: Arc<dyn FrameCreator<D>>,
}

pub struct FramePool<F: SharedFrameInner = RwLock<Frame<'static>>> {
    queue: SegQueue<SharedFrame<F>>,
    config: Arc<RwLock<FramePoolConfig<F::Descriptor>>>,
}

impl<F: SharedFrameInner> FramePool<F> {
    pub fn new() -> Arc<Self>
    where
        DefaultFrameCreator<F::Descriptor>: FrameCreator<F::Descriptor>,
    {
        Arc::new(Self {
            queue: SegQueue::new(),
            config: Arc::new(RwLock::new(FramePoolConfig {
                desc: None,
                creator: DefaultFrameCreator::<F::Descriptor>::default().into(),
            })),
        })
    }

    pub fn new_with_creator(desc: F::Descriptor, creator: Box<GenericFrameCreator<F::Descriptor>>) -> Arc<Self> {
        Arc::new(Self {
            queue: SegQueue::new(),
            config: Arc::new(RwLock::new(FramePoolConfig {
                desc: Some(desc),
                creator: Arc::from(creator),
            })),
        })
    }

    pub fn available(&self) -> usize {
        self.queue.len()
    }

    pub fn configure(&self, desc: Option<F::Descriptor>, creator: Option<Box<GenericFrameCreator<F::Descriptor>>>) {
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

    pub fn recycle_frame(&self, frame: SharedFrame<F>) {
        self.queue.push(frame);
    }

    fn clear(&self) {
        while self.queue.pop().is_some() {}
    }

    fn get_frame_internal<G, N>(self: &Arc<Self>, get_frame_desc: G, new_shared_frame: N) -> Result<SharedFrame<F>>
    where
        G: Fn(&SharedFrame<F>) -> F::Descriptor,
        N: Fn(Frame<'static, F::Descriptor>) -> SharedFrame<F>,
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

    fn set_frame_descriptor(&self, desc: F::Descriptor) {
        let need_update = { self.config.read().unwrap().desc.as_ref() != Some(&desc) };

        if need_update {
            self.config.write().unwrap().desc = Some(desc);
            self.clear();
        }
    }
}

impl<D: FrameDescriptorSpec> FramePool<RwLock<Frame<'static, D>>> {
    pub fn get_frame(self: &Arc<Self>) -> Result<SharedFrame<RwLock<Frame<'static, D>>>> {
        self.get_frame_internal(
            |shared_frame| shared_frame.read().unwrap().desc.clone(),
            |frame| SharedFrame::<RwLock<Frame<'static, D>>>::new(frame),
        )
    }

    pub fn get_frame_with_descriptor(self: &Arc<Self>, desc: D) -> Result<SharedFrame<RwLock<Frame<'static, D>>>> {
        self.set_frame_descriptor(desc);
        self.get_frame()
    }
}

impl<D: FrameDescriptorSpec> FramePool<Frame<'static, D>> {
    pub fn get_frame(self: &Arc<Self>) -> Result<SharedFrame<Frame<'static, D>>> {
        self.get_frame_internal(|shared_frame| shared_frame.read().desc.clone(), |frame| SharedFrame::<Frame<'static, D>>::new(frame))
    }

    pub fn get_frame_with_descriptor(self: &Arc<Self>, desc: D) -> Result<SharedFrame<Frame<'static, D>>> {
        self.set_frame_descriptor(desc);
        self.get_frame()
    }
}
