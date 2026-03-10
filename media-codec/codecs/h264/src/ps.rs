//! H.264/AVC Parameter Sets Container

use std::array;

use media_core::Result;

use crate::{
    constants::{MAX_PPS_COUNT, MAX_SPS_COUNT},
    pps::Pps,
    sps::Sps,
};

/// Container for all SPS and PPS parameter sets
pub struct ParameterSets {
    /// SPS list (indexed by seq_parameter_set_id)
    pub sps_list: [Option<Box<Sps>>; MAX_SPS_COUNT],
    /// PPS list (indexed by pic_parameter_set_id)
    pub pps_list: [Option<Box<Pps>>; MAX_PPS_COUNT],
}

impl ParameterSets {
    pub fn new() -> Self {
        Self {
            sps_list: array::from_fn(|_| None),
            pps_list: array::from_fn(|_| None),
        }
    }

    pub fn get_sps(&self, id: u32) -> Option<&Sps> {
        self.sps_list.get(id as usize)?.as_deref()
    }

    pub fn get_pps(&self, id: u32) -> Option<&Pps> {
        self.pps_list.get(id as usize)?.as_deref()
    }

    pub fn add_sps(&mut self, sps: Sps) {
        let id = sps.seq_parameter_set_id as usize;
        if id < MAX_SPS_COUNT {
            self.sps_list[id] = Some(Box::new(sps));
        }
    }

    pub fn add_pps(&mut self, pps: Pps) -> Result<()> {
        let pps_id = pps.pic_parameter_set_id as usize;
        self.pps_list[pps_id] = Some(Box::new(pps));
        Ok(())
    }
}

impl Default for ParameterSets {
    fn default() -> Self {
        Self::new()
    }
}
