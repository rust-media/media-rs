use crate::{data::DataFrame, error::Error, frame::Frame, invalid_param_error, FrameDescriptor, FrameDescriptorSpec, MediaType, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataFormat {
    Variant = 0, // Variant
    String,      // String
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataFrameDescriptor {
    pub format: DataFormat,
}

impl DataFrameDescriptor {
    pub fn new(format: DataFormat) -> Self {
        Self {
            format,
        }
    }
}

impl From<DataFrameDescriptor> for FrameDescriptor {
    fn from(desc: DataFrameDescriptor) -> Self {
        FrameDescriptor::Data(desc)
    }
}

impl TryFrom<FrameDescriptor> for DataFrameDescriptor {
    type Error = Error;

    fn try_from(value: FrameDescriptor) -> Result<Self> {
        match value {
            FrameDescriptor::Data(desc) => Ok(desc),
            _ => Err(invalid_param_error!(value)),
        }
    }
}

impl FrameDescriptorSpec for DataFrameDescriptor {
    fn media_type(&self) -> MediaType {
        MediaType::Data
    }

    fn create_frame(&self) -> Result<Frame<'static, Self>> {
        Ok(DataFrame::new_with_descriptor(self.clone()))
    }
}
