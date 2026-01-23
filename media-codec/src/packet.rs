use std::{
    io::{self, Read, Write},
    sync::Arc,
};

use bitflags::bitflags;
use media_core::{buffer::Buffer, invalid_error, invalid_param_error, rational::Rational64, Result};

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PacketFlags: u32 {
        const Key = 1;
        const Corrupt = 2;
    }
}

#[derive(Clone, Debug)]
enum PacketData<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
    Buffer(Arc<Buffer>),
}

impl PacketData<'_> {
    fn from_slice(slice: &[u8]) -> PacketData<'_> {
        PacketData::Borrowed(slice)
    }

    fn from_vec(vec: Vec<u8>) -> PacketData<'static> {
        PacketData::Owned(vec)
    }

    fn from_buffer(buffer: Arc<Buffer>) -> PacketData<'static> {
        PacketData::Buffer(buffer)
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            PacketData::Borrowed(slice) => slice,
            PacketData::Owned(vec) => vec.as_slice(),
            PacketData::Buffer(buffer) => buffer.data(),
        }
    }

    fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        match self {
            PacketData::Borrowed(_) => None,
            PacketData::Owned(ref mut vec) => Some(vec.as_mut_slice()),
            PacketData::Buffer(buffer) => {
                let mut_buffer = Arc::get_mut(buffer)?;
                Some(mut_buffer.data_mut())
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            PacketData::Borrowed(slice) => slice.len(),
            PacketData::Owned(vec) => vec.len(),
            PacketData::Buffer(buffer) => buffer.len(),
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn capacity(&self) -> usize {
        match &self {
            PacketData::Borrowed(slice) => slice.len(),
            PacketData::Owned(vec) => vec.capacity(),
            PacketData::Buffer(buffer) => buffer.capacity(),
        }
    }

    fn into_owned(self) -> PacketData<'static> {
        match self {
            PacketData::Borrowed(slice) => PacketData::Owned(slice.to_vec()),
            PacketData::Owned(vec) => PacketData::Owned(vec),
            PacketData::Buffer(buffer) => PacketData::Buffer(buffer),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Packet<'a> {
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub flags: PacketFlags,
    pub pos: Option<usize>,
    pub track_index: Option<usize>,
    data: PacketData<'a>,
}

impl<'a> Packet<'a> {
    fn from_data(data: PacketData<'a>) -> Self {
        Self {
            pts: None,
            dts: None,
            duration: None,
            time_base: None,
            flags: PacketFlags::empty(),
            pos: None,
            track_index: None,
            data,
        }
    }

    pub fn new(size: usize) -> Self {
        Self::from_data(PacketData::from_vec(vec![0; size]))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_data(PacketData::from_vec(Vec::with_capacity(capacity)))
    }

    pub fn from_slice(data: &'a [u8]) -> Self {
        Self::from_data(PacketData::from_slice(data))
    }

    pub fn from_buffer(buffer: Arc<Buffer>) -> Self {
        Self::from_data(PacketData::from_buffer(buffer))
    }

    pub fn into_owned(self) -> Packet<'static> {
        Packet {
            pts: self.pts,
            dts: self.dts,
            duration: self.duration,
            time_base: self.time_base,
            flags: self.flags,
            pos: self.pos,
            track_index: self.track_index,
            data: self.data.into_owned(),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        self.data.as_mut_slice()
    }

    pub fn truncate(&mut self, len: usize) -> Result<()> {
        let current_len = self.data.len();
        if len > current_len {
            return Err(invalid_param_error!(len));
        }

        match &mut self.data {
            PacketData::Borrowed(slice) => {
                self.data = PacketData::Borrowed(&slice[..len]);
            }
            PacketData::Owned(vec) => {
                vec.truncate(len);
            }
            PacketData::Buffer(buffer) => {
                let buffer = Arc::get_mut(buffer).ok_or_else(|| invalid_error!("buffer is shared"))?;
                buffer.resize(len);
            }
        }

        Ok(())
    }
}

pub trait ReadPacket: Read {
    fn read_packet(&mut self, size: usize) -> io::Result<Packet<'_>> {
        let mut packet = Packet::new(size);

        if let Some(data_mut) = packet.data_mut() {
            self.read_exact(data_mut)?;
        } else {
            // Packet::new always creates an owned packet
            unreachable!()
        }

        Ok(packet)
    }
}

pub trait WritePacket: Write {
    fn write_packet(&mut self, packet: &Packet) -> io::Result<()> {
        self.write_all(packet.data())
    }
}

impl<T: Read + ?Sized> ReadPacket for T {}
impl<T: Write + ?Sized> WritePacket for T {}

#[derive(Clone, Copy, Debug)]
pub struct PacketProperties {
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub flags: PacketFlags,
    pub pos: Option<usize>,
    pub track_index: Option<usize>,
}

impl PacketProperties {
    pub fn new() -> Self {
        Self {
            pts: None,
            dts: None,
            duration: None,
            time_base: None,
            flags: PacketFlags::empty(),
            pos: None,
            track_index: None,
        }
    }

    pub fn from_packet(packet: &Packet) -> Self {
        Self {
            pts: packet.pts,
            dts: packet.dts,
            duration: packet.duration,
            time_base: packet.time_base,
            flags: packet.flags,
            pos: packet.pos,
            track_index: packet.track_index,
        }
    }
}

impl Default for PacketProperties {
    fn default() -> Self {
        Self::new()
    }
}
