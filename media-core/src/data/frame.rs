use super::data::{DataFormat, DataFrameDescriptor};
use crate::{
    frame::{Frame, FrameData},
    media::FrameDescriptor,
    variant::Variant,
    Result,
};

pub struct DataFrameCreator;

impl DataFrameCreator {
    pub fn create(&self, format: DataFormat) -> Result<Frame<'static>> {
        let desc = DataFrameDescriptor::new(format);

        self.create_with_descriptor(desc)
    }

    pub fn create_with_descriptor(&self, desc: DataFrameDescriptor) -> Result<Frame<'static>> {
        Ok(Frame::from_data(FrameDescriptor::Data(desc), FrameData::Variant(Variant::new())))
    }

    pub fn create_from_variant(&self, variant: &Variant) -> Result<Frame<'static>> {
        Ok(Frame::from_data(FrameDescriptor::Data(DataFrameDescriptor::new(DataFormat::Variant)), FrameData::Variant(variant.clone())))
    }
}

impl Frame<'_> {
    pub fn data_creator() -> DataFrameCreator {
        DataFrameCreator
    }

    pub fn data_descriptor(&self) -> Option<&DataFrameDescriptor> {
        #[allow(irrefutable_let_patterns)]
        if let FrameDescriptor::Data(desc) = &self.desc {
            Some(desc)
        } else {
            None
        }
    }

    pub fn is_data(&self) -> bool {
        self.desc.is_data()
    }

    pub fn data(&self) -> Option<&Variant> {
        if let FrameData::Variant(v) = &self.data {
            Some(v)
        } else {
            None
        }
    }

    pub fn data_mut(&mut self) -> Option<&mut Variant> {
        if let FrameData::Variant(v) = &mut self.data {
            Some(v)
        } else {
            None
        }
    }
}
