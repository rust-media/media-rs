use variant::Variant;

use crate::{
    data::{DataFormat, DataFrameDescriptor},
    media::MediaFrameDescriptor,
    media_frame::{MediaFrame, MediaFrameData},
    Result,
};

pub struct DataFrameBuilder;

impl DataFrameBuilder {
    pub fn new(&self, format: DataFormat) -> Result<MediaFrame<'static>> {
        let desc = DataFrameDescriptor::new(format);

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: DataFrameDescriptor) -> Result<MediaFrame<'static>> {
        Ok(MediaFrame::default(MediaFrameDescriptor::Data(desc), MediaFrameData::Variant(Variant::new())))
    }

    pub fn from_variant(&self, variant: &Variant) -> Result<MediaFrame<'static>> {
        Ok(MediaFrame::default(MediaFrameDescriptor::Data(DataFrameDescriptor::new(DataFormat::Variant)), MediaFrameData::Variant(variant.clone())))
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
