extern crate x_variant as variant;

pub mod audio;
pub mod audio_frame;
pub mod data;
pub mod data_frame;
pub mod error;
pub mod media;
pub mod media_frame;
pub mod video;
pub mod video_frame;

mod time;
mod utils;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod pixel_buffer;

use utils::*;
