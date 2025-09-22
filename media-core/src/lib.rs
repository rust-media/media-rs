use cfg_if::cfg_if;
pub use x_variant as variant;

cfg_if! {
    if #[cfg(feature = "audio")] {
        pub mod audio;
    }
}

cfg_if! {
    if #[cfg(feature = "video")] {
        pub mod video;
    }
}

pub mod data;
pub mod error;
pub mod frame;
pub mod media;
pub mod time;

pub mod rational {
    pub use num_rational::Rational64;
}

mod utils;

pub use media::*;
#[allow(unused_imports)]
pub(crate) use utils::*;

use crate::error::Error;

pub type Result<T> = std::result::Result<T, Error>;
