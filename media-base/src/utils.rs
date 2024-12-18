use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub(crate) const DEFAULT_ALIGNMENT: u32 = 32;
    } else {
        pub(crate) const DEFAULT_ALIGNMENT: u32 = 16;
    }
}

pub fn align_to(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) & !(alignment - 1)
}

pub fn ceil_rshift(value: u32, shift: u32) -> u32 {
    (value + (1 << shift) - 1) >> shift
}
