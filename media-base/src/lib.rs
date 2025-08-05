pub use x_variant as variant;

pub mod audio;
pub mod audio_frame;
pub mod data;
pub mod data_frame;
pub mod error;
pub mod frame;
pub mod media;
pub mod time;
pub mod video;
pub mod video_frame;
pub mod video_frame_convert;

mod utils;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod pixel_buffer;

pub use media::*;
pub(crate) use utils::*;

use crate::error::Error;

pub type Result<T> = std::result::Result<T, Error>;
