//! Automatic format registration
#[allow(unused_imports)]
use std::sync::Arc;

use ctor::ctor;
#[cfg(all(feature = "demuxer", feature = "isomp4"))]
use media_format_isomp4::Mp4DemuxerBuilder;
#[cfg(all(feature = "demuxer", feature = "matroska"))]
use media_format_matroska::MkvDemuxerBuilder;

#[cfg(feature = "demuxer")]
#[allow(unused_imports)]
use crate::demuxer::register_demuxer;

/// Initializes the format registry with built-in formats
#[ctor]
fn initialize() {
    // Register demuxers
    #[cfg(all(feature = "demuxer", feature = "isomp4"))]
    let _ = register_demuxer(Arc::new(Mp4DemuxerBuilder));
    #[cfg(all(feature = "demuxer", feature = "matroska"))]
    let _ = register_demuxer(Arc::new(MkvDemuxerBuilder));
}
