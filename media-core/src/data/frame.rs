use super::data::{DataFormat, DataFrameDescriptor};
use crate::{
    frame::{Frame, FrameData, FrameSpec},
    invalid_error,
    variant::Variant,
    Error, FrameDescriptor, FrameDescriptorSpec, MediaType, Result,
};

pub type DataFrame<'a> = Frame<'a, DataFrameDescriptor>;

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

impl<D: FrameDescriptorSpec> Frame<'_, D> {
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
}

impl DataFrame<'_> {
    pub fn new(format: DataFormat) -> DataFrame<'static> {
        Self::new_with_descriptor(DataFrameDescriptor::new(format))
    }

    pub fn new_with_descriptor(desc: DataFrameDescriptor) -> DataFrame<'static> {
        Frame::from_data_with_generic_descriptor(desc, FrameData::Variant(Variant::new()))
    }
}

impl<'a> From<DataFrame<'a>> for Frame<'a> {
    fn from(frame: DataFrame<'a>) -> Self {
        Frame {
            desc: FrameDescriptor::Data(frame.desc),
            source: frame.source,
            pts: frame.pts,
            dts: frame.dts,
            duration: frame.duration,
            time_base: frame.time_base,
            metadata: frame.metadata,
            data: frame.data,
        }
    }
}

impl<'a> TryFrom<Frame<'a>> for DataFrame<'a> {
    type Error = Error;

    fn try_from(frame: Frame<'a>) -> Result<Self> {
        #[allow(irrefutable_let_patterns)]
        if let FrameDescriptor::Data(desc) = frame.desc {
            Ok(Frame {
                desc,
                source: frame.source,
                pts: frame.pts,
                dts: frame.dts,
                duration: frame.duration,
                time_base: frame.time_base,
                metadata: frame.metadata,
                data: frame.data,
            })
        } else {
            Err(invalid_error!("not data frame"))
        }
    }
}

impl FrameSpec<DataFrameDescriptor> for DataFrame<'_> {
    fn new_with_descriptor(desc: DataFrameDescriptor) -> Result<Frame<'static, DataFrameDescriptor>> {
        Ok(DataFrame::new_with_descriptor(desc))
    }

    fn media_type(&self) -> MediaType {
        MediaType::Data
    }
}
