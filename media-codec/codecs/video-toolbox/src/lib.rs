#![cfg(any(target_os = "macos", target_os = "ios"))]

#[cfg(feature = "decoder")]
pub mod decoder;
