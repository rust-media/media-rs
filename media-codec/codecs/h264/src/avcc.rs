//! AVC Configuration Record (AVCC) parser

use std::io::{Cursor, Read};

use media_codec_bitstream::{BigEndian, ByteRead, ByteReader};
use media_core::{invalid_data_error, Result};

/// Extended AVCC configuration for high profiles
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvccExt {
    /// Chroma format indicator (0=monochrome, 1=4:2:0, 2=4:2:2, 3=4:4:4)
    pub chroma_format: u8,
    /// Bit depth of luma samples (8-14)
    pub bit_depth_luma: u8,
    /// Bit depth of chroma samples (8-14)
    pub bit_depth_chroma: u8,
    /// Sequence parameter set extension NAL units
    pub sequence_parameter_sets_ext: Vec<Vec<u8>>,
}

impl Default for AvccExt {
    fn default() -> Self {
        Self {
            chroma_format: 1, // 4:2:0
            bit_depth_luma: 8,
            bit_depth_chroma: 8,
            sequence_parameter_sets_ext: Vec::new(),
        }
    }
}

/// AVC Decoder Configuration Record
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Avcc {
    /// Configuration version (always 1)
    pub configuration_version: u8,
    /// AVC profile indication
    pub avc_profile_indication: u8,
    /// Profile compatibility flags
    pub profile_compatibility: u8,
    /// AVC level indication
    pub avc_level_indication: u8,
    /// Length in bytes of the NAL unit length field (1, 2, or 4)
    pub length_size: u8,
    /// Sequence Parameter Set NAL units
    pub sequence_parameter_sets: Vec<Vec<u8>>,
    /// Picture Parameter Set NAL units
    pub picture_parameter_sets: Vec<Vec<u8>>,
    /// Extended configuration (for high profiles)
    pub ext: Option<AvccExt>,
}

impl Avcc {
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

        // Read profile/level information
        let avc_profile_indication = reader.read::<u8>()?;
        let profile_compatibility = reader.read::<u8>()?;
        let avc_level_indication = reader.read::<u8>()?;

        // Read length size (lower 2 bits, add 1)
        let length_size_byte = reader.read::<u8>()?;
        let length_size = (length_size_byte & 0b11) + 1;

        // Validate length size (must be 1, 2, or 4)
        if length_size != 1 && length_size != 2 && length_size != 4 {
            return Err(invalid_data_error!("NAL unit length size", length_size));
        }

        // Read number of SPS (lower 5 bits)
        let num_sps_byte = reader.read::<u8>()?;
        let num_sps = num_sps_byte & 0b0001_1111;

        // Read SPS NAL units
        let mut sequence_parameter_sets = Vec::with_capacity(num_sps as usize);
        for _ in 0..num_sps {
            let sps_length = reader.read::<u16>()? as usize;
            let mut sps_data = vec![0u8; sps_length];
            reader.read_bytes(&mut sps_data)?;
            sequence_parameter_sets.push(sps_data);
        }

        // Read number of PPS
        let num_pps = reader.read::<u8>()?;

        // Read PPS NAL units
        let mut picture_parameter_sets = Vec::with_capacity(num_pps as usize);
        for _ in 0..num_pps {
            let pps_length = reader.read::<u16>()? as usize;
            let mut pps_data = vec![0u8; pps_length];
            reader.read_bytes(&mut pps_data)?;
            picture_parameter_sets.push(pps_data);
        }

        // Try to read extended configuration
        // This is optional and may not be present
        let ext = Self::try_parse_ext(reader);

        Ok(Self {
            configuration_version,
            avc_profile_indication,
            profile_compatibility,
            avc_level_indication,
            length_size,
            sequence_parameter_sets,
            picture_parameter_sets,
            ext,
        })
    }

    /// Try to parse extended configuration
    fn try_parse_ext<R: Read>(reader: &mut ByteReader<R, BigEndian>) -> Option<AvccExt> {
        // Extended configuration is typically present for high profiles
        // Even if not required by profile, some encoders include it
        // Try to parse it if there's more data available

        // Read chroma format
        let chroma_format_byte = match reader.read::<u8>() {
            Ok(b) => b,
            Err(_) => return None,
        };
        let chroma_format = chroma_format_byte & 0b11;

        // Read bit depth luma
        let bit_depth_luma_byte = match reader.read::<u8>() {
            Ok(b) => b,
            Err(_) => return None,
        };
        let bit_depth_luma = (bit_depth_luma_byte & 0b111) + 8;

        // Read bit depth chroma
        let bit_depth_chroma_byte = match reader.read::<u8>() {
            Ok(b) => b,
            Err(_) => return None,
        };
        let bit_depth_chroma = (bit_depth_chroma_byte & 0b111) + 8;

        // Read number of SPS extensions
        let num_sps_ext = match reader.read::<u8>() {
            Ok(n) => n,
            Err(_) => return None,
        };

        // Read SPS extension NAL units
        let mut sps_ext = Vec::with_capacity(num_sps_ext as usize);
        for _ in 0..num_sps_ext {
            let sps_ext_length = match reader.read::<u16>() {
                Ok(len) => len as usize,
                Err(_) => break,
            };
            let mut sps_ext_data = vec![0u8; sps_ext_length];
            if reader.read_bytes(&mut sps_ext_data).is_err() {
                break;
            }
            sps_ext.push(sps_ext_data);
        }

        Some(AvccExt {
            chroma_format,
            bit_depth_luma,
            bit_depth_chroma,
            sequence_parameter_sets_ext: sps_ext,
        })
    }
}
