use media_core::{frame::Frame, Result};

use crate::device::Device;

pub trait CaptureDevice: Device {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static;
}
