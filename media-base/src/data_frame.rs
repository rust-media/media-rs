use variant::Variant;

use super::{
    data::{DataFormat, DataFrameDescription},
    media::MediaFrameType,
    media_frame::{MediaFrame, MediaFrameData, MediaFrameDescription},
};

impl MediaFrame<'_> {
    pub fn from_variant(variant: &Variant) -> Self {
        Self {
            media_type: MediaFrameType::Data,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescription::Data(DataFrameDescription::new(DataFormat::Variant)),
            metadata: None,
            data: MediaFrameData::Variant(variant.clone()),
        }
    }
}
