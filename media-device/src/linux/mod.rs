use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "video")] {
        pub mod libcamera;
    }
}
