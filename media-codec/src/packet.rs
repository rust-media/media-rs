use std::io::{self, Read, Write};

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
pub struct Packet {
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub flags: PacketFlags,
    pub pos: Option<usize>,
    pub stream_index: Option<usize>,
    pub data: Vec<u8>,
}

impl Packet {
    fn from_data(data: Vec<u8>) -> Self {
        Self {
            pts: None,
            dts: None,
            duration: None,
            time_base: None,
            flags: PacketFlags::empty(),
            pos: None,
            stream_index: None,
            data,
        }
    }

    pub fn new(size: usize) -> Self {
        Self::from_data(vec![0; size])
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_data(Vec::with_capacity(capacity))
    }
}

pub trait ReadMediaPacket: Read {
    fn read_packet(&mut self, size: usize) -> io::Result<Packet> {
        let mut packet = Packet::new(size);

        self.read_exact(packet.data.as_mut_slice())?;

        Ok(packet)
    }
}

pub trait WriteMediaPacket: Write {
    fn write_packet(&mut self, packet: &Packet) -> io::Result<()> {
        self.write_all(&packet.data)
    }
}

impl<R: Read + ?Sized> ReadMediaPacket for R {}
impl<W: Write + ?Sized> WriteMediaPacket for W {}
