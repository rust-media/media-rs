use cfg_if::cfg_if;
use media_core::Result;
use media_device_types::{DeviceEvent, DeviceManager};

cfg_if! {
    if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        #[path = "mac/mod.rs"]
        pub mod backend;

        #[cfg(feature = "capture")]
        pub use backend::core_audio::CoreAudioInputDeviceManager as DefaultMicrophoneManager;
        #[cfg(feature = "render")]
        pub use backend::core_audio::CoreAudioOutputDeviceManager as DefaultSpeakerManager;
    } else if #[cfg(target_os = "windows")] {
        #[path = "windows/mod.rs"]
        pub mod backend;

        #[cfg(feature = "capture")]
        pub use backend::wasapi::WasapiInputDeviceManager as DefaultMicrophoneManager;
        #[cfg(feature = "render")]
        pub use backend::wasapi::WasapiOutputDeviceManager as DefaultSpeakerManager;
    }
}

/// A generic audio device manager that wraps a platform-specific
/// [`DeviceManager`] backend.
///
/// Use it together with one of the default backends, e.g.
/// `AudioDeviceManager::<DefaultMicrophoneManager>::new()` for microphones, or
/// `AudioDeviceManager::<DefaultSpeakerManager>::new()` for speakers.
pub struct AudioDeviceManager<T: DeviceManager> {
    backend: T,
}

impl<T: DeviceManager> AudioDeviceManager<T> {
    pub fn new() -> Result<Self> {
        let mut backend = T::init()?;
        backend.refresh()?;
        Ok(Self {
            backend,
        })
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

    pub fn iter(&self) -> T::Iter<'_> {
        self.backend.iter()
    }

    pub fn iter_mut(&mut self) -> T::IterMut<'_> {
        self.backend.iter_mut()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.backend.refresh()
    }

    pub fn set_change_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static,
    {
        self.backend.set_change_handler(handler)
    }
}

impl<T: DeviceManager> Drop for AudioDeviceManager<T> {
    fn drop(&mut self) {
        self.backend.deinit();
    }
}
