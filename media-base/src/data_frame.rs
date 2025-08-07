use crate::{
    data::{DataFormat, DataFrameDescriptor},
    frame::{Frame, FrameData},
    media::FrameDescriptor,
    variant::Variant,
    Result,
};

pub struct DataFrameBuilder;

impl DataFrameBuilder {
    pub fn new(&self, format: DataFormat) -> Result<Frame<'static>> {
        let desc = DataFrameDescriptor::new(format);

        self.new_with_descriptor(desc)
    }

    pub fn new_with_descriptor(&self, desc: DataFrameDescriptor) -> Result<Frame<'static>> {
        Ok(Frame::default(FrameDescriptor::Data(desc), FrameData::Variant(Variant::new())))
    }

    pub fn from_variant(&self, variant: &Variant) -> Result<Frame<'static>> {
        Ok(Frame::default(FrameDescriptor::Data(DataFrameDescriptor::new(DataFormat::Variant)), FrameData::Variant(variant.clone())))
    }
}

impl Frame<'_> {
    pub fn data_builder() -> DataFrameBuilder {
        DataFrameBuilder
    }

    pub fn data_descriptor(&self) -> Option<&DataFrameDescriptor> {
        if let FrameDescriptor::Data(desc) = &self.desc {
            Some(desc)
        } else {
            None
        }
    }

    pub fn is_data(&self) -> bool {
        self.desc.is_data()
    }
}
