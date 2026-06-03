use media_core::{frame::Frame, Result};

use crate::device::Device;

pub trait RenderDevice: Device {
    fn set_input_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&mut Frame) -> Result<()> + Send + Sync + 'static;
}
