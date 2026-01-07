use std::{
    borrow::Cow,
    sync::{Arc, LockResult, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};
#[cfg(any(feature = "audio", feature = "video"))]
use std::{
    ops::{Index, IndexMut},
    slice::{Iter, IterMut},
};

use aligned_vec::{avec, AVec, ConstAlign};
use bytemuck::Pod;
use num_rational::Rational64;
#[cfg(any(feature = "audio", feature = "video"))]
use smallvec::SmallVec;

#[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
use crate::video::pixel_buffer::frame::PixelBuffer;
#[cfg(any(feature = "audio", feature = "video"))]
use crate::{buffer::Buffer, unsupported_error};
use crate::{frame_pool::FramePool, variant::Variant, FrameDescriptor, FrameDescriptorSpec, MediaType, Result, DEFAULT_ALIGNMENT};

#[cfg(any(feature = "audio", feature = "video"))]
const DEFAULT_MAX_PLANES: usize = 8;

#[cfg(any(feature = "audio", feature = "video"))]
pub enum MappedData<'a> {
    RefMut(&'a mut [u8]),
    Ref(&'a [u8]),
}

#[cfg(any(feature = "audio", feature = "video"))]
pub enum MappedPlane<'a> {
    #[cfg(feature = "audio")]
    Audio { data: MappedData<'a>, actual_bytes: usize },
    #[cfg(feature = "video")]
    Video { data: MappedData<'a>, stride: usize, height: u32 },
}

#[cfg(any(feature = "audio", feature = "video"))]
impl MappedPlane<'_> {
    pub fn data(&self) -> Option<&[u8]> {
        match self {
            #[cfg(feature = "audio")]
            MappedPlane::Audio {
                data, ..
            } => match data {
                MappedData::Ref(data) => Some(data),
                MappedData::RefMut(data) => Some(data),
            },
            #[cfg(feature = "video")]
            MappedPlane::Video {
                data, ..
            } => match data {
                MappedData::Ref(data) => Some(data),
                MappedData::RefMut(data) => Some(data),
            },
        }
    }

    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            #[cfg(feature = "audio")]
            MappedPlane::Audio {
                data, ..
            } => match data {
                MappedData::Ref(_) => None,
                MappedData::RefMut(data) => Some(data),
            },
            #[cfg(feature = "video")]
            MappedPlane::Video {
                data, ..
            } => match data {
                MappedData::Ref(_) => None,
                MappedData::RefMut(data) => Some(data),
            },
        }
    }

    #[cfg(feature = "video")]
    pub fn stride(&self) -> Option<usize> {
        #[allow(unreachable_patterns)]
        match self {
            MappedPlane::Video {
                stride, ..
            } => Some(*stride),
            _ => None,
        }
    }

    #[cfg(feature = "video")]
    pub fn height(&self) -> Option<u32> {
        #[allow(unreachable_patterns)]
        match self {
            MappedPlane::Video {
                height, ..
            } => Some(*height),
            _ => None,
        }
    }

    pub fn as_slice_of<T>(&self) -> Option<&[T]>
    where
        T: Pod,
    {
        bytemuck::try_cast_slice(self.data()?).ok()
    }

    pub fn as_mut_slice_of<T>(&mut self) -> Option<&mut [T]>
    where
        T: Pod,
    {
        bytemuck::try_cast_slice_mut(self.data_mut()?).ok()
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
pub(crate) enum DataRef<'a> {
    Immutable(&'a dyn DataMappable),
    Mutable(&'a mut dyn DataMappable),
}

#[cfg(any(feature = "audio", feature = "video"))]
pub struct MappedGuard<'a> {
    pub(crate) data_ref: DataRef<'a>,
}

#[cfg(any(feature = "audio", feature = "video"))]
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

#[cfg(any(feature = "audio", feature = "video"))]
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

#[cfg(any(feature = "audio", feature = "video"))]
type PlaneArray<'a> = [MappedPlane<'a>; DEFAULT_MAX_PLANES];

#[cfg(any(feature = "audio", feature = "video"))]
pub struct MappedPlanes<'a> {
    pub(crate) planes: SmallVec<PlaneArray<'a>>,
}

#[cfg(any(feature = "audio", feature = "video"))]
impl<'a> IntoIterator for MappedPlanes<'a> {
    type Item = MappedPlane<'a>;
    type IntoIter = smallvec::IntoIter<PlaneArray<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.planes.into_iter()
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
impl<'a> Index<usize> for MappedPlanes<'a> {
    type Output = MappedPlane<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.planes[index]
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
impl<'a> IndexMut<usize> for MappedPlanes<'a> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.planes[index]
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
impl<'a> MappedPlanes<'a> {
    pub fn plane_data(&self, index: usize) -> Option<&[u8]> {
        self.planes.get(index).and_then(|plane| plane.data())
    }

    pub fn plane_data_mut(&mut self, index: usize) -> Option<&mut [u8]> {
        self.planes.get_mut(index).and_then(|plane| plane.data_mut())
    }

    #[cfg(feature = "video")]
    pub fn plane_stride(&self, index: usize) -> Option<usize> {
        self.planes.get(index).and_then(|plane| plane.stride())
    }

    #[cfg(feature = "video")]
    pub fn plane_height(&self, index: usize) -> Option<u32> {
        self.planes.get(index).and_then(|plane| plane.height())
    }

    pub fn is_empty(&self) -> bool {
        self.planes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.planes.len()
    }

    pub fn iter(&self) -> Iter<'_, MappedPlane<'_>> {
        self.planes.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, MappedPlane<'a>> {
        self.planes.iter_mut()
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
pub(crate) type PlaneVec<T> = SmallVec<[T; DEFAULT_MAX_PLANES]>;

#[cfg(any(feature = "audio", feature = "video"))]
#[derive(Copy, Clone)]
pub(crate) enum PlaneDescriptor {
    #[cfg(feature = "audio")]
    Audio(usize, usize), // plane_size, actual_bytes
    #[cfg(feature = "video")]
    Video(usize, u32), // stride, height
}

#[allow(unused)]
pub(crate) enum Data<'a, T: Pod = u8> {
    Borrowed(&'a [T]),
    BorrowedMut(&'a mut [T]),
    Owned(AVec<T, ConstAlign<DEFAULT_ALIGNMENT>>),
}

#[allow(unused)]
impl<T: Pod> Data<'_, T> {
    pub(crate) fn new(len: usize, initial_value: T) -> Data<'static, T> {
        Data::Owned(avec![[DEFAULT_ALIGNMENT]| initial_value; len])
    }

    pub(crate) fn from_slice<'a>(slice: &'a [T]) -> Data<'a, T> {
        Data::Borrowed(slice)
    }

    pub(crate) fn from_slice_mut<'a>(slice: &'a mut [T]) -> Data<'a, T> {
        Data::BorrowedMut(slice)
    }

    pub(crate) fn copy_from_slice(slice: &[T]) -> Data<'static, T> {
        Data::Owned(AVec::from_slice(DEFAULT_ALIGNMENT, slice))
    }

    pub(crate) fn into_owned(self) -> Data<'static, T> {
        match self {
            Data::Borrowed(slice) => Data::Owned(AVec::from_slice(DEFAULT_ALIGNMENT, slice)),
            Data::BorrowedMut(slice) => Data::Owned(AVec::from_slice(DEFAULT_ALIGNMENT, slice)),
            Data::Owned(vec) => Data::Owned(vec),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Data::Borrowed(slice) => slice.len(),
            Data::BorrowedMut(slice) => slice.len(),
            Data::Owned(vec) => vec.len(),
        }
    }

    pub(crate) fn as_ref(&self) -> &[T] {
        match self {
            Data::Borrowed(slice) => slice,
            Data::BorrowedMut(slice) => slice,
            Data::Owned(vec) => vec.as_slice(),
        }
    }

    pub(crate) fn as_mut(&mut self) -> Option<&mut [T]> {
        match self {
            Data::Borrowed(_) => None,
            Data::BorrowedMut(slice) => Some(slice),
            Data::Owned(vec) => Some(vec.as_mut_slice()),
        }
    }

    pub(crate) fn as_ptr(&self) -> *const T {
        self.as_ref().as_ptr()
    }

    pub(crate) fn as_mut_ptr(&mut self) -> Option<*mut T> {
        self.as_mut().map(|slice| slice.as_mut_ptr())
    }
}

impl<'a, T: Pod> Clone for Data<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Data::Borrowed(slice) => Data::from_slice(slice),
            Data::BorrowedMut(slice) => Data::copy_from_slice(slice),
            Data::Owned(vec) => Data::Owned(vec.clone()),
        }
    }
}

impl<'a, T: Pod> From<Cow<'a, [T]>> for Data<'a, T> {
    fn from(cow: Cow<'a, [T]>) -> Self {
        match cow {
            Cow::Borrowed(slice) => Data::from_slice(slice),
            Cow::Owned(vec) => Data::copy_from_slice(&vec),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MemoryData<'a> {
    pub(crate) data: Data<'a>,
    #[cfg(any(feature = "audio", feature = "video"))]
    pub(crate) planes: PlaneVec<PlaneDescriptor>,
}

impl MemoryData<'_> {
    fn into_owned(self) -> MemoryData<'static> {
        MemoryData {
            data: self.data.into_owned(),
            #[cfg(any(feature = "audio", feature = "video"))]
            planes: self.planes,
        }
    }

    #[cfg(feature = "audio")]
    #[allow(unreachable_patterns)]
    pub(crate) fn truncate(&mut self, len: usize) -> Result<()> {
        for plane in &mut self.planes {
            match plane {
                PlaneDescriptor::Audio(plane_size, actual_bytes) => {
                    if len > *plane_size || len == 0 {
                        return Err(crate::invalid_param_error!(len));
                    }

                    *actual_bytes = len;
                }
                _ => return Err(unsupported_error!("truncate for non-audio plane")),
            }
        }

        Ok(())
    }
}

#[cfg(feature = "video")]
#[derive(Clone)]
pub(crate) struct SeparateMemoryData<'a> {
    pub(crate) planes: PlaneVec<(&'a [u8], usize, u32)>,
}

#[cfg(feature = "video")]
impl SeparateMemoryData<'_> {
    fn into_owned(self) -> MemoryData<'static> {
        let mut data = AVec::new(DEFAULT_ALIGNMENT);
        let mut planes = PlaneVec::<PlaneDescriptor>::new();

        for (slice, stride, height) in self.planes {
            data.extend_from_slice(slice);
            planes.push(PlaneDescriptor::Video(stride, height));
        }

        MemoryData {
            data: Data::Owned(data),
            planes,
        }
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
#[derive(Clone)]
pub(crate) struct BufferData {
    pub(crate) data: Arc<Buffer>,
    pub(crate) planes: PlaneVec<(usize, PlaneDescriptor)>,
}

#[derive(Clone)]
pub(crate) enum FrameData<'a> {
    #[allow(dead_code)]
    Memory(MemoryData<'a>),
    #[cfg(feature = "video")]
    SeparateMemory(SeparateMemoryData<'a>),
    #[cfg(any(feature = "audio", feature = "video"))]
    #[allow(dead_code)]
    Buffer(BufferData),
    #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
    PixelBuffer(PixelBuffer),
    Variant(Variant),
    #[allow(dead_code)]
    Empty,
}

impl FrameData<'_> {
    pub(crate) fn into_owned(self) -> FrameData<'static> {
        match self {
            FrameData::Memory(data) => FrameData::Memory(data.into_owned()),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(data) => FrameData::Memory(data.into_owned()),
            #[cfg(any(feature = "audio", feature = "video"))]
            FrameData::Buffer(data) => FrameData::Buffer(data),
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(pixel_buffer) => FrameData::PixelBuffer(pixel_buffer),
            FrameData::Variant(variant) => FrameData::Variant(variant),
            FrameData::Empty => FrameData::Empty,
        }
    }

    // Truncate audio frame data to the specified length
    #[cfg(feature = "audio")]
    pub(crate) fn truncate(&mut self, len: usize) -> Result<()> {
        match self {
            FrameData::Memory(data) => data.truncate(len),
            _ => Err(unsupported_error!("truncate for non-memory data")),
        }
    }
}

#[cfg(any(feature = "audio", feature = "video"))]
pub trait DataMappable: Send + Sync {
    fn map(&self) -> Result<MappedGuard<'_>>;
    fn map_mut(&mut self) -> Result<MappedGuard<'_>>;
    fn unmap(&self) -> Result<()>;
    fn unmap_mut(&mut self) -> Result<()>;
    fn planes(&self) -> Option<MappedPlanes<'_>>;
    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>>;
}

#[cfg(any(feature = "audio", feature = "video"))]
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
        let mut data_slice = self.data.as_ref();
        let mut planes = SmallVec::with_capacity(DEFAULT_MAX_PLANES);

        for plane in &self.planes {
            let plane_size = match plane {
                #[cfg(feature = "audio")]
                PlaneDescriptor::Audio(plane_size, _) => *plane_size,
                #[cfg(feature = "video")]
                PlaneDescriptor::Video(stride, height) => stride * (*height as usize),
            };

            if plane_size > data_slice.len() || plane_size == 0 {
                return None;
            }

            let (plane_data, rest) = data_slice.split_at(plane_size);

            let mapped_plane = match plane {
                #[cfg(feature = "audio")]
                PlaneDescriptor::Audio(_, actual_bytes) => MappedPlane::Audio {
                    data: MappedData::Ref(&plane_data[..*actual_bytes]),
                    actual_bytes: *actual_bytes,
                },
                #[cfg(feature = "video")]
                PlaneDescriptor::Video(stride, height) => MappedPlane::Video {
                    data: MappedData::Ref(plane_data),
                    stride: *stride,
                    height: *height,
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
        let mut data_slice = self.data.as_mut()?;
        let mut planes = SmallVec::with_capacity(DEFAULT_MAX_PLANES);

        for plane in &self.planes {
            let plane_size = match plane {
                #[cfg(feature = "audio")]
                PlaneDescriptor::Audio(plane_size, _) => *plane_size,
                #[cfg(feature = "video")]
                PlaneDescriptor::Video(stride, height) => stride * (*height as usize),
            };

            if plane_size > data_slice.len() {
                return None;
            }

            let (plane_data, rest) = data_slice.split_at_mut(plane_size);

            let mapped_plane = match plane {
                #[cfg(feature = "audio")]
                PlaneDescriptor::Audio(_, actual_bytes) => MappedPlane::Audio {
                    data: MappedData::RefMut(&mut plane_data[..*actual_bytes]),
                    actual_bytes: *actual_bytes,
                },
                #[cfg(feature = "video")]
                PlaneDescriptor::Video(stride, height) => MappedPlane::Video {
                    data: MappedData::RefMut(plane_data),
                    stride: *stride,
                    height: *height,
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

#[cfg(feature = "video")]
impl DataMappable for SeparateMemoryData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>> {
        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        Err(unsupported_error!("map"))
    }

    fn unmap(&self) -> Result<()> {
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<()> {
        Err(unsupported_error!("unmap"))
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        let mut planes = SmallVec::with_capacity(DEFAULT_MAX_PLANES);

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

#[cfg(any(feature = "audio", feature = "video"))]
impl DataMappable for BufferData {
    fn map(&self) -> Result<MappedGuard<'_>> {
        Ok(MappedGuard {
            data_ref: DataRef::Immutable(self),
        })
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        Err(unsupported_error!("map"))
    }

    fn unmap(&self) -> Result<()> {
        Ok(())
    }

    fn unmap_mut(&mut self) -> Result<()> {
        Err(unsupported_error!("unmap"))
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        let data = self.data.data();
        let mut planes = SmallVec::with_capacity(DEFAULT_MAX_PLANES);

        for plane in &self.planes {
            let (offset, plane_size) = match plane.1 {
                #[cfg(feature = "audio")]
                PlaneDescriptor::Audio(plane_size, _) => (plane.0, plane_size),
                #[cfg(feature = "video")]
                PlaneDescriptor::Video(stride, height) => (plane.0, stride * (height as usize)),
            };

            if plane_size > data.len() || plane_size == 0 {
                return None;
            }

            #[allow(unused_variables)]
            let plane_data = &data[offset..offset + plane_size];

            let mapped_plane = match plane.1 {
                #[cfg(feature = "audio")]
                PlaneDescriptor::Audio(_, actual_bytes) => MappedPlane::Audio {
                    data: MappedData::Ref(&plane_data[..actual_bytes]),
                    actual_bytes,
                },
                #[cfg(feature = "video")]
                PlaneDescriptor::Video(stride, height) => MappedPlane::Video {
                    data: MappedData::Ref(plane_data),
                    stride,
                    height,
                },
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

#[cfg(any(feature = "audio", feature = "video"))]
impl DataMappable for FrameData<'_> {
    fn map(&self) -> Result<MappedGuard<'_>> {
        match self {
            FrameData::Memory(data) => data.map(),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(data) => data.map(),
            FrameData::Buffer(data) => data.map(),
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(data) => data.map(),
            _ => Err(unsupported_error!("frame data")),
        }
    }

    fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        match self {
            FrameData::Memory(data) => data.map_mut(),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(data) => data.map_mut(),
            FrameData::Buffer(data) => data.map_mut(),
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(data) => data.map_mut(),
            _ => Err(unsupported_error!("frame data")),
        }
    }

    fn unmap(&self) -> Result<()> {
        match self {
            FrameData::Memory(data) => data.unmap(),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(data) => data.unmap(),
            FrameData::Buffer(data) => data.unmap(),
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(data) => data.unmap(),
            _ => Err(unsupported_error!("frame data")),
        }
    }

    fn unmap_mut(&mut self) -> Result<()> {
        match self {
            FrameData::Memory(data) => data.unmap_mut(),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(data) => data.unmap_mut(),
            FrameData::Buffer(data) => data.unmap_mut(),
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(data) => data.unmap_mut(),
            _ => Err(unsupported_error!("frame data")),
        }
    }

    fn planes(&self) -> Option<MappedPlanes<'_>> {
        match self {
            FrameData::Memory(data) => data.planes(),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(data) => data.planes(),
            FrameData::Buffer(data) => data.planes(),
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(data) => data.planes(),
            _ => None,
        }
    }

    fn planes_mut(&mut self) -> Option<MappedPlanes<'_>> {
        match self {
            FrameData::Memory(data) => data.planes_mut(),
            #[cfg(feature = "video")]
            FrameData::SeparateMemory(_) => None,
            FrameData::Buffer(_) => None,
            #[cfg(all(feature = "video", any(target_os = "macos", target_os = "ios")))]
            FrameData::PixelBuffer(data) => data.planes_mut(),
            _ => None,
        }
    }
}

pub trait FrameSpec<D: FrameDescriptorSpec> {
    fn new_with_descriptor(desc: D) -> Result<Frame<'static, D>>;
    fn media_type(&self) -> MediaType;
}

#[derive(Clone)]
pub struct Frame<'a, D: FrameDescriptorSpec = FrameDescriptor> {
    pub(crate) desc: D,
    pub source: Option<String>,
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub metadata: Option<Variant>,
    pub(crate) data: FrameData<'a>,
}

impl Frame<'_, FrameDescriptor> {
    pub fn new_with_generic_descriptor<D>(desc: D) -> Result<Frame<'static>>
    where
        D: Into<FrameDescriptor> + Clone,
    {
        let desc = desc.into();
        match desc {
            #[cfg(feature = "audio")]
            FrameDescriptor::Audio(audio_desc) => Self::audio_creator().create_with_descriptor(audio_desc),
            #[cfg(feature = "video")]
            FrameDescriptor::Video(video_desc) => Self::video_creator().create_with_descriptor(video_desc),
            FrameDescriptor::Data(data_desc) => Self::data_creator().create_with_descriptor(data_desc),
        }
    }

    pub(crate) fn from_data<'a>(desc: FrameDescriptor, data: FrameData<'a>) -> Frame<'a> {
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

    #[cfg(any(feature = "audio", feature = "video"))]
    pub fn convert_to(&self, dst: &mut Frame) -> Result<()> {
        match self.media_type() {
            #[cfg(feature = "audio")]
            MediaType::Audio => self.convert_audio_to(dst),
            #[cfg(feature = "video")]
            MediaType::Video => self.convert_video_to(dst),
            _ => Err(unsupported_error!("media type")),
        }
    }
}

impl<D: FrameDescriptorSpec> Frame<'_, D> {
    pub(crate) fn from_data_with_generic_descriptor<'a>(desc: D, data: FrameData<'a>) -> Frame<'a, D> {
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

    pub fn descriptor(&self) -> &D {
        &self.desc
    }

    pub fn into_owned(self) -> Frame<'static, D> {
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

    #[cfg(any(feature = "audio", feature = "video"))]
    pub fn map(&self) -> Result<MappedGuard<'_>> {
        self.data.map()
    }

    #[cfg(any(feature = "audio", feature = "video"))]
    pub fn map_mut(&mut self) -> Result<MappedGuard<'_>> {
        self.data.map_mut()
    }
}

impl FrameSpec<FrameDescriptor> for Frame<'_, FrameDescriptor> {
    fn new_with_descriptor(desc: FrameDescriptor) -> Result<Frame<'static>> {
        Frame::new_with_generic_descriptor(desc)
    }

    fn media_type(&self) -> MediaType {
        self.media_type()
    }
}

pub trait SharedFrameInner {
    type Descriptor: FrameDescriptorSpec;
}

impl<D: FrameDescriptorSpec> SharedFrameInner for RwLock<Frame<'_, D>> {
    type Descriptor = D;
}

impl<D: FrameDescriptorSpec> SharedFrameInner for Frame<'_, D> {
    type Descriptor = D;
}

#[derive(Clone)]
pub struct SharedFrame<F: SharedFrameInner = RwLock<Frame<'static>>> {
    inner: Arc<F>,
    pub(crate) pool: Option<Weak<FramePool<F>>>,
}

impl<D: FrameDescriptorSpec> SharedFrame<RwLock<Frame<'static, D>>> {
    pub fn new(frame: Frame<'_, D>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(frame.into_owned())),
            pool: None,
        }
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, Frame<'static, D>>> {
        self.inner.read()
    }

    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, Frame<'static, D>>> {
        self.inner.write()
    }
}

impl<D: FrameDescriptorSpec> SharedFrame<Frame<'static, D>> {
    pub fn new(frame: Frame<'_, D>) -> Self {
        Self {
            inner: Arc::new(frame.into_owned()),
            pool: None,
        }
    }

    pub fn read(&self) -> &Frame<'static, D> {
        &self.inner
    }

    pub fn write(&mut self) -> Option<&mut Frame<'static, D>> {
        Arc::get_mut(&mut self.inner)
    }
}

impl<F: SharedFrameInner> Drop for SharedFrame<F> {
    fn drop(&mut self) {
        if let Some(pool) = &self.pool {
            if let Some(pool) = pool.upgrade() {
                let cloned = SharedFrame {
                    inner: Arc::clone(&self.inner),
                    pool: None,
                };
                pool.recycle_frame(cloned);
            }
        }
    }
}
