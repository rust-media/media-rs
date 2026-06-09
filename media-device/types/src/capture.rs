use media_core::{frame::Frame, Result};

pub trait CaptureHanlder {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static;
}

pub trait Source {
    fn read_frame(&mut self) -> Result<Frame<'_>>;
}
