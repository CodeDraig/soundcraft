//! VP8/VP9 codec configuration records.

use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Result;

/// `vpcC` — VP codec configuration (version 1).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct VpcConfig {
    pub profile: u8,
    pub level: u8,
    pub bit_depth: u8,
    pub chroma_subsampling: u8,
    pub full_range: bool,
    pub colour_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub codec_init: Vec<u8>,
}

impl VpcConfig {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur::new(data, "vpcC");
        let (version, _) = c.full_header()?;
        if version == 0 {
            // Version 0 draft layout: profile, level, bitDepth(4)|colorSpace(4), chroma(4)|transfer(3)|range(1)
            let profile = c.u8()?;
            let level = c.u8()?;
            let a = c.u8()?;
            let b = c.u8()?;
            return Ok(VpcConfig { profile, level, bit_depth: a >> 4, chroma_subsampling: b >> 4, full_range: b & 1 != 0, ..Default::default() });
        }
        let profile = c.u8()?;
        let level = c.u8()?;
        let a = c.u8()?;
        let colour_primaries = c.u8()?;
        let transfer_characteristics = c.u8()?;
        let matrix_coefficients = c.u8()?;
        let n = c.u16()? as usize;
        let codec_init = c.bytes(n)?.to_vec();
        Ok(VpcConfig {
            profile,
            level,
            bit_depth: a >> 4,
            chroma_subsampling: (a >> 1) & 7,
            full_range: a & 1 != 0,
            colour_primaries,
            transfer_characteristics,
            matrix_coefficients,
            codec_init,
        })
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u32(1 << 24);
        b.u8(self.profile);
        b.u8(self.level);
        b.u8((self.bit_depth << 4) | ((self.chroma_subsampling & 7) << 1) | self.full_range as u8);
        b.u8(self.colour_primaries);
        b.u8(self.transfer_characteristics);
        b.u8(self.matrix_coefficients);
        b.u16(self.codec_init.len() as u16);
        b.bytes(&self.codec_init);
        b.buf
    }
}
