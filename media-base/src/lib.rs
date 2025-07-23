pub extern crate x_variant as variant;

pub mod audio;
pub mod audio_frame;
pub mod data;
pub mod data_frame;
pub mod error;
pub mod media;
pub mod media_frame;
pub mod time;
pub mod video;
pub mod video_frame;
pub mod video_frame_convert;

mod utils;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod pixel_buffer;

use std::result;

pub(crate) use utils::*;

use crate::error::MediaError;

pub type Result<T> = result::Result<T, MediaError>;
