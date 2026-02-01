use std::sync::Arc;

#[cfg(feature = "audio")]
use media_core::audio::AudioFrameDescriptor;
#[cfg(feature = "video")]
use media_core::video::VideoFrameDescriptor;
use media_core::{
    buffer::BufferPool,
    error::Error,
    frame::{Frame, SharedFrame},
    invalid_param_error,
    variant::Variant,
    MediaType, Result,
};

#[cfg(feature = "audio")]
use crate::AudioParameters;
#[cfg(feature = "video")]
use crate::VideoParameters;
use crate::{packet::Packet, Codec, CodecBuilder, CodecID, CodecParameters, CodecParametersType, CodecSpec, CodecType, MediaParametersType};

#[derive(Clone, Debug, Default)]
pub struct EncoderParameters {
    pub bit_rate: Option<u64>,
    pub profile: Option<i32>,
    pub level: Option<i32>,
    pub use_pool: Option<bool>,
}

impl EncoderParameters {
    fn update(&mut self, other: &EncoderParameters) {
        if other.bit_rate.is_some() {
            self.bit_rate = other.bit_rate;
        }
        if other.profile.is_some() {
            self.profile = other.profile;
        }
        if other.level.is_some() {
            self.level = other.level;
        }
    }

    fn update_with_option(&mut self, key: &str, value: &Variant) {
        match key {
            "bit_rate" => self.bit_rate = value.get_uint64(),
            "profile" => self.profile = value.get_int32(),
            "level" => self.level = value.get_int32(),
            _ => {}
        }
    }
}

impl TryFrom<&CodecParametersType> for EncoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParametersType) -> Result<Self> {
        #[allow(unreachable_patterns)]
        match params {
            CodecParametersType::Encoder(params) => Ok(params.clone()),
            _ => Err(invalid_param_error!(params)),
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioEncoderParameters {
    pub audio: AudioParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "audio")]
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for AudioEncoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: match &params.media {
                MediaParametersType::Audio(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            encoder: match &params.codec {
                CodecParametersType::Encoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
pub struct AudioEncoder {
    pub audio: AudioParameters,
    pub encoder: EncoderParameters,
    // audio encoder specific configuration
    pub frame_size: Option<u32>,
    pub delay: Option<u32>,
}

#[cfg(feature = "audio")]
impl CodecSpec for AudioEncoder {
    type FrameDescriptor = AudioFrameDescriptor;

    fn media_type() -> MediaType {
        MediaType::Audio
    }

    fn codec_type() -> CodecType {
        CodecType::Encoder
    }

    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: (&params.media).try_into()?,
            encoder: (&params.codec).try_into()?,
            frame_size: None,
            delay: None,
        })
    }

    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let audio_params = (&params.media).try_into()?;
        let encoder_params = (&params.codec).try_into()?;
        self.audio.update(&audio_params);
        self.encoder.update(&encoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.audio.update_with_option(key, value);
        self.encoder.update_with_option(key, value);

        match key {
            "frame_size" => self.frame_size = value.get_uint32(),
            "delay" => self.delay = value.get_uint32(),
            _ => {}
        }

        Ok(())
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug, Default)]
pub struct VideoEncoderParameters {
    pub video: VideoParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "video")]
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for VideoEncoderParameters {
    type Error = Error;

    #[allow(unreachable_patterns)]
    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            encoder: match &params.codec {
                CodecParametersType::Encoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug)]
pub struct VideoEncoder {
    pub video: VideoParameters,
    pub encoder: EncoderParameters,
}

#[cfg(feature = "video")]
impl CodecSpec for VideoEncoder {
    type FrameDescriptor = VideoFrameDescriptor;

    fn media_type() -> MediaType {
        MediaType::Video
    }

    fn codec_type() -> CodecType {
        CodecType::Encoder
    }

    #[allow(unreachable_patterns)]
    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            encoder: match &params.codec {
                CodecParametersType::Encoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }

    #[allow(unreachable_patterns)]
    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let video_params = (&params.media).try_into()?;
        let encoder_params = (&params.codec).try_into()?;
        self.video.update(&video_params);
        self.encoder.update(&encoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.video.update_with_option(key, value);
        self.encoder.update_with_option(key, value);
        Ok(())
    }
}

pub trait Encoder<T: CodecSpec>: Codec<T> + Send + Sync {
    fn init(&mut self, _config: &T) -> Result<()> {
        Ok(())
    }
    fn send_frame(&mut self, config: &T, pool: Option<&Arc<BufferPool>>, frame: SharedFrame<Frame<'static, T::FrameDescriptor>>) -> Result<()>;
    fn receive_packet(&mut self, config: &T, pool: Option<&Arc<BufferPool>>) -> Result<Packet<'static>>;
    fn flush(&mut self, config: &T) -> Result<()>;
}

pub trait EncoderBuilder<T: CodecSpec>: CodecBuilder<T> {
    fn new_encoder(&self, id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Box<dyn Encoder<T>>>;
}
