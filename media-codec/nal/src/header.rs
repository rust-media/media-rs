//! NAL header trait definition

use media_core::Result;

/// Trait for parsing NAL unit headers
pub trait NalHeader: Sized + Clone + std::fmt::Debug {
    /// The size of the NAL header in bytes
    const HEADER_SIZE: usize;

    /// Parse a NAL header from the given data
    fn parse(data: &[u8]) -> Result<Self>;

    /// Get the NAL unit type
    fn nal_unit_type(&self) -> u8;

    /// Check if this is a VCL (Video Coding Layer) NAL unit
    fn is_vcl(&self) -> bool;

    /// Check if this NAL unit is an IDR (Instantaneous Decoder Refresh) picture
    fn is_idr(&self) -> bool;

    /// Check if this NAL unit is a parameter set (SPS, PPS, VPS, etc.)
    fn is_parameter_set(&self) -> bool;
}
