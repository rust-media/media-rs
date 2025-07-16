use std::sync::{Arc, LockResult, RwLock, RwLockReadGuard, RwLockWriteGuard};

use smallvec::SmallVec;
use variant::Variant;

use super::{audio::AudioFrameDescriptor, data::DataFrameDescriptor, error::MediaError, media::MediaFrameType, video::VideoFrameDescriptor};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::pixel_buffer::video_frame::PixelBuffer;
use crate::unsupported_error;

pub const MEDIA_FRAME_MAX_PLANES: usize = 8;

#[derive(Clone, Debug)]
pub enum MediaFrameDescriptor {
    Audio(AudioFrameDescriptor),
    Video(VideoFrameDescriptor),
    Data(DataFrameDescriptor),
}

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

pub(super) enum DataRef<'a> {
    Immutable(&'a dyn DataMappable),
    Mutable(&'a mut dyn DataMappable),
}

pub struct MappedGuard<'a> {
    pub(super) data_ref: DataRef<'a>,
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
            DataRef::Mutable(_) => None,
        }
    }

    pub fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        match &mut self.data_ref {
            DataRef::Immutable(_) => None,
            DataRef::Mutable(data) => data.planes_mut(),
        }
    }
}

type PlaneArray<'a> = [MappedPlane<'a>; MEDIA_FRAME_MAX_PLANES];

pub struct MappedPlanes<'a> {
    pub(super) planes: SmallVec<PlaneArray<'a>>,
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
}

#[derive(Copy, Clone)]
pub(super) enum PlaneInformation {
    Video(u32, u32),
    Audio(u32),
}

pub(super) type PlaneInformationVec = SmallVec<[PlaneInformation; MEDIA_FRAME_MAX_PLANES]>;

#[derive(Clone)]
pub(super) enum Data<'a> {
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
pub(super) struct MemoryData<'a> {
    pub(super) data: Data<'a>,
    pub(super) planes: PlaneInformationVec,
}

impl MemoryData<'_> {
    fn into_owned(self) -> MemoryData<'static> {
        MemoryData {
            data: Data::Owned(self.data.into_owned()),
            planes: self.planes,
        }
    }
}

pub(super) type PlaneDataVec<'a> = SmallVec<[(&'a [u8], u32, u32); MEDIA_FRAME_MAX_PLANES]>;

#[derive(Clone)]
pub(super) struct SeparateMemoryData<'a> {
    pub(super) planes: PlaneDataVec<'a>,
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
pub(super) enum MediaFrameData<'a> {
    Memory(MemoryData<'a>),
    SeparateMemory(SeparateMemoryData<'a>),
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    PixelBuffer(PixelBuffer),
    Variant(Variant),
}

impl MediaFrameData<'_> {
    pub fn into_owned(self) -> MediaFrameData<'static> {
        match self {
            MediaFrameData::Memory(data) => MediaFrameData::Memory(data.into_owned()),
            MediaFrameData::SeparateMemory(data) => MediaFrameData::Memory(data.into_owned()),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(pixel_buffer) => MediaFrameData::PixelBuffer(pixel_buffer),
            MediaFrameData::Variant(variant) => MediaFrameData::Variant(variant),
        }
    }
}

pub trait DataMappable: Send + Sync {
    fn map(&self) -> Result<MappedGuard<'_>, MediaError>;
    fn map_mut(&mut self) -> Result<MappedGuard<'_>, MediaError>;
    fn unmap(&self) -> Result<(), MediaError>;
    fn unmap_mut(&mut self) -> Result<(), MediaError>;
    fn planes(&self) -> Option<MappedPlanes<'_>>;
    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>>;
}

impl DataMappable for MemoryData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>, MediaError> {
        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>, MediaError> {
        Ok(MappedGuard {
            data_ref: DataRef::Mutable(self),
        })
    }

    fn unmap(&self) -> Result<(), MediaError> {
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<(), MediaError> {
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
    fn map(&self) -> Result<MappedGuard<'_>, MediaError> {
        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>, MediaError> {
        Err(MediaError::Unsupported("map".to_string()))
    }

    fn unmap(&self) -> Result<(), MediaError> {
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported("unmap".to_string()))
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

impl DataMappable for MediaFrameData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>, MediaError> {
        match self {
            MediaFrameData::Memory(data) => data.map(),
            MediaFrameData::SeparateMemory(data) => data.map(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(data) => data.map(),
            MediaFrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>, MediaError> {
        match self {
            MediaFrameData::Memory(data) => data.map_mut(),
            MediaFrameData::SeparateMemory(data) => data.map_mut(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(data) => data.map_mut(),
            MediaFrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn unmap(&self) -> Result<(), MediaError> {
        match self {
            MediaFrameData::Memory(data) => data.unmap(),
            MediaFrameData::SeparateMemory(data) => data.unmap(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(data) => data.unmap(),
            MediaFrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn unmap_mut(&mut self) -> Result<(), MediaError> {
        match self {
            MediaFrameData::Memory(data) => data.unmap_mut(),
            MediaFrameData::SeparateMemory(data) => data.unmap_mut(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(data) => data.unmap_mut(),
            MediaFrameData::Variant(_) => Err(unsupported_error!(Variant)),
        }
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        match self {
            MediaFrameData::Memory(data) => data.planes(),
            MediaFrameData::SeparateMemory(data) => data.planes(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(data) => data.planes(),
            MediaFrameData::Variant(_) => None,
        }
    }

    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        match self {
            MediaFrameData::Memory(data) => data.planes_mut(),
            MediaFrameData::SeparateMemory(data) => None,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            MediaFrameData::PixelBuffer(data) => data.planes_mut(),
            MediaFrameData::Variant(_) => None,
        }
    }
}

#[derive(Clone)]
pub struct MediaFrame<'a> {
    pub media_type: MediaFrameType,
    pub source: Option<String>,
    pub timestamp: u64,
    pub(super) desc: MediaFrameDescriptor,
    pub metadata: Option<Variant>,
    pub(super) data: MediaFrameData<'a>,
}

impl MediaFrame<'_> {
    pub fn into_owned(self) -> MediaFrame<'static> {
        MediaFrame {
            media_type: self.media_type,
            source: self.source,
            timestamp: self.timestamp,
            desc: self.desc,
            metadata: self.metadata,
            data: self.data.into_owned(),
        }
    }

    pub fn descriptor(&self) -> &MediaFrameDescriptor {
        &self.desc
    }

    pub fn map(&self) -> Result<MappedGuard<'_>, MediaError> {
        self.data.map()
    }

    pub fn map_mut(&mut self) -> Result<MappedGuard<'_>, MediaError> {
        self.data.map_mut()
    }
}

#[derive(Clone)]
pub struct SharedMediaFrame {
    inner: Arc<RwLock<MediaFrame<'static>>>,
}

impl SharedMediaFrame {
    pub fn new(media_frame: MediaFrame<'static>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(media_frame)),
        }
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<MediaFrame<'static>>> {
        self.inner.read()
    }

    pub fn write(&self) -> LockResult<RwLockWriteGuard<MediaFrame<'static>>> {
        self.inner.write()
    }
}
