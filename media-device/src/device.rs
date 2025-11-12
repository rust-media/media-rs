use std::sync::Arc;

use media_core::{frame::Frame, variant::Variant, Result};

#[derive(Clone, Debug)]
pub struct DeviceInformation {
    pub id: String,
    pub name: String,
}

pub enum DeviceEvent {
    Added(DeviceInformation), // Device added
    Removed(String),          // Device removed, removed device ID
    Refreshed(usize),         // All devices refreshed, number of devices
}

#[allow(unused)]
pub(crate) type OutputHandler = Arc<dyn Fn(Frame) -> Result<()> + Send + Sync>;

pub trait Device {
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn configure(&mut self, options: &Variant) -> Result<()>;
    fn control(&mut self, action: &Variant) -> Result<()>;
    fn running(&self) -> bool;
    fn formats(&self) -> Result<Variant>;
}

pub trait OutputDevice: Device {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static;
}

#[allow(unused)]
pub(crate) type DeviceEventHandler = Box<dyn Fn(&DeviceEvent) + Send + Sync>;

pub trait DeviceManager {
    type DeviceType: Device;
    type Iter<'a>: Iterator<Item = &'a Self::DeviceType>
    where
        Self: 'a;
    type IterMut<'a>: Iterator<Item = &'a mut Self::DeviceType>
    where
        Self: 'a;

    fn init() -> Result<Self>
    where
        Self: Sized;
    fn deinit(&mut self);
    fn index(&self, index: usize) -> Option<&Self::DeviceType>;
    fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType>;
    fn lookup(&self, id: &str) -> Option<&Self::DeviceType>;
    fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType>;
    fn iter(&self) -> Self::Iter<'_>;
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
    fn refresh(&mut self) -> Result<()>;
    fn set_change_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static;
}
