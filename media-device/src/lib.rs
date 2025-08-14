pub mod camera;
mod device;

use cfg_if::cfg_if;
pub use device::*;

cfg_if! {
    if #[cfg(any(target_os = "ios", target_os = "macos"))] {
        #[path = "mac/mod.rs"]
        pub mod backend;
    } else if #[cfg(target_os = "windows")] {
        #[path = "windows/mod.rs"]
        pub mod backend;
    } else if #[cfg(target_family = "wasm")] {
        #[path = "web/mod.rs"]
        pub mod backend;
    } else {
        compile_error!("unsupported target");
    }
}
