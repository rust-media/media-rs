#[cfg(feature = "video")]
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "ios"))]
pub mod camera;

mod device;

use cfg_if::cfg_if;
pub use device::*;

cfg_if! {
    if #[cfg(target_os = "windows")] {
        #[path = "windows/mod.rs"]
        pub mod backend;
    } else if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        #[path = "mac/mod.rs"]
        pub mod backend;
    } else if #[cfg(target_os = "linux")] {
        #[path = "linux/mod.rs"]
        pub mod backend;
    } else {
        compile_error!("unsupported os");
    }
}
