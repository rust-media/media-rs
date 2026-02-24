//! H.264/AVC NAL unit header

use media_codec_nal::NalHeader;
use media_core::{invalid_data_error, Result};

/// H.264 NAL unit types as defined in ITU-T H.264 Table 7-1
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NalUnitType {
    /// Unspecified
    Unspecified       = 0,
    /// Coded slice of a non-IDR picture
    SliceNonIdr       = 1,
    /// Coded slice data partition A
    SlicePartA        = 2,
    /// Coded slice data partition B
    SlicePartB        = 3,
    /// Coded slice data partition C
    SlicePartC        = 4,
    /// Coded slice of an IDR picture
    SliceIdr          = 5,
    /// Supplemental enhancement information (SEI)
    Sei               = 6,
    /// Sequence parameter set (SPS)
    Sps               = 7,
    /// Picture parameter set (PPS)
    Pps               = 8,
    /// Access unit delimiter
    Aud               = 9,
    /// End of sequence
    EndOfSequence     = 10,
    /// End of stream
    EndOfStream       = 11,
    /// Filler data
    Filler            = 12,
    /// Sequence parameter set extension
    SpsExt            = 13,
    /// Prefix NAL unit
    Prefix            = 14,
    /// Subset sequence parameter set
    SubsetSps         = 15,
    /// Depth parameter set
    DepthParameterSet = 16,
    // 17-18: Reserved
    /// Coded slice of an auxiliary coded picture without partitioning
    SliceAux          = 19,
    /// Coded slice extension
    SliceExt          = 20,
    /// Coded slice extension for depth view components
    SliceExtDepth     = 21,
    // 22-23: Reserved
    // 24-31: Unspecified
}

impl NalUnitType {
    /// Create from raw NAL unit type value
    pub fn from_u8(value: u8) -> Option<Self> {
        let nal_unit_type = match value {
            0 => Self::Unspecified,
            1 => Self::SliceNonIdr,
            2 => Self::SlicePartA,
            3 => Self::SlicePartB,
            4 => Self::SlicePartC,
            5 => Self::SliceIdr,
            6 => Self::Sei,
            7 => Self::Sps,
            8 => Self::Pps,
            9 => Self::Aud,
            10 => Self::EndOfSequence,
            11 => Self::EndOfStream,
            12 => Self::Filler,
            13 => Self::SpsExt,
            14 => Self::Prefix,
            15 => Self::SubsetSps,
            16 => Self::DepthParameterSet,
            19 => Self::SliceAux,
            20 => Self::SliceExt,
            21 => Self::SliceExtDepth,
            _ => return None,
        };

        Some(nal_unit_type)
    }
}

/// H.264 NAL unit header
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct H264NalHeader {
    /// Forbidden zero bit (should always be 0)
    pub forbidden_zero_bit: bool,
    /// NAL reference indicator (0-3)
    /// Higher values indicate higher importance for reference
    pub nal_ref_idc: u8,
    /// NAL unit type (0-31)
    pub nal_unit_type: NalUnitType,
}

impl H264NalHeader {
    /// Create a new H.264 NAL header
    pub fn new(forbidden_zero_bit: bool, nal_ref_idc: u8, nal_unit_type: NalUnitType) -> Self {
        Self {
            forbidden_zero_bit,
            nal_ref_idc: nal_ref_idc & 0x03,
            nal_unit_type,
        }
    }

    /// Get the NAL unit type as an enum
    pub fn unit_type(&self) -> NalUnitType {
        self.nal_unit_type
    }

    /// Encode the header to a single byte
    pub fn to_byte(&self) -> u8 {
        let f = if self.forbidden_zero_bit {
            0x80
        } else {
            0
        };
        f | ((self.nal_ref_idc & 0x03) << 5) | (self.nal_unit_type as u8 & 0x1F)
    }
}

impl NalHeader for H264NalHeader {
    const HEADER_SIZE: usize = 1;

    fn parse(data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            return Err(invalid_data_error!("H264 NAL: empty data"));
        }

        let byte = data[0];
        let forbidden_zero_bit = (byte >> 7) != 0;
        let nal_ref_idc = (byte >> 5) & 0x03;
        let nal_unit_type = NalUnitType::from_u8(byte & 0x1F).ok_or_else(|| invalid_data_error!("H264 NAL: invalid nal_unit_type"))?;

        // Check forbidden_zero_bit
        if forbidden_zero_bit {
            return Err(invalid_data_error!("H264 NAL: forbidden bit set"));
        }

        Ok(Self {
            forbidden_zero_bit,
            nal_ref_idc,
            nal_unit_type,
        })
    }

    fn nal_unit_type(&self) -> u8 {
        self.nal_unit_type as u8
    }

    fn is_vcl(&self) -> bool {
        // VCL NAL units: types 1-5
        matches!(self.nal_unit_type(), 1..=5)
    }

    fn is_idr(&self) -> bool {
        self.nal_unit_type() == 5
    }

    fn is_parameter_set(&self) -> bool {
        // SPS, PPS, SPS extension, Subset SPS
        matches!(self.nal_unit_type(), 7 | 8 | 13 | 15)
    }
}
