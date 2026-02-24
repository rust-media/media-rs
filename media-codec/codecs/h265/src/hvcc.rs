//! HEVC Configuration Record (HVCC) parser

use std::io::{Cursor, Read};

use media_codec_bitstream::{BigEndian, ByteRead, ByteReader};
use media_core::{invalid_data_error, Result};

use crate::NalUnitType;

/// A single NAL unit array entry in the HVCC
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HvccNaluArray {
    /// Array completeness flag
    pub array_completeness: bool,
    /// NAL unit type
    pub nal_unit_type: u8,
    /// NAL units in this array
    pub nalus: Vec<Vec<u8>>,
}

/// HEVC Decoder Configuration Record
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Hvcc {
    /// Configuration version (always 1)
    pub configuration_version: u8,
    /// General profile space (0-3)
    pub general_profile_space: u8,
    /// General tier flag (0=Main tier, 1=High tier)
    pub general_tier_flag: bool,
    /// General profile IDC
    pub general_profile_idc: u8,
    /// General profile compatibility flags (32 bits)
    pub general_profile_compatibility_flags: u32,
    /// General constraint indicator flags (48 bits)
    pub general_constraint_indicator_flags: u64,
    /// General level IDC
    pub general_level_idc: u8,
    /// Minimum spatial segmentation IDC
    pub min_spatial_segmentation_idc: u16,
    /// Parallelism type
    pub parallelism_type: u8,
    /// Chroma format IDC (0=monochrome, 1=4:2:0, 2=4:2:2, 3=4:4:4)
    pub chroma_format_idc: u8,
    /// Bit depth of luma samples (8-16)
    pub bit_depth_luma: u8,
    /// Bit depth of chroma samples (8-16)
    pub bit_depth_chroma: u8,
    /// Average frame rate (in units of frames / 256 seconds, 0=unspecified)
    pub avg_frame_rate: u16,
    /// Constant frame rate (0=unknown, 1=constant, 2=each temporal layer is
    /// constant)
    pub constant_frame_rate: u8,
    /// Number of temporal layers
    pub num_temporal_layers: u8,
    /// Temporal ID nested flag
    pub temporal_id_nested: bool,
    /// Length in bytes of the NAL unit length field (1, 2, or 4)
    pub length_size: u8,
    /// NAL unit arrays (VPS, SPS, PPS, SEI, etc.)
    pub nalu_arrays: Vec<HvccNaluArray>,
}

impl Hvcc {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let cursor = Cursor::new(data);
        Self::parse_from_reader(cursor)
    }

    pub fn parse_from_reader<R: Read>(reader: R) -> Result<Self> {
        let mut reader = ByteReader::endian(reader, BigEndian);
        Self::parse_from_byte_reader(&mut reader)
    }

    pub fn parse_from_byte_reader<R: Read>(reader: &mut ByteReader<R, BigEndian>) -> Result<Self> {
        // Read configuration version
        let configuration_version = reader.read::<u8>()?;
        if configuration_version != 1 {
            return Err(invalid_data_error!("configuration_version", configuration_version));
        }

        // Read general profile space, tier flag, and profile IDC
        let profile_byte = reader.read::<u8>()?;
        let general_profile_space = (profile_byte >> 6) & 0b11;
        let general_tier_flag = (profile_byte >> 5) & 0b1 != 0;
        let general_profile_idc = profile_byte & 0b0001_1111;

        // Read general profile compatibility flags (32 bits)
        let general_profile_compatibility_flags = reader.read::<u32>()?;

        // Read general constraint indicator flags (48 bits = 6 bytes)
        let constraint_high = reader.read::<u32>()? as u64;
        let constraint_low = reader.read::<u16>()? as u64;
        let general_constraint_indicator_flags = (constraint_high << 16) | constraint_low;

        // Read general level IDC
        let general_level_idc = reader.read::<u8>()?;

        // Read min spatial segmentation IDC (lower 12 bits)
        let min_spatial_segmentation_idc = reader.read::<u16>()? & 0x0FFF;

        // Read parallelism type (lower 2 bits)
        let parallelism_type = reader.read::<u8>()? & 0b11;

        // Read chroma format IDC (lower 2 bits)
        let chroma_format_idc = reader.read::<u8>()? & 0b11;

        // Read bit depth luma (lower 3 bits + 8)
        let bit_depth_luma = (reader.read::<u8>()? & 0b111) + 8;

        // Read bit depth chroma (lower 3 bits + 8)
        let bit_depth_chroma = (reader.read::<u8>()? & 0b111) + 8;

        // Read average frame rate
        let avg_frame_rate = reader.read::<u16>()?;

        // Read constant frame rate, num temporal layers, temporal ID nested, and length
        // size
        let rate_byte = reader.read::<u8>()?;
        let constant_frame_rate = (rate_byte >> 6) & 0b11;
        let num_temporal_layers = (rate_byte >> 3) & 0b111;
        let temporal_id_nested = (rate_byte >> 2) & 0b1 != 0;
        let length_size = (rate_byte & 0b11) + 1;

        // Validate length size (must be 1, 2, or 4)
        if length_size != 1 && length_size != 2 && length_size != 4 {
            return Err(invalid_data_error!("NAL unit length size", length_size));
        }

        // Read number of arrays
        let num_of_arrays = reader.read::<u8>()?;

        // Read NAL unit arrays
        let mut nalu_arrays = Vec::with_capacity(num_of_arrays as usize);
        for _ in 0..num_of_arrays {
            // Read array header
            let array_header = reader.read::<u8>()?;
            let array_completeness = (array_header >> 7) & 0b1 != 0;
            let nal_unit_type = array_header & 0b0011_1111;

            // Read number of NAL units
            let num_nalus = reader.read::<u16>()?;

            // Read NAL units
            let mut nalus = Vec::with_capacity(num_nalus as usize);
            for _ in 0..num_nalus {
                let nalu_length = reader.read::<u16>()? as usize;
                let mut nalu_data = vec![0u8; nalu_length];
                reader.read_bytes(&mut nalu_data)?;
                nalus.push(nalu_data);
            }

            nalu_arrays.push(HvccNaluArray {
                array_completeness,
                nal_unit_type,
                nalus,
            });
        }

        Ok(Self {
            configuration_version,
            general_profile_space,
            general_tier_flag,
            general_profile_idc,
            general_profile_compatibility_flags,
            general_constraint_indicator_flags,
            general_level_idc,
            min_spatial_segmentation_idc,
            parallelism_type,
            chroma_format_idc,
            bit_depth_luma,
            bit_depth_chroma,
            avg_frame_rate,
            constant_frame_rate,
            num_temporal_layers,
            temporal_id_nested,
            length_size,
            nalu_arrays,
        })
    }

    /// Get VPS NAL units as an iterator (zero allocation)
    pub fn vps(&self) -> impl Iterator<Item = &[u8]> {
        self.nalus(NalUnitType::VpsNut as u8)
    }

    /// Get SPS NAL units as an iterator (zero allocation)
    pub fn sps(&self) -> impl Iterator<Item = &[u8]> {
        self.nalus(NalUnitType::SpsNut as u8)
    }

    /// Get PPS NAL units as an iterator (zero allocation)
    pub fn pps(&self) -> impl Iterator<Item = &[u8]> {
        self.nalus(NalUnitType::PpsNut as u8)
    }

    /// Get NAL units by type as an iterator (zero allocation)
    pub fn nalus(&self, nal_unit_type: u8) -> impl Iterator<Item = &[u8]> {
        self.nalu_arrays
            .iter()
            .filter(move |array| array.nal_unit_type == nal_unit_type)
            .flat_map(|array| array.nalus.iter().map(|nalu| nalu.as_slice()))
    }
}
