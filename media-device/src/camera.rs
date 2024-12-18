use cfg_if::cfg_if;
use media_base::{
    error::MediaError,
    video::{ColorRange, VideoFormat},
};

use super::{DeviceEvent, DeviceManager};

cfg_if! {
    if #[cfg(target_os = "windows")] {
        pub use super::backend::media_foundation::MediaFoundationDeviceManager as DefaultCameraManager;
    } else if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        pub use super::backend::av_foundation::AVFoundationCaptureDeviceManager as DefaultCameraManager;
    } else {
        compile_error!("unsupported target");
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CameraFormat {
    pub format: VideoFormat,
    pub color_range: ColorRange,
    pub width: u32,
    pub height: u32,
    pub frame_rates: Vec<f32>,
}

pub struct CameraManager<T: DeviceManager> {
    backend: T,
}

impl<T: DeviceManager> CameraManager<T> {
    pub fn new() -> Result<Self, MediaError> {
        let mut backend = T::init()?;
        backend.refresh()?;
        Ok(Self {
            backend,
        })
    }

    pub fn list(&self) -> Vec<&T::DeviceType> {
        self.backend.list()
    }

    pub fn index(&self, index: usize) -> Option<&T::DeviceType> {
        self.backend.index(index)
    }

    pub fn index_mut(&mut self, index: usize) -> Option<&mut T::DeviceType> {
        self.backend.index_mut(index)
    }

    pub fn lookup(&self, id: &str) -> Option<&T::DeviceType> {
        self.backend.lookup(id)
    }

    pub fn lookup_mut(&mut self, id: &str) -> Option<&mut T::DeviceType> {
        self.backend.lookup_mut(id)
    }

    pub fn refresh(&mut self) -> Result<(), MediaError> {
        self.backend.refresh()
    }

    pub fn set_change_handler<F>(&mut self, handler: F) -> Result<(), MediaError>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static,
    {
        self.backend.set_change_handler(handler)
    }
}

impl<T: DeviceManager> Drop for CameraManager<T> {
    fn drop(&mut self) {
        self.backend.uninit();
    }
}

impl CameraManager<DefaultCameraManager> {
    pub fn default() -> Result<Self, MediaError> {
        Self::new()
    }
}
