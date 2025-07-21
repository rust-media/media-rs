use variant::Variant;

use super::{
    data::{DataFormat, DataFrameDescriptor},
    media::MediaFrameDescriptor,
    media_frame::{MediaFrame, MediaFrameData},
};
use crate::error::MediaError;

pub struct DataFrameBuilder;

impl DataFrameBuilder {
    pub fn new(&self, format: DataFormat) -> Result<MediaFrame<'static>, MediaError> {
        let desc = DataFrameDescriptor::new(format);

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: DataFrameDescriptor) -> Result<MediaFrame<'static>, MediaError> {
        Ok(MediaFrame {
            desc: MediaFrameDescriptor::Data(desc),
            source: None,
            timestamp: 0,
            metadata: None,
            data: MediaFrameData::Variant(Variant::new()),
        })
    }

    pub fn from_variant(&self, variant: &Variant) -> Result<MediaFrame<'static>, MediaError> {
        Ok(MediaFrame {
            desc: MediaFrameDescriptor::Data(DataFrameDescriptor::new(DataFormat::Variant)),
            source: None,
            timestamp: 0,
            metadata: None,
            data: MediaFrameData::Variant(variant.clone()),
        })
    }
}

impl MediaFrame<'_> {
    pub fn data_builder() -> DataFrameBuilder {
        DataFrameBuilder
    }

    pub fn is_data(&self) -> bool {
        self.desc.is_data()
    }
}
