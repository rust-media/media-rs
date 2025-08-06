use std::{
    borrow::Cow,
    io::{self, Read, Write},
};

use bitflags::bitflags;
use num_rational::Rational64;

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PacketFlags: u32 {
        const Key = 1;
        const Corrupt = 2;
    }
}

#[derive(Clone)]
pub struct Packet<'a> {
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub flags: PacketFlags,
    pub pos: Option<usize>,
    pub stream_index: Option<usize>,
    pub data: Cow<'a, [u8]>,
}

impl<'a> Packet<'a> {
    fn from_data<T>(data: T) -> Self
    where
        T: Into<Cow<'a, [u8]>>,
    {
        Self {
            pts: None,
            dts: None,
            duration: None,
            time_base: None,
            flags: PacketFlags::empty(),
            pos: None,
            stream_index: None,
            data: data.into(),
        }
    }

    pub fn new(size: usize) -> Self {
        Self::from_data(vec![0; size])
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_data(Vec::with_capacity(capacity))
    }

    pub fn from_slice(data: &'a [u8]) -> Self {
        Self::from_data(data)
    }

    pub fn to_owned(&self) -> Packet<'static> {
        Packet {
            pts: self.pts,
            dts: self.dts,
            duration: self.duration,
            time_base: self.time_base,
            flags: self.flags,
            pos: self.pos,
            stream_index: self.stream_index,
            data: Cow::Owned(self.data.to_vec()),
        }
    }

    pub fn into_owned(self) -> Packet<'static> {
        Packet {
            pts: self.pts,
            dts: self.dts,
            duration: self.duration,
            time_base: self.time_base,
            flags: self.flags,
            pos: self.pos,
            stream_index: self.stream_index,
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}

pub trait ReadPacket: Read {
    fn read_packet(&mut self, size: usize) -> io::Result<Packet> {
        let mut packet = Packet::new(size);

        if let Cow::Owned(ref mut vec) = packet.data {
            self.read_exact(vec)?;
        } else {
            // Packet::new always creates an owned packet
            unreachable!()
        }

        Ok(packet)
    }
}

pub trait WritePacket: Write {
    fn write_packet(&mut self, packet: &Packet) -> io::Result<()> {
        self.write_all(&packet.data)
    }
}

impl<R: Read + ?Sized> ReadPacket for R {}
impl<W: Write + ?Sized> WritePacket for W {}
