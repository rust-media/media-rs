use std::slice;

use bytemuck::{self, Pod};
use smallvec::SmallVec;

use crate::{
    audio::{SampleFormat, DEFAULT_MAX_CHANNELS},
    frame::Data,
    invalid_param_error, Result,
};

pub(crate) struct AudioData<'a, T: Pod = u8> {
    planes: SmallVec<[Data<'a, T>; DEFAULT_MAX_CHANNELS]>,
    ptrs: SmallVec<[*const T; DEFAULT_MAX_CHANNELS]>,
    mut_ptrs: Option<SmallVec<[*mut T; DEFAULT_MAX_CHANNELS]>>,
    pub(crate) format: SampleFormat,
    pub(crate) channels: u8,
    pub(crate) samples: u32,
}

impl<T: Pod> AudioData<'_, T> {
    pub(crate) fn new(format: SampleFormat, channels: u8, samples: u32) -> AudioData<'static, T> {
        let num_planes = if format.is_planar() {
            channels as usize
        } else {
            1
        };
        let plane_size = format.calc_plane_size(channels, samples);

        let mut planes: SmallVec<[Data<'static, T>; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut ptrs: SmallVec<[*const T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut mut_ptrs: SmallVec<[*mut T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);

        for _ in 0..num_planes {
            let mut plane = Data::new(plane_size, T::zeroed());
            ptrs.push(plane.as_ptr());
            mut_ptrs.push(plane.as_mut_ptr().unwrap());
            planes.push(plane);
        }

        AudioData {
            planes,
            ptrs,
            mut_ptrs: Some(mut_ptrs),
            format,
            channels,
            samples,
        }
    }

    pub(crate) fn from_slices<'a>(slices: &'a [&[T]], format: SampleFormat, channels: u8, samples: u32) -> Result<AudioData<'a, T>> {
        let num_planes = Self::validate(slices, format, channels, samples)?;
        let mut planes: SmallVec<[Data<'a, T>; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut ptrs: SmallVec<[*const T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);

        for slice in slices {
            let plane = Data::from_slice(slice);
            ptrs.push(plane.as_ptr());
            planes.push(plane);
        }

        Ok(AudioData {
            planes,
            ptrs,
            mut_ptrs: None,
            format,
            channels,
            samples,
        })
    }

    pub(crate) fn from_slices_mut<'a>(slices: &'a mut [&mut [T]], format: SampleFormat, channels: u8, samples: u32) -> Result<AudioData<'a, T>> {
        let num_planes = Self::validate(slices, format, channels, samples)?;
        let mut planes: SmallVec<[Data<'a, T>; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut ptrs: SmallVec<[*const T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut mut_ptrs: SmallVec<[*mut T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);

        for slice in slices {
            let mut plane = Data::from_slice_mut(slice);
            ptrs.push(plane.as_ptr());
            mut_ptrs.push(plane.as_mut_ptr().unwrap());
            planes.push(plane);
        }

        Ok(AudioData {
            planes,
            ptrs,
            mut_ptrs: Some(mut_ptrs),
            format,
            channels,
            samples,
        })
    }

    pub(crate) fn plane_ptrs(&self) -> &[*const T] {
        self.ptrs.as_ref()
    }

    pub(crate) fn plane_mut_ptrs(&mut self) -> Option<&[*mut T]> {
        self.mut_ptrs.as_ref().map(|vec| vec.as_slice())
    }

    pub(crate) fn plane_ptrs_as<SampleType>(&self) -> &[*const SampleType] {
        unsafe { slice::from_raw_parts(self.ptrs.as_ptr() as *const *const SampleType, self.ptrs.len()) }
    }

    pub(crate) fn plane_mut_ptrs_as<SampleType>(&mut self) -> Option<&[*mut SampleType]> {
        self.mut_ptrs.as_ref().map(|vec| unsafe { slice::from_raw_parts(vec.as_ptr() as *const *mut SampleType, vec.len()) })
    }

    pub(crate) fn plane_ref_as<SampleType: Pod>(&self, index: usize) -> &[SampleType] {
        bytemuck::cast_slice(self.planes[index].as_ref())
    }

    pub(crate) fn plane_mut_as<SampleType: Pod>(&mut self, index: usize) -> Option<&mut [SampleType]> {
        self.planes[index].as_mut().map(|data| bytemuck::cast_slice_mut(data))
    }

    pub(crate) fn into_owned(self) -> AudioData<'static, T> {
        let num_planes = self.planes.len();
        let mut planes: SmallVec<[Data<'static, T>; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut ptrs: SmallVec<[*const T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);
        let mut mut_ptrs: SmallVec<[*mut T; DEFAULT_MAX_CHANNELS]> = SmallVec::with_capacity(num_planes);

        for plane in self.planes {
            let mut owned = plane.into_owned();
            ptrs.push(owned.as_ptr());
            mut_ptrs.push(owned.as_mut_ptr().unwrap());
            planes.push(owned);
        }

        AudioData {
            planes,
            ptrs,
            mut_ptrs: Some(mut_ptrs),
            format: self.format,
            channels: self.channels,
            samples: self.samples,
        }
    }

    fn validate<S>(slices: &[S], format: SampleFormat, channels: u8, samples: u32) -> Result<usize>
    where
        S: AsRef<[T]>,
    {
        let num_planes = if format.is_planar() {
            channels as usize
        } else {
            1
        };

        if slices.len() != num_planes {
            return Err(invalid_param_error!(channels));
        }

        let plane_size = format.calc_plane_size(channels, samples);
        if slices.iter().any(|slice| slice.as_ref().len() != plane_size) {
            return Err(invalid_param_error!(channels));
        }

        Ok(num_planes)
    }
}
