use crate::{
    data::{DataFormat, DataFrameDescription},
    media::MediaFrameType,
    media_frame::{MediaFrame, MediaFrameData, MediaFrameDescription},
    variant::Variant,
};

impl<'a> MediaFrame<'a> {
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
