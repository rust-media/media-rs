//! Automatic codec registration
#[allow(unused_imports)]
use std::sync::Arc;

use ctor::ctor;
#[cfg(all(feature = "decoder", feature = "video", feature = "video-toolbox", any(target_os = "macos", target_os = "ios")))]
use media_codec_video_toolbox::decoder::VTDecoderBuilder;

#[cfg(feature = "decoder")]
#[allow(unused_imports)]
use crate::decoder::register_decoder;
#[cfg(feature = "encoder")]
#[allow(unused_imports)]
use crate::encoder::register_encoder;

/// Initializes the codec registry with built-in codecs
#[ctor]
pub fn initialize() {
    // Register decoders
    #[cfg(all(feature = "decoder", feature = "video", feature = "video-toolbox", any(target_os = "macos", target_os = "ios")))]
    register_decoder(Arc::new(VTDecoderBuilder), false);
}
