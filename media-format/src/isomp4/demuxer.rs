use std::{io::SeekFrom, num::NonZeroU32};

#[cfg(feature = "audio")]
use media_codec::AudioParameters;
use media_codec::{
    decoder::DecoderParameters,
    packet::{Packet, PacketFlags},
    CodecID, CodecParameters,
};
#[cfg(feature = "video")]
use media_codec::{decoder::ExtraData, VideoParameters};
#[cfg(feature = "audio")]
use media_core::audio::ChannelLayout;
#[cfg(feature = "video")]
use media_core::video::ColorRange;
use media_core::{invalid_error, not_found_error, rational::Rational64, time::USEC_PER_SEC, variant::Variant, MediaType, Result};
#[cfg(feature = "audio")]
use mp4_atom::Audio;
use mp4_atom::{Atom, Codec as Mp4Codec, Ftyp, Header, Mdat, Moov, ReadAtom, ReadFrom, Stbl, StszSamples};
#[cfg(feature = "video")]
use mp4_atom::{Avcc, Colr, Hvcc, Visual};

use crate::{
    demuxer::{Demuxer, DemuxerState, Reader, SeekFlags},
    format::Format,
    stream::Stream,
    track::Track,
};

pub struct Mp4Demuxer {
    pub ftyp: Option<Ftyp>,
    pub moov: Option<Moov>,
    // Track current sample index for each track
    track_sample_indices: Vec<usize>,
}

impl Default for Mp4Demuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mp4Demuxer {
    pub fn new() -> Self {
        Self {
            ftyp: None,
            moov: None,
            track_sample_indices: Vec::new(),
        }
    }

    #[cfg(feature = "video")]
    fn make_video_params(visual: &Visual, colr: Option<&Colr>) -> VideoParameters {
        let mut video_params = VideoParameters {
            width: NonZeroU32::new(visual.width as u32),
            height: NonZeroU32::new(visual.height as u32),
            ..Default::default()
        };

        let Some(colr) = colr else { return video_params };

        let (primaries, transfer, matrix, range) = match colr {
            Colr::Nclx {
                colour_primaries,
                transfer_characteristics,
                matrix_coefficients,
                full_range_flag,
            } => (
                *colour_primaries,
                *transfer_characteristics,
                *matrix_coefficients,
                Some(if *full_range_flag {
                    ColorRange::Full
                } else {
                    ColorRange::Video
                }),
            ),
            _ => return video_params,
        };

        video_params.color_primaries = (primaries as usize).try_into().ok();
        video_params.color_transfer_characteristics = (transfer as usize).try_into().ok();
        video_params.color_matrix = (matrix as usize).try_into().ok();
        video_params.color_range = range;

        video_params
    }

    #[cfg(feature = "audio")]
    fn make_audio_params(audio: &Audio) -> AudioParameters {
        AudioParameters {
            sample_rate: NonZeroU32::new(audio.sample_rate.integer() as u32),
            channel_layout: ChannelLayout::default_from_channels(audio.channel_count as u8).ok(),
            ..Default::default()
        }
    }

    #[cfg(feature = "video")]
    fn make_avc_codec_params(avc: &Avcc) -> DecoderParameters {
        DecoderParameters {
            extra_data: Some(ExtraData::AVC {
                sps: avc.sequence_parameter_sets.clone(),
                pps: avc.picture_parameter_sets.clone(),
                nalu_length_size: avc.length_size,
            }),
            ..Default::default()
        }
    }

    #[cfg(feature = "video")]
    fn make_hevc_codec_params(hvcc: &Hvcc) -> DecoderParameters {
        let mut decoder_params = DecoderParameters::default();

        let mut vps: Option<Vec<Vec<u8>>> = None;
        let mut sps = Vec::new();
        let mut pps = Vec::new();

        for array in &hvcc.arrays {
            match array.nal_unit_type {
                32 => vps.get_or_insert_with(Vec::new).extend(array.nalus.iter().cloned()),
                33 => sps.extend(array.nalus.iter().cloned()),
                34 => pps.extend(array.nalus.iter().cloned()),
                _ => {}
            }
        }

        decoder_params.extra_data = Some(ExtraData::HEVC {
            vps,
            sps,
            pps,
            nalu_length_size: hvcc.length_size_minus_one + 1,
        });

        decoder_params
    }

    fn codec_to_params(codec: &Mp4Codec) -> Option<(CodecID, CodecParameters)> {
        match codec {
            #[cfg(feature = "video")]
            Mp4Codec::Avc1(avc1) => {
                let video_params = Self::make_video_params(&avc1.visual, avc1.colr.as_ref());
                let decoder_params = Self::make_avc_codec_params(&avc1.avcc);
                Some((CodecID::H264, CodecParameters::new(video_params, decoder_params)))
            }
            #[cfg(feature = "video")]
            Mp4Codec::Hev1(hev1) => {
                let video_params = Self::make_video_params(&hev1.visual, hev1.colr.as_ref());
                let decoder_params = Self::make_hevc_codec_params(&hev1.hvcc);
                Some((CodecID::HEVC, CodecParameters::new(video_params, decoder_params)))
            }
            #[cfg(feature = "video")]
            Mp4Codec::Hvc1(hvc1) => {
                let video_params = Self::make_video_params(&hvc1.visual, hvc1.colr.as_ref());
                let decoder_params = Self::make_hevc_codec_params(&hvc1.hvcc);
                Some((CodecID::HEVC, CodecParameters::new(video_params, decoder_params)))
            }
            #[cfg(feature = "video")]
            Mp4Codec::Vp08(vp08) => {
                let video_params = Self::make_video_params(&vp08.visual, vp08.colr.as_ref());
                Some((CodecID::VP8, CodecParameters::new(video_params, DecoderParameters::default())))
            }
            #[cfg(feature = "video")]
            Mp4Codec::Vp09(vp09) => {
                let video_params = Self::make_video_params(&vp09.visual, vp09.colr.as_ref());
                Some((CodecID::VP9, CodecParameters::new(video_params, DecoderParameters::default())))
            }
            #[cfg(feature = "video")]
            Mp4Codec::Av01(av01) => {
                let video_params = Self::make_video_params(&av01.visual, av01.colr.as_ref());
                Some((CodecID::AV1, CodecParameters::new(video_params, DecoderParameters::default())))
            }
            #[cfg(feature = "audio")]
            Mp4Codec::Mp4a(mp4a) => {
                let audio_params = Self::make_audio_params(&mp4a.audio);
                Some((CodecID::AAC, CodecParameters::new(audio_params, DecoderParameters::default())))
            }
            #[cfg(feature = "audio")]
            Mp4Codec::Opus(opus) => {
                let audio_params = Self::make_audio_params(&opus.audio);
                Some((CodecID::OPUS, CodecParameters::new(audio_params, DecoderParameters::default())))
            }
            #[cfg(feature = "audio")]
            Mp4Codec::Flac(flac) => {
                let audio_params = Self::make_audio_params(&flac.audio);
                Some((CodecID::FLAC, CodecParameters::new(audio_params, DecoderParameters::default())))
            }
            #[cfg(feature = "audio")]
            Mp4Codec::Ac3(ac3) => {
                let audio_params = Self::make_audio_params(&ac3.audio);
                Some((CodecID::AC3, CodecParameters::new(audio_params, DecoderParameters::default())))
            }
            #[cfg(feature = "audio")]
            Mp4Codec::Eac3(eac3) => {
                let audio_params = Self::make_audio_params(&eac3.audio);
                Some((CodecID::EAC3, CodecParameters::new(audio_params, DecoderParameters::default())))
            }
            _ => None,
        }
    }

    fn find_sample_index(stbl: &Stbl, target_dts: i64) -> usize {
        let mut accumulated_dts = 0i64;
        let mut sample_index = 0usize;

        for entry in &stbl.stts.entries {
            let samples_in_entry = entry.sample_count as usize;
            let entry_duration = entry.sample_count as i64 * entry.sample_delta as i64;

            if accumulated_dts + entry_duration > target_dts {
                let offset = (target_dts - accumulated_dts) / entry.sample_delta as i64;
                sample_index += offset as usize;
                break;
            }

            accumulated_dts += entry_duration;
            sample_index += samples_in_entry;
        }

        // Clamp to valid range
        let total_samples = match &stbl.stsz.samples {
            StszSamples::Identical {
                count, ..
            } => *count as usize,
            StszSamples::Different {
                sizes,
            } => sizes.len(),
        };
        sample_index.min(total_samples.saturating_sub(1))
    }
}

impl Format for Mp4Demuxer {
    fn set_option(&mut self, _key: &str, _value: &Variant) -> Result<()> {
        Ok(())
    }
}

impl Demuxer for Mp4Demuxer {
    fn read_header<R: Reader>(&mut self, reader: &mut R, state: &mut DemuxerState) -> Result<()> {
        // Read atoms until find moov
        loop {
            let header = match Header::read_from(reader) {
                Ok(h) => h,
                Err(e) => {
                    if self.moov.is_none() {
                        return Err(not_found_error!("moov"));
                    }
                    return Err(invalid_error!(e.to_string()));
                }
            };

            match header.kind {
                Ftyp::KIND => {
                    let ftyp = Ftyp::read_atom(&header, reader).map_err(|e| invalid_error!(e.to_string()))?;
                    self.ftyp = Some(ftyp);
                }
                Moov::KIND => {
                    let moov = Moov::read_atom(&header, reader).map_err(|e| invalid_error!(e.to_string()))?;

                    // Initialize track_sample_indices with the number of tracks
                    self.track_sample_indices = vec![0; moov.trak.len()];

                    // Create a single stream
                    let mut stream = Stream::new(0);

                    // Process each track and add to stream
                    for trak in &moov.trak {
                        let track_id = trak.tkhd.track_id as isize;
                        let timescale = trak.mdia.mdhd.timescale;
                        let time_base = Rational64::new(1, timescale as i64);

                        // Get codec info from stsd
                        if let Some(codec) = trak.mdia.minf.stbl.stsd.codecs.first() {
                            if let Some((codec_id, params)) = Self::codec_to_params(codec) {
                                let mut track = Track::new(track_id, codec_id, params, time_base);
                                track.duration = Some(trak.mdia.mdhd.duration as i64);
                                stream.add_track(state.tracks.add_track(track));
                            }
                        }
                    }

                    state.streams.add_stream(stream);

                    let timescale = moov.mvhd.timescale as i64;
                    let duration = moov.mvhd.duration as i64;
                    if timescale > 0 && duration > 0 {
                        state.duration = Some(duration * USEC_PER_SEC / timescale);
                    }

                    self.moov = Some(moov);

                    return Ok(());
                }
                Mdat::KIND => {
                    // Skip mdat atom, read data later
                    let skip_size = header.size.unwrap_or(0) as i64;
                    reader.seek(SeekFrom::Current(skip_size))?;
                }
                _ => {
                    // Skip unknown atoms
                    if let Some(size) = header.size {
                        reader.seek(SeekFrom::Current(size as i64))?;
                    }
                }
            }
        }
    }

    fn read_packet<R: Reader>(&mut self, reader: &mut R, state: &DemuxerState) -> Result<Packet<'static>> {
        let moov = self.moov.as_ref().ok_or_else(|| not_found_error!("moov"))?;

        // Find the track with the earliest next sample
        let mut earliest_track_idx: Option<usize> = None;
        let mut earliest_dts_us = i64::MAX;
        let mut earliest_dts_raw = 0i64; // DTS in track's native timescale

        for (track_idx, trak) in moov.trak.iter().enumerate() {
            let sample_index = self.track_sample_indices[track_idx];

            // Check if this track has more samples
            let stts = &trak.mdia.minf.stbl.stts;
            let mut total_samples = 0u32;
            for entry in &stts.entries {
                total_samples += entry.sample_count;
            }

            if sample_index >= total_samples as usize {
                continue; // This track is exhausted
            }

            // Calculate DTS for this sample (in track's native timescale)
            let mut dts = 0i64;
            let mut accumulated_samples = 0usize;
            for entry in &stts.entries {
                if accumulated_samples + entry.sample_count as usize > sample_index {
                    dts += (sample_index - accumulated_samples) as i64 * entry.sample_delta as i64;
                    break;
                }
                dts += entry.sample_count as i64 * entry.sample_delta as i64;
                accumulated_samples += entry.sample_count as usize;
            }

            // Convert DTS to microseconds for cross-track comparison
            let timescale = trak.mdia.mdhd.timescale as i64;
            let dts_us = dts * USEC_PER_SEC / timescale;

            if dts_us < earliest_dts_us {
                earliest_dts_us = dts_us;
                earliest_dts_raw = dts;
                earliest_track_idx = Some(track_idx);
            }
        }

        let track_idx = earliest_track_idx.ok_or_else(|| not_found_error!("no more samples"))?;

        // Find the corresponding trak
        let trak = &moov.trak[track_idx];
        let track_id = trak.tkhd.track_id;

        let sample_index = self.track_sample_indices[track_idx];
        let stbl = &trak.mdia.minf.stbl;

        // Calculate sample duration from stts
        let mut duration = 0i64;
        let mut accumulated_samples = 0usize;
        for entry in &stbl.stts.entries {
            if accumulated_samples + entry.sample_count as usize > sample_index {
                duration = entry.sample_delta as i64;
                break;
            }
            accumulated_samples += entry.sample_count as usize;
        }

        // Calculate PTS offset from ctts (Composition Time to Sample)
        let pts_offset = if let Some(ref ctts) = stbl.ctts {
            let mut accumulated_samples = 0usize;
            let mut offset = 0i32;
            for entry in &ctts.entries {
                if accumulated_samples + entry.sample_count as usize > sample_index {
                    offset = entry.sample_offset;
                    break;
                }
                accumulated_samples += entry.sample_count as usize;
            }
            offset as i64
        } else {
            0i64
        };

        let sample_size = match &stbl.stsz.samples {
            StszSamples::Identical {
                size, ..
            } => *size as usize,
            StszSamples::Different {
                sizes,
            } => *sizes.get(sample_index).ok_or_else(|| not_found_error!("sample size"))? as usize,
        };

        // Get chunk and offset
        let mut chunk_index = 0usize;
        let mut sample_in_chunk = sample_index;

        for (i, entry) in stbl.stsc.entries.iter().enumerate() {
            let next_first_chunk = stbl.stsc.entries.get(i + 1).map(|e| e.first_chunk).unwrap_or(u32::MAX);

            let chunks_in_this_group = next_first_chunk - entry.first_chunk;
            let samples_per_chunk = entry.samples_per_chunk as usize;
            let samples_in_this_group = chunks_in_this_group as usize * samples_per_chunk;

            if sample_in_chunk < samples_in_this_group {
                chunk_index = (entry.first_chunk - 1) as usize + sample_in_chunk / samples_per_chunk;
                sample_in_chunk %= samples_per_chunk;
                break;
            }
            sample_in_chunk -= samples_in_this_group;
        }

        let chunk_offset = if let Some(ref stco) = stbl.stco {
            *stco.entries.get(chunk_index).ok_or_else(|| not_found_error!("chunk offset"))? as u64
        } else if let Some(ref co64) = stbl.co64 {
            *co64.entries.get(chunk_index).ok_or_else(|| not_found_error!("chunk offset"))?
        } else {
            return Err(not_found_error!("chunk offset"));
        };

        // Calculate sample offset within chunk
        let mut sample_offset = chunk_offset;
        for i in 0..sample_in_chunk {
            let prev_sample_idx = sample_index - sample_in_chunk + i;
            let prev_size = match &stbl.stsz.samples {
                StszSamples::Identical {
                    size, ..
                } => *size as u64,
                StszSamples::Different {
                    sizes,
                } => *sizes.get(prev_sample_idx).ok_or_else(|| not_found_error!("sample size"))? as u64,
            };
            sample_offset += prev_size;
        }

        let track = state.tracks.find_track(track_id as isize).ok_or_else(|| not_found_error!("track"))?;

        let mut packet = Packet::from_buffer(track.pool.get_buffer_with_length(sample_size));
        let buffer = packet.data_mut().ok_or_else(|| invalid_error!("packet buffer is not mutable"))?;

        reader.seek(SeekFrom::Start(sample_offset))?;
        reader.read_exact(buffer)?;

        let timescale = trak.mdia.mdhd.timescale;
        let time_base = Rational64::new(1, timescale as i64);

        packet.track_index = Some(track.index());
        packet.dts = Some(earliest_dts_raw);
        packet.pts = Some(earliest_dts_raw + pts_offset);
        packet.duration = Some(duration);
        packet.time_base = Some(time_base);

        // Check if this is a keyframe (sync sample)
        packet.flags = if stbl.stss.is_some() {
            let key = stbl.stss.as_ref().map(|stss| stss.entries.contains(&((sample_index + 1) as u32))).unwrap_or(false);

            if key {
                PacketFlags::Key
            } else {
                PacketFlags::empty()
            }
        } else {
            PacketFlags::Key // If no stss, all samples are keyframes
        };

        // Update sample index
        self.track_sample_indices[track_idx] = sample_index + 1;

        Ok(packet)
    }

    fn seek<R: Reader>(
        &mut self,
        _reader: &mut R,
        state: &DemuxerState,
        track_index: Option<usize>,
        timestamp_us: i64,
        flags: SeekFlags,
    ) -> Result<()> {
        let moov = self.moov.as_ref().ok_or_else(|| not_found_error!("moov"))?;

        // Determine the target track index
        let track_index = track_index.unwrap_or_else(|| {
            // Find the first video track, or fall back to the first track
            state.tracks.into_iter().find(|t| t.media_type() == MediaType::Video).map(|t| t.index()).unwrap_or(0)
        });

        let target_trak = moov.trak.get(track_index).ok_or_else(|| not_found_error!("track at index {}", track_index))?;
        let target_timescale = target_trak.mdia.mdhd.timescale;
        let target_stbl = &target_trak.mdia.minf.stbl;

        // Convert timestamp (in microseconds) to target track's timescale
        let track_target_dts = timestamp_us * target_timescale as i64 / USEC_PER_SEC;

        let mut target_sample_index = Self::find_sample_index(target_stbl, track_target_dts);

        // Apply keyframe seeking (skip if ANY flag is set)
        if !flags.contains(SeekFlags::ANY) {
            if let Some(ref stss) = target_stbl.stss {
                let target_sample_number = (target_sample_index + 1) as u32;

                let keyframe_sample = if flags.contains(SeekFlags::BACKWARD) {
                    // Find the largest sync sample that is <= target
                    match stss.entries.partition_point(|s| *s <= target_sample_number) {
                        0 => 1,
                        i => stss.entries[i - 1],
                    }
                } else {
                    // Find the nearest keyframe (before or after)
                    let pos = stss.entries.partition_point(|s| *s < target_sample_number);
                    let candidates = [pos.checked_sub(1).and_then(|i| stss.entries.get(i)), stss.entries.get(pos)];
                    candidates.into_iter().flatten().min_by_key(|s| s.abs_diff(target_sample_number)).copied().unwrap_or(1)
                };

                target_sample_index = (keyframe_sample - 1) as usize;
            }
        }
        // Keep the original target_sample_index (may be non-keyframe)

        // Calculate the actual DTS of the selected keyframe
        let mut actual_dts = 0i64;
        let mut accumulated_samples = 0usize;
        for entry in &target_stbl.stts.entries {
            if accumulated_samples + entry.sample_count as usize > target_sample_index {
                actual_dts += (target_sample_index - accumulated_samples) as i64 * entry.sample_delta as i64;
                break;
            }
            actual_dts += entry.sample_count as i64 * entry.sample_delta as i64;
            accumulated_samples += entry.sample_count as usize;
        }

        // Synchronize all tracks
        for (trak_idx, trak) in moov.trak.iter().enumerate() {
            let sample_index = if trak_idx == track_index {
                // Target track: use keyframe-aligned position
                target_sample_index
            } else {
                // Other tracks: find sample at the actual timestamp
                let timescale = trak.mdia.mdhd.timescale;
                let track_dts = actual_dts * timescale as i64 / target_timescale as i64;
                Self::find_sample_index(&trak.mdia.minf.stbl, track_dts)
            };

            self.track_sample_indices[trak_idx] = sample_index;
        }

        Ok(())
    }
}
