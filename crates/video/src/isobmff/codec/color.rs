//! Color description and mastering-display metadata.

use crate::isobmff::bytes::Cur;
use crate::isobmff::bytes::FourCc;
use crate::isobmff::error::Result;

/// SMPTE ST 2086 mastering display metadata, in the units of the `mdcv` box and the H.264/HEVC
/// SEI: chromaticities in 0.00002 steps (order G, B, R as the SEI), luminance in 0.0001 cd/m².
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasteringDisplay {
    pub primaries: [(u16, u16); 3],
    pub white_point: (u16, u16),
    pub max_luminance: u32,
    pub min_luminance: u32,
}

impl MasteringDisplay {
    /// BT.2020 primaries, D65, with the given peak and black in cd/m².
    pub fn bt2020(max_nits: f64, min_nits: f64) -> Self {
        let q = |v: f64| (v / 0.00002).round() as u16;
        MasteringDisplay {
            primaries: [(q(0.170), q(0.797)), (q(0.131), q(0.046)), (q(0.708), q(0.292))],
            white_point: (q(0.3127), q(0.3290)),
            max_luminance: (max_nits * 10_000.0).round() as u32,
            min_luminance: (min_nits * 10_000.0).round() as u32,
        }
    }
    pub fn max_nits(&self) -> f64 {
        self.max_luminance as f64 / 10_000.0
    }
    /// The 24-byte payload shared by `mdcv` and the SEI message.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24);
        for (x, y) in self.primaries {
            v.extend_from_slice(&x.to_be_bytes());
            v.extend_from_slice(&y.to_be_bytes());
        }
        v.extend_from_slice(&self.white_point.0.to_be_bytes());
        v.extend_from_slice(&self.white_point.1.to_be_bytes());
        v.extend_from_slice(&self.max_luminance.to_be_bytes());
        v.extend_from_slice(&self.min_luminance.to_be_bytes());
        v
    }
    pub fn parse(p: &[u8]) -> Option<Self> {
        if p.len() < 24 {
            return None;
        }
        let u16_at = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
        let u32_at = |i: usize| u32::from_be_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]]);
        Some(MasteringDisplay {
            primaries: [(u16_at(0), u16_at(2)), (u16_at(4), u16_at(6)), (u16_at(8), u16_at(10))],
            white_point: (u16_at(12), u16_at(14)),
            max_luminance: u32_at(16),
            min_luminance: u32_at(20),
        })
    }
}

/// `colr` box contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorInfo {
    /// ISO `nclx` (with full-range flag).
    Nclx { primaries: u16, transfer: u16, matrix: u16, full_range: bool },
    /// QuickTime `nclc`.
    Nclc { primaries: u16, transfer: u16, matrix: u16 },
    /// Embedded ICC profile (`prof` restricted/unrestricted `rICC`).
    Icc { kind: FourCc, profile: Vec<u8> },
}

pub(super) fn parse_colr(p: &[u8]) -> Result<ColorInfo> {
    let mut c = Cur::new(p, "colr");
    let kind = c.fourcc()?;
    Ok(match &kind.0 {
        b"nclx" => {
            ColorInfo::Nclx { primaries: c.u16()?, transfer: c.u16()?, matrix: c.u16()?, full_range: c.u8().map(|v| v & 0x80 != 0).unwrap_or(false) }
        }
        b"nclc" => ColorInfo::Nclc { primaries: c.u16()?, transfer: c.u16()?, matrix: c.u16()? },
        _ => ColorInfo::Icc { kind, profile: c.rest().to_vec() },
    })
}
