use std::sync::Arc;

use media_base::{error::MediaError, media_frame::MediaFrame, Result};
use variant::Variant;

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

pub(crate) type OutputHandler = Arc<dyn Fn(MediaFrame) -> Result<()> + Send + Sync>;

pub trait Device {
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn configure(&mut self, options: Variant) -> Result<()>;
    fn control(&mut self, action: Variant) -> Result<()>;
    fn running(&self) -> bool;
    fn formats(&self) -> Result<Variant>;
}

pub trait OutputDevice: Device {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(MediaFrame) -> Result<()> + Send + Sync + 'static;
}

pub(crate) type DeviceEventHandler = Box<dyn Fn(&DeviceEvent) + Send + Sync>;

pub trait DeviceManager {
    type DeviceType: Device;

    fn init() -> Result<Self>
    where
        Self: Sized;
    fn uninit(&mut self);
    fn list(&self) -> Vec<&Self::DeviceType>;
    fn index(&self, index: usize) -> Option<&Self::DeviceType>;
    fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType>;
    fn lookup(&self, id: &str) -> Option<&Self::DeviceType>;
    fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType>;
    fn refresh(&mut self) -> Result<()>;
    fn set_change_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static;
}
