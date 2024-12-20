pub const NSEC_PER_USEC: u64 = 1_000;
pub const NSEC_PER_MSEC: u64 = 1_000_000;
pub const NSEC_PER_SEC: u64 = 1_000_000_000;
pub const USEC_PER_MSEC: u64 = 1_000;
pub const USEC_PER_SEC: u64 = 1_000_000;
pub const MSEC_PER_SEC: u64 = 1_000;

pub fn timestamp_usec() -> u64 {
    let now = std::time::Instant::now();
    let duration = now.elapsed();
    duration.as_secs() * USEC_PER_SEC + duration.subsec_micros() as u64
}

pub fn timestamp_msec() -> u64 {
    timestamp_usec() / USEC_PER_MSEC
}

pub fn timestamp_sec() -> u64 {
    timestamp_usec() / USEC_PER_SEC
}

pub fn tick_count() -> u64 {
    timestamp_msec()
}
