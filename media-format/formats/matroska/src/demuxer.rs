//! Matroska/WebM demuxer

use std::{io::SeekFrom, num::NonZeroU32};

#[cfg(feature = "video")]
use media_codec_h264::avcc::Avcc;
#[cfg(feature = "video")]
use media_codec_h265::hvcc::Hvcc;
#[cfg(any(feature = "audio", feature = "video"))]
use media_codec_types::decoder::ExtraData;
#[cfg(feature = "audio")]
use media_codec_types::AudioParameters;
#[cfg(feature = "video")]
use media_codec_types::VideoParameters;
use media_codec_types::{
    decoder::DecoderParameters,
    packet::{Packet, PacketFlags},
    CodecID, CodecParameters,
};
#[cfg(feature = "audio")]
use media_core::audio::ChannelLayout;
use media_core::{
    not_found_error,
    rational::Rational64,
    read_failed_error,
    time::{MSEC_PER_SEC, NSEC_PER_SEC, USEC_PER_SEC},
    unsupported_error,
    variant::Variant,
    MediaType, Result,
};
use media_format_types::{
    demuxer::{Demuxer, DemuxerBuilder, DemuxerState, Reader, SeekFlags},
    stream::Stream,
    track::Track,
    Format,
};
use mkv_element::{
    io::blocking_impl::{ReadElement, ReadFrom},
    prelude::{
        BlockGroup, Cluster, Cues, Ebml, Element, Header, Info, Position, PrevSize, SeekHead, Segment, SimpleBlock, Tags, Timestamp, TimestampScale,
        TrackEntry, Tracks, VInt64, Void,
    },
    ClusterBlock, FrameData,
};

const MAX_ELEMENTS: u32 = 8192;
const MAX_UNKNOWN_ELEMENTS: u32 = 8;
const MAX_ELEMENT_SIZE: u64 = 256 * 1024 * 1024;
const MAX_HEADER_ELEMENTS: u32 = 256;
const MAX_SKIPS: u32 = 8;
const SEEK_LOOKAHEAD_SEC: i64 = 10;

/// Document type for Matroska container formats
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DocType {
    #[default]
    Matroska,
    WebM,
}

impl DocType {
    fn from_doc_type(doc_type: &str) -> Self {
        match doc_type.to_lowercase().as_str() {
            "webm" => DocType::WebM,
            _ => DocType::Matroska,
        }
    }
}

/// Matroska/WebM demuxer implementation
pub struct MkvDemuxer {
    /// Document type (.mkv or .webm)
    doc_type: DocType,
    /// Time base in seconds
    time_base: Rational64,
    /// Duration in timestamp units
    duration: Option<f64>,
    /// Cues (index table) for fast seeking
    cues: Option<Cues>,
    /// Segment data start position (for calculating absolute positions from cue
    /// positions)
    segment_data_position: u64,
    /// Current cluster being read
    current_cluster: Option<Cluster>,
    /// Current frame index within the current cluster
    current_frame_index: usize,
    /// Current lace index within the current frame (for laced frames with
    /// multiple sub-frames)
    current_lace_index: usize,
}

impl Default for MkvDemuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl MkvDemuxer {
    /// Creates a new Matroska demuxer
    pub fn new() -> Self {
        Self {
            doc_type: DocType::default(),
            time_base: Rational64::new(TimestampScale::default().0 as i64, NSEC_PER_SEC),
            duration: None,
            cues: None,
            segment_data_position: 0,
            current_cluster: None,
            current_frame_index: 0,
            current_lace_index: 0,
        }
    }

    /// Codec ID mapping based on:
    /// - Matroska Specs: https://www.matroska.org/technical/codec_specs.html
    /// - IETF Draft: https://datatracker.ietf.org/doc/draft-ietf-cellar-codec/
    ///
    /// Note: `A_EAC3` and `V_MPEGI/ISO/VVC` are widely used in practice but
    /// not yet in the official specification.
    fn str_to_codec_id(codec_id_str: &str, media_type: MediaType, doc_type: DocType) -> Option<CodecID> {
        match (media_type, doc_type) {
            #[cfg(feature = "video")]
            (MediaType::Video, DocType::WebM) => match codec_id_str {
                "V_VP8" => Some(CodecID::VP8),
                "V_VP9" => Some(CodecID::VP9),
                "V_AV1" => Some(CodecID::AV1),
                _ => None,
            },
            #[cfg(feature = "video")]
            (MediaType::Video, DocType::Matroska) => match codec_id_str {
                "V_MPEG4/ISO/AVC" => Some(CodecID::H264),
                "V_MPEGH/ISO/HEVC" => Some(CodecID::HEVC),
                "V_MPEGI/ISO/VVC" => Some(CodecID::VVC),
                "V_VP8" => Some(CodecID::VP8),
                "V_VP9" => Some(CodecID::VP9),
                "V_AV1" => Some(CodecID::AV1),
                "V_MPEG1" => Some(CodecID::MPEG1),
                "V_MPEG2" => Some(CodecID::MPEG2),
                "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/AP" => Some(CodecID::MPEG4),
                _ => None,
            },
            #[cfg(feature = "audio")]
            (MediaType::Audio, DocType::WebM) => match codec_id_str {
                "A_VORBIS" => Some(CodecID::VORBIS),
                "A_OPUS" => Some(CodecID::OPUS),
                _ => None,
            },
            #[cfg(feature = "audio")]
            (MediaType::Audio, DocType::Matroska) => match codec_id_str {
                "A_AAC" | "A_AAC/MPEG2/MAIN" | "A_AAC/MPEG2/LC" | "A_AAC/MPEG2/LC/SBR" | "A_AAC/MPEG2/SSR" | "A_AAC/MPEG4/MAIN" |
                "A_AAC/MPEG4/LC" | "A_AAC/MPEG4/LC/SBR" | "A_AAC/MPEG4/SSR" | "A_AAC/MPEG4/LTP" => Some(CodecID::AAC),
                "A_OPUS" => Some(CodecID::OPUS),
                "A_VORBIS" => Some(CodecID::VORBIS),
                "A_FLAC" => Some(CodecID::FLAC),
                "A_AC3" | "A_AC3/BSID9" | "A_AC3/BSID10" => Some(CodecID::AC3),
                "A_EAC3" => Some(CodecID::EAC3),
                "A_DTS" | "A_DTS/EXPRESS" | "A_DTS/LOSSLESS" => Some(CodecID::DTS),
                "A_MPEG/L1" => Some(CodecID::MP1),
                "A_MPEG/L2" => Some(CodecID::MP2),
                "A_MPEG/L3" => Some(CodecID::MP3),
                _ => None,
            },
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Convert Matroska track type to MediaType
    fn track_type_to_media_type(track_type: u64) -> Option<MediaType> {
        match track_type {
            #[cfg(feature = "video")]
            1 => Some(MediaType::Video),
            #[cfg(feature = "audio")]
            2 => Some(MediaType::Audio),
            _ => None,
        }
    }

    /// Create video parameters from track entry
    #[cfg(feature = "video")]
    fn make_video_params(track: &TrackEntry) -> Option<VideoParameters> {
        let video = track.video.as_ref()?;

        Some(VideoParameters {
            width: NonZeroU32::new(video.pixel_width.0 as u32),
            height: NonZeroU32::new(video.pixel_height.0 as u32),
            ..Default::default()
        })
    }

    /// Create audio parameters from track entry
    #[cfg(feature = "audio")]
    fn make_audio_params(track: &TrackEntry) -> Option<AudioParameters> {
        let audio = track.audio.as_ref()?;

        Some(AudioParameters {
            sample_rate: NonZeroU32::new(audio.sampling_frequency.0 as u32),
            channel_layout: ChannelLayout::default_from_channels(audio.channels.0 as u8).ok(),
            ..Default::default()
        })
    }

    /// Create decoder parameters from track entry (codec private data)
    #[cfg(feature = "video")]
    fn make_video_decoder_params(track: &TrackEntry) -> DecoderParameters {
        let codec_id_str: &str = &track.codec_id.0;
        let codec_private = track.codec_private.as_ref().map(|cp| cp.0.as_ref());

        match codec_id_str {
            "V_MPEG4/ISO/AVC" => {
                if let Some(data) = codec_private {
                    if let Some(extra) = Self::parse_avcc(data) {
                        return DecoderParameters {
                            extra_data: Some(extra),
                            ..Default::default()
                        };
                    }
                }
                DecoderParameters::default()
            }
            "V_MPEGH/ISO/HEVC" => {
                if let Some(data) = codec_private {
                    if let Some(extra) = Self::parse_hvcc(data) {
                        return DecoderParameters {
                            extra_data: Some(extra),
                            ..Default::default()
                        };
                    }
                }
                DecoderParameters::default()
            }
            _ => {
                if let Some(data) = codec_private {
                    DecoderParameters {
                        extra_data: Some(ExtraData::Raw(data.to_vec())),
                        ..Default::default()
                    }
                } else {
                    DecoderParameters::default()
                }
            }
        }
    }

    #[cfg(feature = "video")]
    fn parse_avcc(data: &[u8]) -> Option<ExtraData> {
        let avcc = Avcc::parse(data).ok()?;

        Some(ExtraData::AVC {
            sps: avcc.sequence_parameter_sets,
            pps: avcc.picture_parameter_sets,
            nalu_length_size: avcc.length_size,
        })
    }

    #[cfg(feature = "video")]
    fn parse_hvcc(data: &[u8]) -> Option<ExtraData> {
        let hvcc = Hvcc::parse(data).ok()?;

        let vps: Vec<Vec<u8>> = hvcc.vps().map(|v| v.to_vec()).collect();
        let sps: Vec<Vec<u8>> = hvcc.sps().map(|v| v.to_vec()).collect();
        let pps: Vec<Vec<u8>> = hvcc.pps().map(|v| v.to_vec()).collect();

        Some(ExtraData::HEVC {
            vps: if vps.is_empty() {
                None
            } else {
                Some(vps)
            },
            sps,
            pps,
            nalu_length_size: hvcc.length_size,
        })
    }

    /// Convert track entry to codec parameters
    fn track_to_params(track: &TrackEntry, doc_type: DocType) -> Option<(CodecID, CodecParameters)> {
        let codec_id_str: &str = &track.codec_id.0;
        let media_type = Self::track_type_to_media_type(track.track_type.0)?;
        let codec_id = Self::str_to_codec_id(codec_id_str, media_type, doc_type)?;

        match media_type {
            #[cfg(feature = "video")]
            MediaType::Video => {
                let video_params = Self::make_video_params(track)?;
                let decoder_params = Self::make_video_decoder_params(track);
                Some((codec_id, CodecParameters::new(video_params, decoder_params)))
            }
            #[cfg(feature = "audio")]
            MediaType::Audio => {
                let audio_params = Self::make_audio_params(track)?;
                let decoder_params = if let Some(ref cp) = track.codec_private {
                    DecoderParameters {
                        extra_data: Some(ExtraData::Raw(cp.0.to_vec())),
                        ..Default::default()
                    }
                } else {
                    DecoderParameters::default()
                };
                Some((codec_id, CodecParameters::new(audio_params, decoder_params)))
            }
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Find Cues position from SeekHead entries
    fn find_cues_position(seek_heads: &[SeekHead], segment_data_position: u64) -> Option<u64> {
        for seek_head in seek_heads {
            for seek in &seek_head.seek {
                // Parse the SeekID to get the element ID
                let mut id_bytes = &seek.seek_id[..];
                if let Ok(element_id) = VInt64::read_from(&mut id_bytes) {
                    // Check if this is a Cues element
                    if element_id == Cues::ID {
                        // SeekPosition is relative to segment data start
                        return Some(*seek.seek_position + segment_data_position);
                    }
                }
            }
        }
        None
    }

    /// Read an element body into a Vec<u8> for decoding
    fn read_element_body(reader: &mut dyn Reader, size: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size as usize];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Read the next cluster from the reader
    /// Handles both known-size and unknown-size clusters
    fn read_next_cluster(&mut self, reader: &mut dyn Reader) -> Result<bool> {
        // Safety counter to prevent infinite loops
        let mut skip_count = 0;

        loop {
            // Read cluster header
            let header = match Header::read_from(reader) {
                Ok(h) => h,
                Err(_) => return Ok(false), // End of file
            };

            // Handle non-cluster elements between clusters
            if header.id == Cluster::ID {
                // Check if this cluster has unknown size
                if header.size.is_unknown {
                    // Unknown size cluster - need to parse element by element
                    return self.read_unknown_size_cluster(reader);
                }

                // Known size cluster - use normal read_element
                let cluster = match Cluster::read_element(&header, reader) {
                    Ok(c) => c,
                    Err(e) => {
                        // Try to skip failed cluster if size is known
                        if header.size.value > 0 {
                            let _ = reader.seek(SeekFrom::Current(header.size.value as i64));
                            skip_count += 1;
                            if skip_count >= MAX_SKIPS {
                                return Ok(false);
                            }
                            continue;
                        }
                        return Err(read_failed_error!("cluster", e));
                    }
                };

                self.current_frame_index = 0;
                self.current_lace_index = 0;
                self.current_cluster = Some(cluster);
                return Ok(true);
            }

            // Skip non-cluster element
            let size = header.size.value;

            // Check for unknown size - can't skip these reliably
            if header.size.is_unknown {
                // Can't skip unknown size, assume end of readable data
                return Ok(false);
            }

            // Sanity check: don't try to skip unreasonably large elements
            // This prevents hangs from corrupted size values
            if size > MAX_ELEMENT_SIZE {
                return Ok(false);
            }

            // Skip the element
            if size > 0 && reader.seek(SeekFrom::Current(size as i64)).is_err() {
                return Ok(false);
            }

            skip_count += 1;
            if skip_count >= MAX_SKIPS {
                return Ok(false);
            }
        }
    }

    /// Read unknown-size cluster
    fn read_unknown_size_cluster(&mut self, reader: &mut dyn Reader) -> Result<bool> {
        let mut timestamp: Option<u64> = None;
        let mut blocks: Vec<ClusterBlock> = Vec::new();

        // Safety counter
        let mut element_count = 0;
        let mut unknown_element_count = 0;

        while let (Ok(pos_before_header), Ok(sub_header)) = (reader.stream_position(), Header::read_from(reader)) {
            // Check for next top-level element
            if sub_header.id == Cluster::ID {
                // Hit next cluster - seek back to cluster start and exit
                let _ = reader.seek(SeekFrom::Start(pos_before_header));
                break;
            }

            // Check for other top-level elements that indicate end of cluster
            let id_val = sub_header.id.value;
            if id_val == Cues::ID.value || id_val == Tags::ID.value || id_val == Info::ID.value || id_val == Tracks::ID.value {
                // Hit another top-level element - seek back and exit
                let _ = reader.seek(SeekFrom::Start(pos_before_header));
                break;
            }

            // Parse cluster sub-elements
            let element_size = sub_header.size.value;

            // Safety check for element size
            if sub_header.size.is_unknown || element_size > MAX_ELEMENT_SIZE {
                // Can't process unknown size or very large elements
                break;
            }

            match sub_header.id {
                Timestamp::ID => {
                    // Read timestamp
                    if let Ok(body) = Self::read_element_body(reader, element_size) {
                        if let Ok(ts) = Timestamp::decode_body(&mut body.as_slice()) {
                            timestamp = Some(ts.0);
                        }
                    } else {
                        // Read failed, skip by seeking
                        if reader.seek(SeekFrom::Current(element_size as i64)).is_err() {
                            break;
                        }
                    }
                    unknown_element_count = 0;
                }
                SimpleBlock::ID => {
                    // Read SimpleBlock
                    if let Ok(body) = Self::read_element_body(reader, element_size) {
                        if let Ok(block) = SimpleBlock::decode_body(&mut body.as_slice()) {
                            blocks.push(ClusterBlock::Simple(block));
                        }
                    } else {
                        // Read failed, skip by seeking
                        if reader.seek(SeekFrom::Current(element_size as i64)).is_err() {
                            break;
                        }
                    }
                    unknown_element_count = 0;
                }
                BlockGroup::ID => {
                    // Read BlockGroup
                    if let Ok(body) = Self::read_element_body(reader, element_size) {
                        if let Ok(block) = BlockGroup::decode_body(&mut body.as_slice()) {
                            blocks.push(ClusterBlock::Group(block));
                        }
                    } else {
                        // Read failed, skip by seeking
                        if reader.seek(SeekFrom::Current(element_size as i64)).is_err() {
                            break;
                        }
                    }
                    unknown_element_count = 0;
                }
                Position::ID | PrevSize::ID | Void::ID => {
                    // Skip known but unimportant elements
                    if element_size > 0 && reader.seek(SeekFrom::Current(element_size as i64)).is_err() {
                        break;
                    }
                    unknown_element_count = 0;
                }
                _ => {
                    unknown_element_count += 1;
                    if unknown_element_count >= MAX_UNKNOWN_ELEMENTS {
                        // Too many unknown elements, may be corrupted
                        break;
                    }

                    // Skip unknown element
                    if element_size > 0 && reader.seek(SeekFrom::Current(element_size as i64)).is_err() {
                        break;
                    }
                }
            }

            element_count += 1;
            if element_count >= MAX_ELEMENTS {
                break;
            }
        }

        // Must have at least timestamp to form a valid cluster
        let timestamp = match timestamp {
            Some(ts) => ts,
            None => return Ok(false),
        };

        // Build the cluster
        let cluster = Cluster {
            crc32: None,
            void: None,
            timestamp: Timestamp(timestamp),
            position: None,
            prev_size: None,
            blocks,
        };

        self.current_frame_index = 0;
        self.current_lace_index = 0;
        self.current_cluster = Some(cluster);
        Ok(true)
    }
}

impl Format for MkvDemuxer {
    fn set_option(&mut self, _key: &str, _value: &Variant) -> Result<()> {
        Ok(())
    }
}

impl Demuxer for MkvDemuxer {
    fn read_header(&mut self, reader: &mut dyn Reader, state: &mut DemuxerState) -> Result<()> {
        // Read EBML header and extract doc_type
        let ebml = Ebml::read_from(reader).map_err(|e| read_failed_error!("EBML header", e))?;

        // Cache the document type (mkv or webm)
        self.doc_type = ebml.doc_type.as_ref().map(|dt| DocType::from_doc_type(&dt.0)).unwrap_or(DocType::Matroska);

        // Safety limit for header reading
        let mut element_count = 0;

        // Read top-level elements until Segment is found
        loop {
            let header = match Header::read_from(reader) {
                Ok(h) => h,
                Err(e) => {
                    return Err(read_failed_error!("header", e));
                }
            };

            if header.id == Segment::ID {
                // Record segment data start position
                self.segment_data_position = reader.stream_position()?;

                // Read sub-elements until Cluster is found
                let mut info: Option<Info> = None;
                let mut tracks: Option<Tracks> = None;
                let mut cues: Option<Cues> = None;
                let mut seek_head: Vec<SeekHead> = Vec::new();

                loop {
                    let current_pos = reader.stream_position()?;

                    let sub_header = match Header::read_from(reader) {
                        Ok(h) => h,
                        Err(_) => break, // End of file or segment
                    };

                    if sub_header.id == Info::ID {
                        info = Some(Info::read_element(&sub_header, reader).map_err(|e| read_failed_error!(e.to_string()))?);
                    } else if sub_header.id == Tracks::ID {
                        tracks = Some(Tracks::read_element(&sub_header, reader).map_err(|e| read_failed_error!(e.to_string()))?);
                    } else if sub_header.id == Cues::ID {
                        cues = Some(Cues::read_element(&sub_header, reader).map_err(|e| read_failed_error!(e.to_string()))?);
                    } else if sub_header.id == SeekHead::ID {
                        // Parse SeekHead to get positions of other elements
                        if let Ok(sh) = SeekHead::read_element(&sub_header, reader) {
                            seek_head.push(sh);
                        }
                    } else if sub_header.id == Cluster::ID {
                        // Try to locate Cues using SeekHead
                        if cues.is_none() {
                            if let Some(cues_pos) = Self::find_cues_position(&seek_head, self.segment_data_position) {
                                if cues_pos > current_pos && reader.seek(SeekFrom::Start(cues_pos)).is_ok() {
                                    if let Ok(cues_header) = Header::read_from(reader) {
                                        if cues_header.id == Cues::ID {
                                            if let Ok(c) = Cues::read_element(&cues_header, reader) {
                                                cues = Some(c);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Seek back to cluster start for later reading
                        reader.seek(SeekFrom::Start(current_pos))?;
                        break;
                    } else {
                        // Skip unknown elements
                        let size = sub_header.size.value;

                        // Check for unknown size or unreasonably large size
                        if sub_header.size.is_unknown || size > MAX_ELEMENT_SIZE {
                            // Can't reliably skip - stop header reading
                            break;
                        }

                        if size > 0 && reader.seek(SeekFrom::Current(size as i64)).is_err() {
                            break;
                        }
                    }

                    element_count += 1;
                    if element_count >= MAX_HEADER_ELEMENTS {
                        break;
                    }
                }

                // Validate required elements
                let info = info.ok_or_else(|| not_found_error!("info element"))?;

                let timestamp_scale = info.timestamp_scale.0;

                // Store segment info
                self.time_base = Rational64::new(info.timestamp_scale.0 as i64, NSEC_PER_SEC);
                self.duration = info.duration.as_ref().map(|d| d.0);
                self.cues = cues;

                // Calculate duration in microseconds
                if let Some(duration) = self.duration {
                    state.duration = Some((duration * timestamp_scale as f64 / MSEC_PER_SEC as f64) as i64);
                }

                // Process tracks
                if let Some(ref tracks) = tracks {
                    let mut stream = Stream::new(0);

                    for track_entry in &tracks.track_entry {
                        let track_number = track_entry.track_number.0 as isize;

                        if let Some((codec_id, params)) = Self::track_to_params(track_entry, self.doc_type) {
                            let mut track = Track::new(track_number, codec_id, params, self.time_base);

                            // Set track duration if available
                            if let Some(duration) = self.duration {
                                track.duration = Some(duration as i64);
                            }

                            stream.add_track(state.tracks.add_track(track));
                        }
                    }

                    state.streams.add_stream(stream);
                }

                return Ok(());
            } else {
                // Skip unknown top-level elements
                let size = header.size.value;
                if size > 0 {
                    reader.seek(SeekFrom::Current(size as i64))?;
                }
            }
        }
    }

    fn read_packet(&mut self, reader: &mut dyn Reader, state: &DemuxerState) -> Result<Packet<'static>> {
        loop {
            if let Some(ref cluster) = self.current_cluster {
                // Collect frames from current cluster starting at current_frame_index
                let frames: Vec<_> = cluster.frames().filter_map(|r| r.ok()).collect();

                while self.current_frame_index < frames.len() {
                    let frame = &frames[self.current_frame_index];

                    let track = state.tracks.find_track(frame.track_number as isize).ok_or_else(|| not_found_error!("track", frame.track_number))?;

                    // Extract frame data from FrameData enum, handling laced frames
                    let (frame_bytes, advance_frame): (&[u8], bool) = match &frame.data {
                        FrameData::Single(data) => (data, true),
                        FrameData::Multiple(data_vec) => {
                            // For laced frames, return each sub-frame as a separate packet
                            if self.current_lace_index < data_vec.len() {
                                let data = data_vec[self.current_lace_index];
                                self.current_lace_index += 1;
                                // Only advance to next frame when all laced sub-frames are consumed
                                let advance = self.current_lace_index >= data_vec.len();
                                if advance {
                                    self.current_lace_index = 0;
                                }
                                (data, advance)
                            } else {
                                // Should not happen, but reset and move to next frame
                                self.current_lace_index = 0;
                                self.current_frame_index += 1;
                                continue;
                            }
                        }
                    };

                    if advance_frame {
                        self.current_frame_index += 1;
                    }

                    let mut packet = Packet::from_buffer(track.pool.get_buffer_with_length(frame_bytes.len()));
                    if let Some(buffer) = packet.data_mut() {
                        buffer.copy_from_slice(frame_bytes);
                    }

                    packet.track_index = Some(track.index());
                    packet.dts = Some(frame.timestamp);
                    packet.pts = Some(frame.timestamp);
                    packet.time_base = Some(self.time_base);

                    if let Some(duration) = frame.duration {
                        packet.duration = Some(duration.get() as i64);
                    }

                    // Set packet flags - only the first laced frame should be marked as keyframe
                    packet.flags = if frame.is_keyframe &&
                        (matches!(&frame.data, FrameData::Single(_)) ||
                            self.current_lace_index == 1 ||
                            (self.current_lace_index == 0 && advance_frame))
                    {
                        PacketFlags::Key
                    } else {
                        PacketFlags::empty()
                    };

                    return Ok(packet);
                }
            }

            // Need to read the next cluster
            if !self.read_next_cluster(reader)? {
                return Err(not_found_error!("more packets"));
            }
        }
    }

    fn seek(&mut self, reader: &mut dyn Reader, state: &DemuxerState, track_index: Option<usize>, timestamp_us: i64, flags: SeekFlags) -> Result<()> {
        // Convert timestamp to segment ticks
        let target_ticks = Rational64::from_integer(timestamp_us) / (Rational64::from_integer(USEC_PER_SEC) * self.time_base);
        let target_ticks = target_ticks.to_integer();

        // Save current position for rollback
        let saved_position = reader.stream_position()?;

        // Determine target track
        let target_track_index =
            track_index.unwrap_or_else(|| state.tracks.into_iter().find(|t| t.media_type() == MediaType::Video).map(|t| t.index()).unwrap_or(0));

        let target_track = state.tracks.get_track(target_track_index).ok_or_else(|| not_found_error!("track", target_track_index))?;

        let target_track_number = target_track.id as u64;

        // Use Cues for seeking
        let Some(ref cues) = self.cues else {
            return Err(unsupported_error!("seek without Cues index"));
        };

        // Save current cluster state for potential rollback on failure
        let saved_cluster = self.current_cluster.take();
        let saved_frame_index = self.current_frame_index;

        // Collect all cue points for our target track
        let mut track_cues: Vec<(i64, u64)> = Vec::new(); // (cue_time, cluster_pos)

        for cue_point in &cues.cue_point {
            let cue_time = cue_point.cue_time.0 as i64;
            for track_pos in &cue_point.cue_track_positions {
                if track_pos.cue_track.0 == target_track_number {
                    let absolute_pos = self.segment_data_position + track_pos.cue_cluster_position.0;
                    track_cues.push((cue_time, absolute_pos));
                    break;
                }
            }
        }

        // Find the best cue point based on seek flags
        let best_cue = if flags.contains(SeekFlags::BACKWARD) {
            // BACKWARD: Find the largest cue_time <= target
            track_cues.iter().filter(|(t, _)| *t <= target_ticks).max_by_key(|(t, _)| *t).copied()
        } else {
            // Default: Find the nearest cue point (before or after)
            track_cues.iter().min_by_key(|(t, _)| (t - target_ticks).abs()).copied()
        };

        if let Some((_best_cue_time, cluster_pos)) = best_cue {
            // Scan forward to find the frame closest to target
            reader.seek(SeekFrom::Start(cluster_pos))?;

            // Track the best frame before and after target
            let mut best_before: Option<(u64, i64)> = None;
            let mut best_after: Option<(u64, i64)> = None;

            // For ANY mode, search any frame, otherwise search only key frame
            let search_any = flags.contains(SeekFlags::ANY);

            loop {
                let cluster_start_pos = reader.stream_position()?;

                // Try to read next cluster
                if !self.read_next_cluster(reader)? {
                    break; // End of file
                }

                let cluster_timestamp = self.current_cluster.as_ref().map(|c| c.timestamp.0 as i64).unwrap_or(0);

                // Find frames for our target track in this cluster
                if let Some(ref cluster) = self.current_cluster {
                    for frame in cluster.frames().flatten() {
                        // For ANY mode, consider all frames; otherwise only keyframes
                        if frame.track_number == target_track_number && (search_any || frame.is_keyframe) {
                            if frame.timestamp <= target_ticks {
                                // Frame before or at target - update best_before
                                if best_before.is_none() || frame.timestamp > best_before.as_ref().unwrap().1 {
                                    best_before = Some((cluster_start_pos, frame.timestamp));
                                }
                            } else if best_after.is_none() {
                                // First frame after target
                                best_after = Some((cluster_start_pos, frame.timestamp));
                            }
                        }
                    }
                }

                // Decide when to stop scanning
                if flags.contains(SeekFlags::BACKWARD) {
                    // For BACKWARD: stop when cluster timestamp is past target
                    if cluster_timestamp > target_ticks {
                        break;
                    }
                } else {
                    // For default: stop after finding the first frame after target
                    if best_after.is_some() {
                        break;
                    }
                    // Safety: don't search too far past target
                    let lookahead_ticks = (Rational64::from_integer(SEEK_LOOKAHEAD_SEC) / self.time_base).to_integer();
                    if cluster_timestamp > target_ticks + lookahead_ticks {
                        break;
                    }
                }
            }

            // Choose the best frame based on flags
            let chosen = if flags.contains(SeekFlags::BACKWARD) {
                best_before
            } else {
                // Find the nearest frame
                match (best_before, best_after) {
                    (Some(before), Some(after)) => {
                        let dist_before = target_ticks - before.1;
                        let dist_after = after.1 - target_ticks;
                        if dist_before <= dist_after {
                            Some(before)
                        } else {
                            Some(after)
                        }
                    }
                    (Some(before), None) => Some(before),
                    (None, Some(after)) => Some(after),
                    (None, None) => None,
                }
            };

            // Seek to the chosen cluster and set frame index
            if let Some((chosen_cluster_pos, _chosen_ts)) = chosen {
                reader.seek(SeekFrom::Start(chosen_cluster_pos))?;
                self.current_frame_index = 0;
                self.current_lace_index = 0;
                self.current_cluster = None;

                // Re-read the chosen cluster
                self.read_next_cluster(reader)?;

                return Ok(());
            }
        }

        // Seek failed - restore previous state so read_packet can continue
        let _ = reader.seek(SeekFrom::Start(saved_position));
        self.current_cluster = saved_cluster;
        self.current_frame_index = saved_frame_index;

        Err(not_found_error!("seek position"))
    }
}

/// Builder for Matroska/WebM demuxer
pub struct MkvDemuxerBuilder;

impl media_format_types::FormatBuilder for MkvDemuxerBuilder {
    fn name(&self) -> &'static str {
        "matroska"
    }

    fn extensions(&self) -> &[&'static str] {
        &["mkv", "webm"]
    }
}

impl DemuxerBuilder for MkvDemuxerBuilder {
    fn new_demuxer(&self) -> Result<Box<dyn Demuxer>> {
        Ok(Box::new(MkvDemuxer::new()))
    }

    fn probe(&self, reader: &mut dyn Reader) -> bool {
        let mut buf = [0; 4];
        reader.read_exact(&mut buf).ok();
        buf == [0x1A, 0x45, 0xDF, 0xA3]
    }
}
