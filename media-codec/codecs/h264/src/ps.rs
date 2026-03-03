//! H.264/AVC Parameter Sets Container

use std::array;

use media_core::{not_found_error, Result};

use crate::{
    constants::{MAX_PPS_COUNT, MAX_QP_COUNT, MAX_SPS_COUNT},
    pps::Pps,
    sps::Sps,
    tables::CHROMA_QP,
};

/// Container for all SPS and PPS parameter sets
pub struct ParameterSets {
    /// SPS list (indexed by seq_parameter_set_id)
    pub sps_list: [Option<Box<Sps>>; MAX_SPS_COUNT],
    /// PPS list (indexed by pic_parameter_set_id)
    pub pps_list: [Option<Box<Pps>>; MAX_PPS_COUNT],
    /// Chroma QP tables for each PPS (indexed by pic_parameter_set_id)
    chroma_qp_tables: [Option<Box<ChromaQpTables>>; MAX_PPS_COUNT],
}

/// Precomputed chroma QP tables for a PPS
#[derive(Clone)]
struct ChromaQpTables {
    /// Cb QP table
    cb: [u8; MAX_QP_COUNT],
    /// Cr QP table  
    cr: [u8; MAX_QP_COUNT],
}

impl ParameterSets {
    pub fn new() -> Self {
        Self {
            sps_list: array::from_fn(|_| None),
            pps_list: array::from_fn(|_| None),
            chroma_qp_tables: array::from_fn(|_| None),
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
        let sps_id = pps.seq_parameter_set_id as usize;

        let sps = self.sps_list.get(sps_id).and_then(|s| s.as_ref()).ok_or_else(|| not_found_error!("SPS", sps_id))?;

        // Build chroma QP tables
        let bit_depth = sps.bit_depth_luma as u32;
        let mut tables = ChromaQpTables {
            cb: [0u8; MAX_QP_COUNT],
            cr: [0u8; MAX_QP_COUNT],
        };

        Self::build_qp_table(&mut tables.cb, pps.chroma_qp_index_offset, bit_depth);
        Self::build_qp_table(&mut tables.cr, pps.second_chroma_qp_index_offset, bit_depth);

        self.chroma_qp_tables[pps_id] = Some(Box::new(tables));
        self.pps_list[pps_id] = Some(Box::new(pps));
        Ok(())
    }

    /// Get chroma Cb QP for a given luma QP and PPS
    pub fn get_chroma_qp_cb(&self, pps_id: u32, qp_y: i32) -> u8 {
        self.chroma_qp_tables
            .get(pps_id as usize)
            .and_then(|t| t.as_ref())
            .map(|t| t.cb[qp_y.clamp(0, MAX_QP_COUNT as i32 - 1) as usize])
            .unwrap_or(0)
    }

    /// Get chroma Cr QP for a given luma QP and PPS
    pub fn get_chroma_qp_cr(&self, pps_id: u32, qp_y: i32) -> u8 {
        self.chroma_qp_tables
            .get(pps_id as usize)
            .and_then(|t| t.as_ref())
            .map(|t| t.cr[qp_y.clamp(0, MAX_QP_COUNT as i32 - 1) as usize])
            .unwrap_or(0)
    }

    /// Get full chroma QP tables for a PPS (for decode_mb_residual)
    pub fn get_chroma_qp_tables(&self, pps_id: u32) -> Option<[[u8; MAX_QP_COUNT]; 2]> {
        self.chroma_qp_tables.get(pps_id as usize).and_then(|t| t.as_ref()).map(|t| [t.cb, t.cr])
    }

    /// Build chroma QP table
    fn build_qp_table(table: &mut [u8; MAX_QP_COUNT], index: i32, depth: u32) {
        let max_qp = (51 + 6 * (depth as i32 - 8)) as usize;
        for (i, item) in table.iter_mut().enumerate().take(max_qp + 1) {
            let qp = (i as i32 + index).clamp(0, max_qp as i32) as usize;
            *item = CHROMA_QP[depth as usize - 8][qp];
        }
    }
}

impl Default for ParameterSets {
    fn default() -> Self {
        Self::new()
    }
}
