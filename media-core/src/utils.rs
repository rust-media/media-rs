use std::ops::{Add, BitAnd, Not, Shl, Shr, Sub};

use cfg_if::cfg_if;
use num_traits::One;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[allow(dead_code)]
        pub(crate) const DEFAULT_ALIGNMENT: u32 = 32;
    } else {
        #[allow(dead_code)]
        pub(crate) const DEFAULT_ALIGNMENT: u32 = 16;
    }
}

#[allow(dead_code)]
pub fn align_to<T>(value: T, alignment: T) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + BitAnd<Output = T> + Not<Output = T> + One,
{
    (value + alignment - T::one()) & !(alignment - T::one())
}

#[allow(dead_code)]
pub fn ceil_rshift<T>(value: T, shift: T) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Shl<Output = T> + Shr<Output = T> + One,
{
    (value + (T::one() << shift) - T::one()) >> shift
}
