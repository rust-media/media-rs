use crate::media::MediaFrameDescriptor;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DataFormat {
    Variant = 0, // Variant
    String,      // String
}

#[derive(Clone, Debug)]
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

impl From<DataFrameDescriptor> for MediaFrameDescriptor {
    fn from(desc: DataFrameDescriptor) -> Self {
        MediaFrameDescriptor::Data(desc)
    }
}
