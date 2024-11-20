#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DataFormat {
    Variant = 0, // Variant
    String,      // String
}

#[derive(Clone, Debug)]
pub struct DataFrameDescription {
    pub format: DataFormat,
}

impl DataFrameDescription {
    pub fn new(format: DataFormat) -> Self {
        Self {
            format,
        }
    }
}
