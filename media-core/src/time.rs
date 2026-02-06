pub const NSEC_PER_USEC: i64 = 1_000;
pub const NSEC_PER_MSEC: i64 = 1_000_000;
pub const NSEC_PER_SEC: i64 = 1_000_000_000;
pub const USEC_PER_MSEC: i64 = 1_000;
pub const USEC_PER_SEC: i64 = 1_000_000;
pub const MSEC_PER_SEC: i64 = 1_000;

pub fn timestamp_usec() -> i64 {
    let now = std::time::Instant::now();
    let duration = now.elapsed();
    duration.as_secs() as i64 * USEC_PER_SEC + duration.subsec_micros() as i64
}

pub fn timestamp_msec() -> i64 {
    timestamp_usec() / USEC_PER_MSEC
}

pub fn timestamp_sec() -> i64 {
    timestamp_usec() / USEC_PER_SEC
}

pub fn tick_count() -> i64 {
    timestamp_msec()
}
