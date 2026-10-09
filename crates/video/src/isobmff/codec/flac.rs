//! FLAC decoder configuration records.

use crate::bitstream::BitReader;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Result;

/// `dfLa` — FLAC metadata blocks, with STREAMINFO decoded.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FlacConfig {
    /// Raw metadata blocks (each with its 4-byte header) as stored in `dfLa`.
    pub metadata_blocks: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    pub total_samples: u64,
}

impl FlacConfig {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur::new(data, "dfLa");
        c.full_header()?;
        let blocks = c.rest().to_vec();
        let mut cfg = FlacConfig { metadata_blocks: blocks.clone(), ..Default::default() };
        let mut b = Cur::new(&blocks, "dfLa");
        let hdr = b.u32()?;
        if (hdr >> 24) & 0x7F == 0 {
            let si = b.bytes(((hdr & 0xFF_FFFF) as usize).min(34))?;
            if si.len() >= 18 {
                let mut r = BitReader::new(&si[10..18]);
                cfg.sample_rate = r.read_bits(20).unwrap_or(0);
                cfg.channels = r.read_bits(3).unwrap_or(0) as u8 + 1;
                cfg.bits_per_sample = r.read_bits(5).unwrap_or(0) as u8 + 1;
                cfg.total_samples = r.read_bits_u64(36).unwrap_or(0);
            }
        }
        Ok(cfg)
    }
}
