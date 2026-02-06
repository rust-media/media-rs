use media_core::{rational::Rational64, variant::Variant, Result};

pub const DEFAULT_TIME_BASE: Rational64 = Rational64::new_raw(1, 1_000_000);

pub trait Format: Send + Sync {
    fn set_option(&mut self, key: &str, value: &Variant) -> Result<()>;
}
