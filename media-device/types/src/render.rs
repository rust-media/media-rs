use media_core::{
    frame::{Frame, SharedFrame},
    Result,
};

pub trait RenderHandler {
    fn set_input_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&mut Frame) -> Result<()> + Send + Sync + 'static;
}

pub trait Sink {
    fn write_frame(&mut self, frame: SharedFrame) -> Result<()>;
}
