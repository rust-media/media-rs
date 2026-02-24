//! H.265/HEVC NAL unit header

use media_codec_nal::NalHeader;
use media_core::{invalid_data_error, Result};

/// H.265 NAL unit types as defined in ITU-T H.265 Table 7-1
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NalUnitType {
    /// Coded slice segment of a trailing picture (TRAIL_N)
    TrailN       = 0,
    /// Coded slice segment of a trailing picture (TRAIL_R)
    TrailR       = 1,
    /// Coded slice segment of a TSA picture (TSA_N)
    TsaN         = 2,
    /// Coded slice segment of a TSA picture (TSA_R)
    TsaR         = 3,
    /// Coded slice segment of a STSA picture (STSA_N)
    StsaN        = 4,
    /// Coded slice segment of a STSA picture (STSA_R)
    StsaR        = 5,
    /// Coded slice segment of a RADL picture (RADL_N)
    RadlN        = 6,
    /// Coded slice segment of a RADL picture (RADL_R)
    RadlR        = 7,
    /// Coded slice segment of a RASL picture (RASL_N)
    RaslN        = 8,
    /// Coded slice segment of a RASL picture (RASL_R)
    RaslR        = 9,
    // 10-15: Reserved VCL NAL unit types
    /// Coded slice segment of a BLA picture (BLA_W_LP)
    BlaWLp       = 16,
    /// Coded slice segment of a BLA picture (BLA_W_RADL)
    BlaWRadl     = 17,
    /// Coded slice segment of a BLA picture (BLA_N_LP)
    BlaNLp       = 18,
    /// Coded slice segment of an IDR picture (IDR_W_RADL)
    IdrWRadl     = 19,
    /// Coded slice segment of an IDR picture (IDR_N_LP)
    IdrNLp       = 20,
    /// Coded slice segment of a CRA picture
    CraNut       = 21,
    // 22-23: Reserved IRAP VCL NAL unit types
    // 24-31: Reserved non-IRAP VCL NAL unit types
    /// Video parameter set (VPS)
    VpsNut       = 32,
    /// Sequence parameter set (SPS)
    SpsNut       = 33,
    /// Picture parameter set (PPS)
    PpsNut       = 34,
    /// Access unit delimiter
    AudNut       = 35,
    /// End of sequence
    EosNut       = 36,
    /// End of bitstream
    EobNut       = 37,
    /// Filler data
    FdNut        = 38,
    /// Supplemental enhancement information (prefix)
    PrefixSeiNut = 39,
    /// Supplemental enhancement information (suffix)
    SuffixSeiNut = 40,
    // 41-47: Reserved
    // 48-63: Unspecified
}

impl NalUnitType {
    /// Create from raw NAL unit type value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::TrailN),
            1 => Some(Self::TrailR),
            2 => Some(Self::TsaN),
            3 => Some(Self::TsaR),
            4 => Some(Self::StsaN),
            5 => Some(Self::StsaR),
            6 => Some(Self::RadlN),
            7 => Some(Self::RadlR),
            8 => Some(Self::RaslN),
            9 => Some(Self::RaslR),
            16 => Some(Self::BlaWLp),
            17 => Some(Self::BlaWRadl),
            18 => Some(Self::BlaNLp),
            19 => Some(Self::IdrWRadl),
            20 => Some(Self::IdrNLp),
            21 => Some(Self::CraNut),
            32 => Some(Self::VpsNut),
            33 => Some(Self::SpsNut),
            34 => Some(Self::PpsNut),
            35 => Some(Self::AudNut),
            36 => Some(Self::EosNut),
            37 => Some(Self::EobNut),
            38 => Some(Self::FdNut),
            39 => Some(Self::PrefixSeiNut),
            40 => Some(Self::SuffixSeiNut),
            _ => None,
        }
    }

    /// Check if this is an IRAP (Intra Random Access Point) NAL unit type
    ///
    /// IRAP NAL units include BLA, IDR, and CRA pictures (types 16-21)
    #[inline]
    pub fn is_irap(&self) -> bool {
        matches!(self, Self::BlaWLp | Self::BlaWRadl | Self::BlaNLp | Self::IdrWRadl | Self::IdrNLp | Self::CraNut)
    }

    /// Check if this is an IDR (Instantaneous Decoder Refresh) NAL unit type
    #[inline]
    pub fn is_idr(&self) -> bool {
        matches!(self, Self::IdrWRadl | Self::IdrNLp)
    }

    /// Check if this is a VCL (Video Coding Layer) NAL unit type
    #[inline]
    pub fn is_vcl(&self) -> bool {
        (*self as u8) <= 31
    }

    /// Check if this is a parameter set NAL unit type (VPS, SPS, or PPS)
    #[inline]
    pub fn is_parameter_set(&self) -> bool {
        matches!(self, Self::VpsNut | Self::SpsNut | Self::PpsNut)
    }
}

/// H.265 NAL unit header
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct H265NalHeader {
    /// Forbidden zero bit (should always be 0)
    pub forbidden_zero_bit: bool,
    /// NAL unit type (0-63)
    pub nal_unit_type: NalUnitType,
    /// Layer ID (0-63)
    pub nuh_layer_id: u8,
    /// Temporal ID plus 1 (1-7)
    pub nuh_temporal_id_plus1: u8,
}

impl H265NalHeader {
    /// Create a new H.265 NAL header
    pub fn new(forbidden_zero_bit: bool, nal_unit_type: NalUnitType, nuh_layer_id: u8, nuh_temporal_id_plus1: u8) -> Self {
        Self {
            forbidden_zero_bit,
            nal_unit_type,
            nuh_layer_id: nuh_layer_id & 0x3F,
            nuh_temporal_id_plus1: nuh_temporal_id_plus1 & 0x07,
        }
    }

    /// Get the NAL unit type as an enum
    pub fn unit_type(&self) -> NalUnitType {
        self.nal_unit_type
    }

    /// Get the temporal ID (nuh_temporal_id_plus1 - 1)
    pub fn temporal_id(&self) -> u8 {
        self.nuh_temporal_id_plus1.saturating_sub(1)
    }

    /// Check if this is an IRAP (Intra Random Access Point) NAL unit
    ///
    /// IRAP NAL units include BLA, IDR, and CRA pictures
    pub fn is_irap(&self) -> bool {
        self.nal_unit_type.is_irap()
    }

    /// Encode the header to two bytes
    pub fn to_bytes(&self) -> [u8; 2] {
        let byte0 = if self.forbidden_zero_bit {
            0x80
        } else {
            0
        } | ((self.nal_unit_type as u8 & 0x3F) << 1) |
            ((self.nuh_layer_id >> 5) & 0x01);
        let byte1 = ((self.nuh_layer_id & 0x1F) << 3) | (self.nuh_temporal_id_plus1 & 0x07);
        [byte0, byte1]
    }
}

impl NalHeader for H265NalHeader {
    const HEADER_SIZE: usize = 2;

    fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 2 {
            return Err(invalid_data_error!("H265 NAL: data too short"));
        }

        let byte0 = data[0];
        let byte1 = data[1];

        let forbidden_zero_bit = (byte0 >> 7) != 0;
        let nal_unit_type_raw = (byte0 >> 1) & 0x3F;
        let nuh_layer_id = ((byte0 & 0x01) << 5) | ((byte1 >> 3) & 0x1F);
        let nuh_temporal_id_plus1 = byte1 & 0x07;

        // Check forbidden_zero_bit
        if forbidden_zero_bit {
            return Err(invalid_data_error!("H265 NAL: forbidden bit set"));
        }

        // Check temporal_id_plus1 is not 0
        if nuh_temporal_id_plus1 == 0 {
            return Err(invalid_data_error!("H265 NAL: nuh_temporal_id_plus1 cannot be 0"));
        }

        let nal_unit_type = NalUnitType::from_u8(nal_unit_type_raw).ok_or_else(|| invalid_data_error!("H265 NAL: invalid nal_unit_type"))?;

        Ok(Self {
            forbidden_zero_bit,
            nal_unit_type,
            nuh_layer_id,
            nuh_temporal_id_plus1,
        })
    }

    fn nal_unit_type(&self) -> u8 {
        self.nal_unit_type as u8
    }

    fn is_vcl(&self) -> bool {
        self.nal_unit_type.is_vcl()
    }

    fn is_idr(&self) -> bool {
        self.nal_unit_type.is_idr()
    }

    fn is_parameter_set(&self) -> bool {
        self.nal_unit_type.is_parameter_set()
    }
}
