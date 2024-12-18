use media_base::{error::MediaError, media_frame::MediaFrame};
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

pub trait Device {
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn start(&mut self) -> Result<(), MediaError>;
    fn stop(&mut self) -> Result<(), MediaError>;
    fn configure(&mut self, options: Variant) -> Result<(), MediaError>;
    fn control(&mut self, action: Variant) -> Result<(), MediaError>;
    fn running(&self) -> bool;
    fn formats(&self) -> Result<Variant, MediaError>;
}

pub trait OutputDevice: Device {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<(), MediaError>
    where
        F: Fn(MediaFrame) -> Result<(), MediaError> + Send + Sync + 'static;
}

pub trait DeviceManager {
    type DeviceType: Device;

    fn init() -> Result<Self, MediaError>
    where
        Self: Sized;
    fn uninit(&mut self);
    fn list(&self) -> Vec<&Self::DeviceType>;
    fn index(&self, index: usize) -> Option<&Self::DeviceType>;
    fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType>;
    fn lookup(&self, id: &str) -> Option<&Self::DeviceType>;
    fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType>;
    fn refresh(&mut self) -> Result<(), MediaError>;
    fn set_change_handler<F>(&mut self, handler: F) -> Result<(), MediaError>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static;
}
