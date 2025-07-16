use variant::Variant;

use super::{
    data::{DataFormat, DataFrameDescriptor},
    media::MediaFrameType,
    media_frame::{MediaFrame, MediaFrameData, MediaFrameDescriptor},
};
use crate::error::MediaError;

pub struct DataFrameBuilder;

impl DataFrameBuilder {
    pub fn from_variant(&self, variant: &Variant) -> Result<MediaFrame<'_>, MediaError> {
        Ok(MediaFrame {
            media_type: MediaFrameType::Data,
            source: None,
            timestamp: 0,
            desc: MediaFrameDescriptor::Data(DataFrameDescriptor::new(DataFormat::Variant)),
            metadata: None,
            data: MediaFrameData::Variant(variant.clone()),
        })
    }
}

impl MediaFrame<'_> {
    pub fn data_builder() -> DataFrameBuilder {
        DataFrameBuilder
    }
}
