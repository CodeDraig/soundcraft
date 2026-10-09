//! AV1 decoder configuration records.

use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Error;
use crate::isobmff::error::Result;

/// `av1C` — AV1CodecConfigurationRecord.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Av1Config {
    pub seq_profile: u8,
    pub seq_level_idx_0: u8,
    pub seq_tier_0: bool,
    pub high_bitdepth: bool,
    pub twelve_bit: bool,
    pub monochrome: bool,
    pub chroma_subsampling_x: bool,
    pub chroma_subsampling_y: bool,
    pub chroma_sample_position: u8,
    pub initial_presentation_delay_minus_one: Option<u8>,
    pub config_obus: Vec<u8>,
}

impl Av1Config {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur::new(data, "av1C");
        let m = c.u8()?;
        if m & 0x80 == 0 {
            return Err(Error::Invalid("av1C marker bit not set".into()));
        }
        let a = c.u8()?;
        let b = c.u8()?;
        let d = c.u8()?;
        Ok(Av1Config {
            seq_profile: a >> 5,
            seq_level_idx_0: a & 0x1F,
            seq_tier_0: b & 0x80 != 0,
            high_bitdepth: b & 0x40 != 0,
            twelve_bit: b & 0x20 != 0,
            monochrome: b & 0x10 != 0,
            chroma_subsampling_x: b & 0x08 != 0,
            chroma_subsampling_y: b & 0x04 != 0,
            chroma_sample_position: b & 3,
            initial_presentation_delay_minus_one: if d & 0x10 != 0 { Some(d & 0x0F) } else { None },
            config_obus: c.rest().to_vec(),
        })
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u8(0x81);
        b.u8((self.seq_profile << 5) | (self.seq_level_idx_0 & 0x1F));
        b.u8(((self.seq_tier_0 as u8) << 7)
            | ((self.high_bitdepth as u8) << 6)
            | ((self.twelve_bit as u8) << 5)
            | ((self.monochrome as u8) << 4)
            | ((self.chroma_subsampling_x as u8) << 3)
            | ((self.chroma_subsampling_y as u8) << 2)
            | (self.chroma_sample_position & 3));
        b.u8(match self.initial_presentation_delay_minus_one {
            Some(d) => 0x10 | (d & 0x0F),
            None => 0,
        });
        b.bytes(&self.config_obus);
        b.buf
    }
}
