//! Format trait and related types

use media_core::{rational::Rational64, variant::Variant, Result};

/// Default time base for media formats (1 microsecond)
pub const DEFAULT_TIME_BASE: Rational64 = Rational64::new_raw(1, 1_000_000);

/// Base trait for all media format implementations
pub trait Format: Send + Sync {
    /// Set a format-specific option
    fn set_option(&mut self, key: &str, value: &Variant) -> Result<()>;
}

/// Trait for format builders (base for DemuxerBuilder and MuxerBuilder)
pub trait FormatBuilder: Send + Sync {
    /// Returns the name of this format
    fn name(&self) -> &'static str;

    /// Returns the file extensions this format supports (without leading dot)
    fn extensions(&self) -> &[&'static str];
}
