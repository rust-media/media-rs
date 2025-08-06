use std::sync::{Arc, LockResult, RwLock, RwLockReadGuard, RwLockWriteGuard};

use num_rational::Rational64;
use smallvec::SmallVec;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::pixel_buffer::video_frame::PixelBuffer;
use crate::{
    error::Error,
    media::{FrameDescriptor, MediaType},
    unsupported_error,
    variant::Variant,
    Result,
};

const DEFAULT_MAX_PLANES: usize = 8;

pub enum MappedData<'a> {
    RefMut(&'a mut [u8]),
    Ref(&'a [u8]),
}

#[derive(Default)]
pub enum MappedPlane<'a> {
    Video {
        data: MappedData<'a>,
        stride: u32,
        height: u32,
    },
    Audio {
        data: MappedData<'a>,
    },
    #[default]
    None,
}

impl MappedPlane<'_> {
    pub fn data(&self) -> Option<&[u8]> {
        match self {
            MappedPlane::Video {
                data, ..
            } => match data {
                MappedData::Ref(data) => Some(data),
                MappedData::RefMut(data) => Some(data),
            },
            MappedPlane::Audio {
                data,
            } => match data {
                MappedData::Ref(data) => Some(data),
                MappedData::RefMut(data) => Some(data),
            },
            MappedPlane::None => None,
        }
    }

    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            MappedPlane::Video {
                data, ..
            } => match data {
                MappedData::Ref(_) => None,
                MappedData::RefMut(data) => Some(data),
            },
            MappedPlane::Audio {
                data,
            } => match data {
                MappedData::Ref(_) => None,
                MappedData::RefMut(data) => Some(data),
            },
            MappedPlane::None => None,
        }
    }

    pub fn stride(&self) -> Option<u32> {
        match self {
            MappedPlane::Video {
                stride, ..
            } => Some(*stride),
            _ => None,
        }
    }

    pub fn height(&self) -> Option<u32> {
        match self {
            MappedPlane::Video {
                height, ..
            } => Some(*height),
            _ => None,
        }
    }
}

pub(crate) enum DataRef<'a> {
    Immutable(&'a dyn DataMappable),
    Mutable(&'a mut dyn DataMappable),
}

pub struct MappedGuard<'a> {
    pub(crate) data_ref: DataRef<'a>,
}

impl Drop for MappedGuard<'_> {
    fn drop(&mut self) {
        match &mut self.data_ref {
            DataRef::Immutable(data) => {
                data.unmap().ok();
            }
            DataRef::Mutable(data) => {
                data.unmap_mut().ok();
            }
        }
    }
}

impl MappedGuard<'_> {
    pub fn planes(&self) -> Option<MappedPlanes<'_>> {
        match &self.data_ref {
            DataRef::Immutable(data) => data.planes(),
            DataRef::Mutable(data) => data.planes(),
        }
    }

    pub fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        match &mut self.data_ref {
            DataRef::Immutable(_) => None,
            DataRef::Mutable(data) => data.planes_mut(),
        }
    }
}

type PlaneArray<'a> = [MappedPlane<'a>; DEFAULT_MAX_PLANES];

pub struct MappedPlanes<'a> {
    pub(crate) planes: SmallVec<PlaneArray<'a>>,
}

impl<'a> IntoIterator for MappedPlanes<'a> {
    type Item = MappedPlane<'a>;
    type IntoIter = smallvec::IntoIter<PlaneArray<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.planes.into_iter()
    }
}

impl MappedPlanes<'_> {
    pub fn plane_data(&self, index: usize) -> Option<&[u8]> {
        self.planes.get(index).and_then(|plane| plane.data())
    }

    pub fn plane_data_mut(&mut self, index: usize) -> Option<&mut [u8]> {
        self.planes.get_mut(index).and_then(|plane| plane.data_mut())
    }

    pub fn plane_stride(&self, index: usize) -> Option<u32> {
        self.planes.get(index).and_then(|plane| plane.stride())
    }

    pub fn plane_height(&self, index: usize) -> Option<u32> {
        self.planes.get(index).and_then(|plane| plane.height())
    }

    pub fn len(&self) -> usize {
        self.planes.len()
    }
}

#[derive(Copy, Clone)]
pub(crate) enum PlaneInformation {
    Video(u32, u32),
    Audio(u32),
}

pub(crate) type PlaneInformationVec = SmallVec<[PlaneInformation; DEFAULT_MAX_PLANES]>;

#[derive(Clone)]
pub(crate) enum Data<'a> {
    Owned(Vec<u8>),
    Borrowed(&'a [u8]),
}

impl Data<'_> {
    fn as_slice(&self) -> &[u8] {
        match self {
            Data::Owned(ref vec) => vec.as_slice(),
            Data::Borrowed(slice) => slice,
        }
    }

    fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        match self {
            Data::Owned(ref mut vec) => Some(vec.as_mut_slice()),
            Data::Borrowed(_) => None,
        }
    }
}

impl Data<'_> {
    fn into_owned(self) -> Vec<u8> {
        match self {
            Data::Owned(data) => data,
            Data::Borrowed(data) => data.to_vec(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MemoryData<'a> {
    pub(crate) data: Data<'a>,
    pub(crate) planes: PlaneInformationVec,
}

impl MemoryData<'_> {
    fn into_owned(self) -> MemoryData<'static> {
        MemoryData {
            data: Data::Owned(self.data.into_owned()),
            planes: self.planes,
        }
    }
}

pub(crate) type PlaneDataVec<'a> = SmallVec<[(&'a [u8], u32, u32); DEFAULT_MAX_PLANES]>;

#[derive(Clone)]
pub(crate) struct SeparateMemoryData<'a> {
    pub(crate) planes: PlaneDataVec<'a>,
}

impl SeparateMemoryData<'_> {
    fn into_owned(self) -> MemoryData<'static> {
        let mut data = Vec::new();
        let mut planes = PlaneInformationVec::new();

        for (slice, stride, height) in self.planes {
            data.extend_from_slice(slice);
            planes.push(PlaneInformation::Video(stride, height));
        }

        MemoryData {
            data: Data::Owned(data),
            planes,
        }
    }
}

#[derive(Clone)]
pub(crate) enum FrameData<'a> {
    Memory(MemoryData<'a>),
    SeparateMemory(SeparateMemoryData<'a>),
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    PixelBuffer(PixelBuffer),
    Variant(Variant),
}

impl FrameData<'_> {
    pub fn into_owned(self) -> FrameData<'static> {
        match self {
            FrameData::Memory(data) => FrameData::Memory(data.into_owned()),
            FrameData::SeparateMemory(data) => FrameData::Memory(data.into_owned()),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(pixel_buffer) => FrameData::PixelBuffer(pixel_buffer),
            FrameData::Variant(variant) => FrameData::Variant(variant),
        }
    }
}

pub trait DataMappable: Send + Sync {
    fn map(&self) -> Result<MappedGuard<'_>>;
    fn map_mut(&mut self) -> Result<MappedGuard<'_>>;
    fn unmap(&self) -> Result<()>;
    fn unmap_mut(&mut self) -> Result<()>;
    fn planes(&self) -> Option<MappedPlanes<'_>>;
    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>>;
}

impl DataMappable for MemoryData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>> {
        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        Ok(MappedGuard {
            data_ref: DataRef::Mutable(self),
        })
    }

    fn unmap(&self) -> Result<()> {
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<()> {
        Ok(())
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        let mut data_slice = self.data.as_slice();
        let mut planes = SmallVec::new();

        for plane in &self.planes {
            let plane_size = match plane {
                PlaneInformation::Video(stride, height) => stride * height,
                PlaneInformation::Audio(stride) => *stride,
            } as usize;

            if plane_size > data_slice.len() {
                return None;
            }

            let (plane_data, rest) = data_slice.split_at(plane_size);
            let mapped_plane = match plane {
                PlaneInformation::Video(stride, height) => MappedPlane::Video {
                    data: MappedData::Ref(plane_data),
                    stride: *stride,
                    height: *height,
                },
                PlaneInformation::Audio(_) => MappedPlane::Audio {
                    data: MappedData::Ref(plane_data),
                },
            };

            planes.push(mapped_plane);
            data_slice = rest;
        }

        Some(MappedPlanes {
            planes,
        })
    }

    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        let mut data_slice = self.data.as_mut_slice()?;
        let mut planes = SmallVec::new();

        for plane in &self.planes {
            let plane_size = match plane {
                PlaneInformation::Video(stride, height) => stride * height,
                PlaneInformation::Audio(stride) => *stride,
            } as usize;

            if plane_size > data_slice.len() {
                return None;
            }

            let (plane_data, rest) = data_slice.split_at_mut(plane_size);
            let mapped_plane = match plane {
                PlaneInformation::Video(stride, height) => MappedPlane::Video {
                    data: MappedData::RefMut(plane_data),
                    stride: *stride,
                    height: *height,
                },
                PlaneInformation::Audio(_) => MappedPlane::Audio {
                    data: MappedData::RefMut(plane_data),
                },
            };

            planes.push(mapped_plane);
            data_slice = rest;
        }

        Some(MappedPlanes {
            planes,
        })
    }
}

impl DataMappable for SeparateMemoryData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>> {
        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        Err(Error::Unsupported("map".to_string()))
    }

    fn unmap(&self) -> Result<()> {
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<()> {
        Err(Error::Unsupported("unmap".to_string()))
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        let mut planes = SmallVec::new();

        for (slice, stride, height) in &self.planes {
            let mapped_plane = MappedPlane::Video {
                data: MappedData::Ref(slice),
                stride: *stride,
                height: *height,
            };
            planes.push(mapped_plane);
        }

        Some(MappedPlanes {
            planes,
        })
    }

    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        None
    }
}

impl DataMappable for FrameData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>> {
        match self {
            FrameData::Memory(data) => data.map(),
            FrameData::SeparateMemory(data) => data.map(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(data) => data.map(),
            FrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        match self {
            FrameData::Memory(data) => data.map_mut(),
            FrameData::SeparateMemory(data) => data.map_mut(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(data) => data.map_mut(),
            FrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn unmap(&self) -> Result<()> {
        match self {
            FrameData::Memory(data) => data.unmap(),
            FrameData::SeparateMemory(data) => data.unmap(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(data) => data.unmap(),
            FrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn unmap_mut(&mut self) -> Result<()> {
        match self {
            FrameData::Memory(data) => data.unmap_mut(),
            FrameData::SeparateMemory(data) => data.unmap_mut(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(data) => data.unmap_mut(),
            FrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        match self {
            FrameData::Memory(data) => data.planes(),
            FrameData::SeparateMemory(data) => data.planes(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(data) => data.planes(),
            FrameData::Variant(_) => None,
        }
    }

    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        match self {
            FrameData::Memory(data) => data.planes_mut(),
            FrameData::SeparateMemory(_) => None,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            FrameData::PixelBuffer(data) => data.planes_mut(),
            FrameData::Variant(_) => None,
        }
    }
}

#[derive(Clone)]
pub struct Frame<'a> {
    pub(crate) desc: FrameDescriptor,
    pub source: Option<String>,
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub metadata: Option<Variant>,
    pub(crate) data: FrameData<'a>,
}

#[deprecated = "Use 'Frame' directly"]
pub type MediaFrame<'a> = Frame<'a>;

impl Frame<'_> {
    pub fn new_with_descriptor<T>(desc: T) -> Result<Frame<'static>>
    where
        T: Into<FrameDescriptor> + Clone,
    {
        let desc = desc.into();
        match desc {
            FrameDescriptor::Audio(audio_desc) => Self::audio_builder().new_with_descriptor(audio_desc),
            FrameDescriptor::Video(video_desc) => Self::video_builder().new_with_descriptor(video_desc),
            FrameDescriptor::Data(data_desc) => Self::data_builder().new_with_descriptor(data_desc),
        }
    }

    pub(crate) fn default<'a>(desc: FrameDescriptor, data: FrameData<'a>) -> Frame<'a> {
        Frame {
            desc,
            source: None,
            pts: None,
            dts: None,
            duration: None,
            time_base: None,
            metadata: None,
            data,
        }
    }

    pub fn media_type(&self) -> MediaType {
        self.desc.media_type()
    }

    pub fn into_owned(self) -> Frame<'static> {
        Frame {
            desc: self.desc,
            source: self.source,
            pts: self.pts,
            dts: self.dts,
            duration: self.duration,
            time_base: self.time_base,
            metadata: self.metadata,
            data: self.data.into_owned(),
        }
    }

    pub fn to_owned(&self) -> Frame<'static> {
        self.clone().into_owned()
    }

    pub fn descriptor(&self) -> &FrameDescriptor {
        &self.desc
    }

    pub fn map(&self) -> Result<MappedGuard<'_>> {
        self.data.map()
    }

    pub fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        self.data.map_mut()
    }
}

#[derive(Clone)]
pub struct SharedFrame {
    inner: Arc<RwLock<Frame<'static>>>,
}

impl SharedFrame {
    pub fn new(frame: Frame<'static>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(frame)),
        }
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<Frame<'static>>> {
        self.inner.read()
    }

    pub fn write(&self) -> LockResult<RwLockWriteGuard<Frame<'static>>> {
        self.inner.write()
    }
}
