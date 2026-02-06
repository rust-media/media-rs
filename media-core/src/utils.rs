use std::ops::{Add, BitAnd, Not, Shl, Shr, Sub};

use cfg_if::cfg_if;
use num_traits::One;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[allow(dead_code)]
        pub(crate) const DEFAULT_ALIGNMENT: usize = 32;
    } else if #[cfg(target_arch = "wasm32")] {
        #[allow(dead_code)]
        pub(crate) const DEFAULT_ALIGNMENT: usize = 8;
    } else {
        #[allow(dead_code)]
        pub(crate) const DEFAULT_ALIGNMENT: usize = 16;
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

#[macro_export]
macro_rules! fourcc_le {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        (($a as u8) as u32) | ((($b as u8) as u32) << 8) | ((($c as u8) as u32) << 16) | ((($d as u8) as u32) << 24)
    };

    ($s:expr) => {{
        const BYTES: &[u8] = $s;
        const _: () = assert!(BYTES.len() == 4, "FourCC must be exactly 4 bytes");
        $crate::fourcc_le!(BYTES[0], BYTES[1], BYTES[2], BYTES[3])
    }};
}

#[macro_export]
macro_rules! fourcc_be {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        ((($a as u8) as u32) << 24) | ((($b as u8) as u32) << 16) | ((($c as u8) as u32) << 8) | (($d as u8) as u32)
    };

    ($s:expr) => {{
        const BYTES: &[u8] = $s;
        const _: () = assert!(BYTES.len() == 4, "FourCC must be exactly 4 bytes");
        $crate::fourcc_be!(BYTES[0], BYTES[1], BYTES[2], BYTES[3])
    }};
}

#[macro_export]
macro_rules! fourcc {
    ($($tt:tt)*) => {
        $crate::fourcc_le!($($tt)*)
    };
}
