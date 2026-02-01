use std::sync::Arc;

#[cfg(feature = "audio")]
use media_core::audio::AudioFrameDescriptor;
#[cfg(feature = "video")]
use media_core::video::VideoFrameDescriptor;
use media_core::{
    error::Error,
    frame::{Frame, SharedFrame},
    frame_pool::FramePool,
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
pub struct DecoderParameters {
    pub extra_data: Option<Vec<u8>>,
    pub use_pool: Option<bool>,
}

impl DecoderParameters {
    fn update(&mut self, other: &DecoderParameters) {
        if let Some(ref extra_data) = other.extra_data {
            self.extra_data = Some(extra_data.clone());
        }
    }

    fn update_with_option(&mut self, key: &str, value: &Variant) {
        #[allow(clippy::single_match)]
        match key {
            "extra_data" => self.extra_data = value.get_buffer(),
            "use_pool" => self.use_pool = value.get_bool(),
            _ => {}
        }
    }
}

impl TryFrom<&CodecParametersType> for DecoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParametersType) -> Result<Self> {
        #[allow(unreachable_patterns)]
        match params {
            CodecParametersType::Decoder(params) => Ok(params.clone()),
            _ => Err(invalid_param_error!(params)),
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default)]
pub struct AudioDecoderParameters {
    pub audio: AudioParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "audio")]
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for AudioDecoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: match &params.media {
                MediaParametersType::Audio(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            decoder: match &params.codec {
                CodecParametersType::Decoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug)]
pub struct AudioDecoder {
    pub audio: AudioParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "audio")]
impl CodecSpec for AudioDecoder {
    type FrameDescriptor = AudioFrameDescriptor;

    fn media_type() -> MediaType {
        MediaType::Audio
    }

    fn codec_type() -> CodecType {
        CodecType::Decoder
    }

    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            audio: (&params.media).try_into()?,
            decoder: (&params.codec).try_into()?,
        })
    }

    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let audio_params = (&params.media).try_into()?;
        let decoder_params = (&params.codec).try_into()?;
        self.audio.update(&audio_params);
        self.decoder.update(&decoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.audio.update_with_option(key, value);
        self.decoder.update_with_option(key, value);
        Ok(())
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug, Default)]
pub struct VideoDecoderParameters {
    pub video: VideoParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "video")]
#[allow(unreachable_patterns)]
impl TryFrom<&CodecParameters> for VideoDecoderParameters {
    type Error = Error;

    fn try_from(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            decoder: match &params.codec {
                CodecParametersType::Decoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }
}

#[cfg(feature = "video")]
#[derive(Clone, Debug)]
pub struct VideoDecoder {
    pub video: VideoParameters,
    pub decoder: DecoderParameters,
}

#[cfg(feature = "video")]
impl CodecSpec for VideoDecoder {
    type FrameDescriptor = VideoFrameDescriptor;

    fn media_type() -> MediaType {
        MediaType::Video
    }

    fn codec_type() -> CodecType {
        CodecType::Decoder
    }

    #[allow(unreachable_patterns)]
    fn from_parameters(params: &CodecParameters) -> Result<Self> {
        Ok(Self {
            video: match &params.media {
                MediaParametersType::Video(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
            decoder: match &params.codec {
                CodecParametersType::Decoder(params) => params.clone(),
                _ => return Err(invalid_param_error!(params)),
            },
        })
    }

    #[allow(unreachable_patterns)]
    fn configure(&mut self, params: &CodecParameters) -> Result<()> {
        let video_params = (&params.media).try_into()?;
        let decoder_params = (&params.codec).try_into()?;
        self.video.update(&video_params);
        self.decoder.update(&decoder_params);
        Ok(())
    }

    fn configure_with_option(&mut self, key: &str, value: &Variant) -> Result<()> {
        self.video.update_with_option(key, value);
        self.decoder.update_with_option(key, value);
        Ok(())
    }
}

pub trait Decoder<T: CodecSpec>: Codec<T> + Send + Sync {
    fn init(&mut self, _config: &T) -> Result<()> {
        Ok(())
    }
    fn send_packet(&mut self, config: &T, pool: Option<&Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>, packet: &Packet) -> Result<()>;
    fn receive_frame(
        &mut self,
        config: &T,
        pool: Option<&Arc<FramePool<Frame<'static, T::FrameDescriptor>>>>,
    ) -> Result<SharedFrame<Frame<'static, T::FrameDescriptor>>>;
    fn flush(&mut self, config: &T) -> Result<()>;
}

pub trait DecoderBuilder<T: CodecSpec>: CodecBuilder<T> {
    fn new_decoder(&self, id: CodecID, params: &CodecParameters, options: Option<&Variant>) -> Result<Box<dyn Decoder<T>>>;
}
