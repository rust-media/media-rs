use std::io::{self, Read, Write};

use bitflags::bitflags;
use num_rational::Rational64;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct PacketFlags: u32 {
        const KEY = 0x0001;
        const CORRUPT = 0x0002;
    }
}

#[derive(Clone)]
pub struct MediaPacket {
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub duration: Option<i64>,
    pub time_base: Option<Rational64>,
    pub flags: PacketFlags,
    pub pos: Option<usize>,
    pub stream_index: Option<usize>,
    pub data: Vec<u8>,
}

impl MediaPacket {
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
    fn read_packet(&mut self, size: usize) -> io::Result<MediaPacket> {
        let mut packet = MediaPacket::new(size);

        self.read_exact(packet.data.as_mut_slice())?;

        Ok(packet)
    }
}

pub trait WriteMediaPacket: Write {
    fn write_packet(&mut self, packet: &MediaPacket) -> io::Result<()> {
        self.write_all(&packet.data)
    }
}

impl<R: Read + ?Sized> ReadMediaPacket for R {}
impl<W: Write + ?Sized> WriteMediaPacket for W {}
