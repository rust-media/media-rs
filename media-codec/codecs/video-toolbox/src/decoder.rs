use std::{
    collections::VecDeque,
    sync::{Arc, LazyLock, Mutex},
};

use core_media::{
    block_buffer::CMBlockBuffer,
    format_description::{
        kCMVideoCodecType_AV1, kCMVideoCodecType_H264, kCMVideoCodecType_HEVC, kCMVideoCodecType_VP9, CMVideoFormatDescription, TCMFormatDescription,
    },
    sample_buffer::{CMSampleBuffer, CMSampleTimingInfo},
    time::CMTime,
};
use core_video::pixel_buffer::CVPixelBuffer;
use media_codec_types::{
    codec::{Codec, CodecID},
    decoder::{Decoder, DecoderBuilder, ExtraData, VideoDecoder, VideoDecoderParameters},
    packet::Packet,
    CodecBuilder, CodecInformation, CodecParameters,
};
use media_core::{
    error::Error, failed_error, frame::SharedFrame, frame_pool::FramePool, not_found_error, rational::Rational64, unsupported_error,
    variant::Variant, video::VideoFrame, Result,
};
use os_ver::if_greater_than;
use video_toolbox::{decompression_session::VTDecompressionSession, errors::VTDecodeFrameFlags};

const CODEC_NAME: &str = "video-toolbox";

pub struct VTDecoder {
    id: CodecID,
    session: VTDecompressionSession,
    format_desc: CMVideoFormatDescription,
    output_queue: Arc<Mutex<VecDeque<SharedFrame<VideoFrame<'static>>>>>,
}

unsafe impl Send for VTDecoder {}
unsafe impl Sync for VTDecoder {}

impl CodecInformation for VTDecoder {
    fn id(&self) -> CodecID {
        self.id
    }

    fn name(&self) -> &'static str {
        CODEC_NAME
    }
}

impl Codec<VideoDecoder> for VTDecoder {
    fn configure(&mut self, _params: Option<&CodecParameters>, _options: Option<&Variant>) -> Result<()> {
        Ok(())
    }

    fn set_option(&mut self, _key: &str, _value: &Variant) -> Result<()> {
        Ok(())
    }
}

impl Decoder<VideoDecoder> for VTDecoder {
    fn send_packet(&mut self, _config: &VideoDecoder, _pool: Option<&Arc<FramePool<VideoFrame<'static>>>>, packet: &Packet) -> Result<()> {
        let data = packet.data();
        if data.is_empty() {
            return Ok(());
        }

        let block_buffer = unsafe {
            CMBlockBuffer::new_with_memory_block_from_slice(data, 0, data.len(), 0).map_err(|_| Error::CreationFailed("CMBlockBuffer".into()))?
        };

        block_buffer.replace_data_bytes(data, 0).map_err(|err| failed_error!(err))?;

        let pts = packet.pts.unwrap_or(0);
        let dts = packet.dts.unwrap_or(pts);
        let duration = packet.duration.unwrap_or(0);
        let time_base = packet.time_base.unwrap_or(Rational64::new(1, 1_000_000));

        let timing_info = CMSampleTimingInfo {
            duration: CMTime::make(duration * time_base.numer(), *time_base.denom() as i32),
            presentationTimeStamp: CMTime::make(pts * time_base.numer(), *time_base.denom() as i32),
            decodeTimeStamp: CMTime::make(dts * time_base.numer(), *time_base.denom() as i32),
        };

        let format_desc = &self.format_desc.as_format_description();
        let sample_buffer = unsafe {
            CMSampleBuffer::new(Some(&block_buffer), true, None, None, Some(format_desc), 1, Some(&[timing_info]), Some(&[data.len()]))
                .map_err(|_| Error::CreationFailed("CMSampleBuffer".into()))?
        };

        let queue = Arc::clone(&self.output_queue);
        self.session
            .decode_frame_with_closure(sample_buffer, VTDecodeFrameFlags::empty(), move |status, _flags, image_buffer, pts, duration| {
                if status == 0 {
                    if let Some(pixel_buffer) = image_buffer.downcast::<CVPixelBuffer>() {
                        if let Ok(mut video_frame) = VideoFrame::from_pixel_buffer(&pixel_buffer) {
                            video_frame.pts = Some(pts.value);
                            video_frame.dts = None;
                            video_frame.duration = Some(duration.value);

                            let shared_frame: SharedFrame<VideoFrame<'static>> = SharedFrame::<VideoFrame<'static>>::new(video_frame);
                            if let Ok(mut queue) = queue.lock() {
                                queue.push_back(shared_frame);
                            }
                        }
                    }
                }
            })
            .map_err(|err| failed_error!("decode frame", err))?;

        Ok(())
    }

    fn receive_frame(
        &mut self,
        _config: &VideoDecoder,
        _pool: Option<&Arc<FramePool<VideoFrame<'static>>>>,
    ) -> Result<SharedFrame<VideoFrame<'static>>> {
        let mut queue = self.output_queue.lock().map_err(|err| failed_error!(err))?;

        if let Some(frame) = queue.pop_front() {
            Ok(frame)
        } else {
            Err(Error::Again("no frame available".into()))
        }
    }

    fn flush(&mut self, _config: &VideoDecoder) -> Result<()> {
        // Wait for all async frames
        self.session.wait_for_asynchronous_frames().map_err(|err| failed_error!("wait for frames", err))?;

        // Clear output queue
        if let Ok(mut queue) = self.output_queue.lock() {
            queue.clear();
        }

        Ok(())
    }
}

impl VTDecoder {
    pub fn new(id: CodecID, params: &VideoDecoderParameters, _options: Option<&Variant>) -> Result<Self> {
        let format_desc = Self::create_format_description(id, params)?;
        let output_queue = Arc::new(Mutex::new(VecDeque::new()));

        let session =
            VTDecompressionSession::new(format_desc.clone(), None, None).map_err(|_| Error::CreationFailed("VTDecompressionSession".into()))?;

        Ok(Self {
            id,
            session,
            format_desc,
            output_queue,
        })
    }

    fn codec_id_to_type(id: CodecID) -> Result<u32> {
        match id {
            CodecID::H264 => Ok(kCMVideoCodecType_H264),
            CodecID::HEVC => Ok(kCMVideoCodecType_HEVC),
            CodecID::VP9 => Ok(kCMVideoCodecType_VP9),
            CodecID::AV1 => Ok(kCMVideoCodecType_AV1),
            _ => Err(unsupported_error!("codec", id)),
        }
    }

    fn get_nalu_length_size(extra_data: &Option<ExtraData>) -> u8 {
        match extra_data {
            Some(ExtraData::AVC {
                nalu_length_size, ..
            }) => *nalu_length_size,
            Some(ExtraData::HEVC {
                nalu_length_size, ..
            }) => *nalu_length_size,
            _ => 0,
        }
    }

    fn create_format_description(id: CodecID, params: &VideoDecoderParameters) -> Result<CMVideoFormatDescription> {
        let codec_type = Self::codec_id_to_type(id)?;
        let width = params.video.width.ok_or_else(|| not_found_error!("width"))?;
        let height = params.video.height.ok_or_else(|| not_found_error!("height"))?;

        match &params.decoder.extra_data {
            Some(ExtraData::AVC {
                sps,
                pps,
                ..
            }) => {
                // Get first SPS and PPS
                if sps.is_empty() || pps.is_empty() {
                    return Err(not_found_error!("SPS or PPS"));
                }
                let sps_slice: &[u8] = &sps[0];
                let pps_slice: &[u8] = &pps[0];
                CMVideoFormatDescription::from_h264_parameter_sets(
                    &[sps_slice, pps_slice],
                    Self::get_nalu_length_size(&params.decoder.extra_data) as i32,
                )
                .map_err(|_| Error::CreationFailed("CMVideoFormatDescription".into()))
            }
            Some(ExtraData::HEVC {
                vps,
                sps,
                pps,
                ..
            }) => {
                // Get first SPS and PPS
                if sps.is_empty() || pps.is_empty() {
                    return Err(not_found_error!("SPS or PPS"));
                }
                let mut parameter_sets: Vec<&[u8]> = Vec::new();
                if let Some(vps_vec) = vps {
                    if !vps_vec.is_empty() {
                        parameter_sets.push(&vps_vec[0]);
                    }
                }
                parameter_sets.push(&sps[0]);
                parameter_sets.push(&pps[0]);

                CMVideoFormatDescription::from_hevc_parameter_sets(
                    &parameter_sets,
                    Self::get_nalu_length_size(&params.decoder.extra_data) as i32,
                    None,
                )
                .map_err(|_| Error::CreationFailed("CMVideoFormatDescription".into()))
            }
            _ => CMVideoFormatDescription::new(codec_type, width.get() as i32, height.get() as i32, None)
                .map_err(|_| Error::CreationFailed("CMVideoFormatDescription".into())),
        }
    }
}

pub struct VTDecoderBuilder;

impl DecoderBuilder<VideoDecoder> for VTDecoderBuilder {
    fn new_decoder(&self, codec_id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Box<dyn Decoder<VideoDecoder>>> {
        Ok(Box::new(VTDecoder::new(codec_id, &params.try_into()?, options)?))
    }
}

impl CodecBuilder<VideoDecoder> for VTDecoderBuilder {
    fn ids(&self) -> &'static [CodecID] {
        &SUPPORTED_CODEC_IDS
    }

    fn name(&self) -> &'static str {
        CODEC_NAME
    }
}

static SUPPORTED_CODEC_IDS: LazyLock<Vec<CodecID>> = LazyLock::new(|| {
    let mut ids = Vec::new();
    if VTDecompressionSession::is_hardware_decode_supported(kCMVideoCodecType_H264) {
        ids.push(CodecID::H264);
    }
    if_greater_than! {(10, 13) => {
        if VTDecompressionSession::is_hardware_decode_supported(kCMVideoCodecType_HEVC) {
            ids.push(CodecID::HEVC);
        }
    }}
    if_greater_than! {(14, 0) => {
        if VTDecompressionSession::is_hardware_decode_supported(kCMVideoCodecType_VP9) {
            ids.push(CodecID::VP9);
        }
        if VTDecompressionSession::is_hardware_decode_supported(kCMVideoCodecType_AV1) {
            ids.push(CodecID::AV1);
        }
    }}
    ids
});
