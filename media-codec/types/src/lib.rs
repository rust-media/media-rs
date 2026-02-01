#[cfg(any(feature = "decoder", feature = "encoder"))]
pub mod codec;
#[cfg(feature = "decoder")]
pub mod decoder;
#[cfg(feature = "encoder")]
pub mod encoder;
pub mod packet;

#[cfg(any(feature = "decoder", feature = "encoder"))]
pub use codec::*;
