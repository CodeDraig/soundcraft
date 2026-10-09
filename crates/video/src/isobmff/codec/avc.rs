//! AVC decoder configuration records.

use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Result;

/// `avcC` — AVCDecoderConfigurationRecord (ISO/IEC 14496-15 §5.3.3).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AvcConfig {
    pub profile: u8,
    pub compatibility: u8,
    pub level: u8,
    /// NAL length field size in bytes (1, 2 or 4).
    pub length_size: u8,
    pub sps: Vec<Vec<u8>>,
    pub pps: Vec<Vec<u8>>,
    /// Bytes after the PPS list (high-profile extension: chroma format, bit depths, SPS-ext).
    pub ext: Vec<u8>,
}

impl AvcConfig {
    /// Build from parameter sets (NAL units without start codes); profile/level taken from the first SPS.
    pub fn new(sps: Vec<Vec<u8>>, pps: Vec<Vec<u8>>, length_size: u8) -> Self {
        let (profile, compatibility, level) = sps.first().filter(|s| s.len() >= 4).map(|s| (s[1], s[2], s[3])).unwrap_or((0, 0, 0));
        AvcConfig { profile, compatibility, level, length_size, sps, pps, ext: Vec::new() }
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur::new(data, "avcC");
        let _version = c.u8()?;
        let profile = c.u8()?;
        let compatibility = c.u8()?;
        let level = c.u8()?;
        let length_size = (c.u8()? & 3) + 1;
        let nsps = (c.u8()? & 0x1F) as usize;
        let mut sps = Vec::with_capacity(nsps);
        for _ in 0..nsps {
            let n = c.u16()? as usize;
            sps.push(c.bytes(n)?.to_vec());
        }
        let npps = c.u8()? as usize;
        let mut pps = Vec::with_capacity(npps);
        for _ in 0..npps {
            let n = c.u16()? as usize;
            pps.push(c.bytes(n)?.to_vec());
        }
        Ok(AvcConfig { profile, compatibility, level, length_size, sps, pps, ext: c.rest().to_vec() })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u8(1);
        b.u8(self.profile);
        b.u8(self.compatibility);
        b.u8(self.level);
        b.u8(0xFC | (self.length_size.clamp(1, 4) - 1));
        b.u8(0xE0 | (self.sps.len().min(31) as u8));
        for s in self.sps.iter().take(31) {
            b.u16(s.len() as u16);
            b.bytes(s);
        }
        b.u8(self.pps.len().min(255) as u8);
        for p in self.pps.iter().take(255) {
            b.u16(p.len() as u16);
            b.bytes(p);
        }
        b.bytes(&self.ext);
        b.buf
    }
}
