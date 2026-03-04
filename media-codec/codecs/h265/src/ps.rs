//! H.265/HEVC Parameter Sets Container

use std::array;

use media_core::{not_found_error, Result};

use crate::{
    constants::{MAX_PPS_COUNT, MAX_SPS_COUNT, MAX_VPS_COUNT},
    pps::Pps,
    sps::Sps,
    vps::Vps,
};

/// Container for all VPS, SPS and PPS parameter sets
pub struct ParameterSets {
    /// VPS list (indexed by video_parameter_set_id)
    pub vps_list: [Option<Box<Vps>>; MAX_VPS_COUNT],
    /// SPS list (indexed by seq_parameter_set_id)
    pub sps_list: [Option<Box<Sps>>; MAX_SPS_COUNT],
    /// PPS list (indexed by pic_parameter_set_id)
    pub pps_list: [Option<Box<Pps>>; MAX_PPS_COUNT],
}

impl ParameterSets {
    pub fn new() -> Self {
        Self {
            vps_list: array::from_fn(|_| None),
            sps_list: array::from_fn(|_| None),
            pps_list: array::from_fn(|_| None),
        }
    }

    pub fn get_vps(&self, id: u32) -> Option<&Vps> {
        self.vps_list.get(id as usize)?.as_deref()
    }

    pub fn get_sps(&self, id: u32) -> Option<&Sps> {
        self.sps_list.get(id as usize)?.as_deref()
    }

    pub fn get_pps(&self, id: u32) -> Option<&Pps> {
        self.pps_list.get(id as usize)?.as_deref()
    }

    pub fn add_vps(&mut self, vps: Vps) {
        let id = vps.video_parameter_set_id as usize;
        if id < MAX_VPS_COUNT {
            self.vps_list[id] = Some(Box::new(vps));
        }
    }

    pub fn add_sps(&mut self, sps: Sps) -> Result<()> {
        let sps_id = sps.seq_parameter_set_id as usize;
        let vps_id = sps.video_parameter_set_id as usize;

        self.vps_list.get(vps_id).and_then(|v| v.as_ref()).ok_or_else(|| not_found_error!("vps_id", vps_id))?;

        if sps_id < MAX_SPS_COUNT {
            self.sps_list[sps_id] = Some(Box::new(sps));
        }

        Ok(())
    }

    pub fn add_pps(&mut self, pps: Pps) -> Result<()> {
        let pps_id = pps.pic_parameter_set_id as usize;
        let sps_id = pps.seq_parameter_set_id as usize;

        self.sps_list.get(sps_id).and_then(|s| s.as_ref()).ok_or_else(|| not_found_error!("sps_id", sps_id))?;

        if pps_id < MAX_PPS_COUNT {
            self.pps_list[pps_id] = Some(Box::new(pps));
        }

        Ok(())
    }
}

impl Default for ParameterSets {
    fn default() -> Self {
        Self::new()
    }
}
