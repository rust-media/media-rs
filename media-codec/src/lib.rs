#[cfg(any(feature = "decoder", feature = "encoder"))]
pub mod codec;
#[cfg(any(feature = "decoder", feature = "encoder"))]
pub mod codecs_registry;
#[cfg(feature = "decoder")]
pub mod decoder;
#[cfg(feature = "encoder")]
pub mod encoder;

#[cfg(any(feature = "decoder", feature = "encoder"))]
pub use media_codec_types::codec::*;
pub use media_codec_types::packet;
