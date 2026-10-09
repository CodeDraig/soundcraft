//! HEVC decoder configuration records and NAL arrays.

use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Result;

/// One NAL array of an `hvcC` record.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HevcNalArray {
    pub completeness: bool,
    pub nal_type: u8,
    pub nalus: Vec<Vec<u8>>,
}

/// `hvcC` — HEVCDecoderConfigurationRecord (ISO/IEC 14496-15 §8.3.3).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HevcConfig {
    pub general_profile_space: u8,
    pub general_tier_flag: bool,
    pub general_profile_idc: u8,
    pub general_profile_compatibility_flags: u32,
    /// 48 bits of general constraint indicator flags.
    pub general_constraint_indicator_flags: u64,
    pub general_level_idc: u8,
    pub min_spatial_segmentation_idc: u16,
    pub parallelism_type: u8,
    pub chroma_format_idc: u8,
    pub bit_depth_luma: u8,
    pub bit_depth_chroma: u8,
    pub avg_frame_rate: u16,
    pub constant_frame_rate: u8,
    pub num_temporal_layers: u8,
    pub temporal_id_nested: bool,
    pub length_size: u8,
    pub arrays: Vec<HevcNalArray>,
}

impl HevcConfig {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur::new(data, "hvcC");
        let _version = c.u8()?;
        let b = c.u8()?;
        let compat = c.u32()?;
        let hi = c.u16()? as u64;
        let lo = c.u32()? as u64;
        let level = c.u8()?;
        let min_seg = c.u16()? & 0x0FFF;
        let par = c.u8()? & 3;
        let chroma = c.u8()? & 3;
        let bdl = (c.u8()? & 7) + 8;
        let bdc = (c.u8()? & 7) + 8;
        let afr = c.u16()?;
        let x = c.u8()?;
        let narrays = c.u8()? as usize;
        let mut arrays = Vec::new();
        for _ in 0..narrays {
            let t = c.u8()?;
            let n = c.u16()? as usize;
            c.check_count(n, 2)?;
            let mut nalus = Vec::with_capacity(n);
            for _ in 0..n {
                let len = c.u16()? as usize;
                nalus.push(c.bytes(len)?.to_vec());
            }
            arrays.push(HevcNalArray { completeness: t & 0x80 != 0, nal_type: t & 0x3F, nalus });
        }
        Ok(HevcConfig {
            general_profile_space: b >> 6,
            general_tier_flag: b & 0x20 != 0,
            general_profile_idc: b & 0x1F,
            general_profile_compatibility_flags: compat,
            general_constraint_indicator_flags: (hi << 32) | lo,
            general_level_idc: level,
            min_spatial_segmentation_idc: min_seg,
            parallelism_type: par,
            chroma_format_idc: chroma,
            bit_depth_luma: bdl,
            bit_depth_chroma: bdc,
            avg_frame_rate: afr,
            constant_frame_rate: x >> 6,
            num_temporal_layers: (x >> 3) & 7,
            temporal_id_nested: x & 4 != 0,
            length_size: (x & 3) + 1,
            arrays,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u8(1);
        b.u8((self.general_profile_space << 6) | ((self.general_tier_flag as u8) << 5) | (self.general_profile_idc & 0x1F));
        b.u32(self.general_profile_compatibility_flags);
        b.u16((self.general_constraint_indicator_flags >> 32) as u16);
        b.u32(self.general_constraint_indicator_flags as u32);
        b.u8(self.general_level_idc);
        b.u16(0xF000 | (self.min_spatial_segmentation_idc & 0x0FFF));
        b.u8(0xFC | (self.parallelism_type & 3));
        b.u8(0xFC | (self.chroma_format_idc & 3));
        b.u8(0xF8 | (self.bit_depth_luma.saturating_sub(8) & 7));
        b.u8(0xF8 | (self.bit_depth_chroma.saturating_sub(8) & 7));
        b.u16(self.avg_frame_rate);
        b.u8((self.constant_frame_rate << 6)
            | ((self.num_temporal_layers & 7) << 3)
            | ((self.temporal_id_nested as u8) << 2)
            | (self.length_size.clamp(1, 4) - 1));
        b.u8(self.arrays.len() as u8);
        for a in &self.arrays {
            b.u8(((a.completeness as u8) << 7) | (a.nal_type & 0x3F));
            b.u16(a.nalus.len() as u16);
            for n in &a.nalus {
                b.u16(n.len() as u16);
                b.bytes(n);
            }
        }
        b.buf
    }

    fn nalus_of(&self, t: u8) -> Vec<&[u8]> {
        self.arrays.iter().filter(|a| a.nal_type == t).flat_map(|a| a.nalus.iter().map(|n| n.as_slice())).collect()
    }
    pub fn vps(&self) -> Vec<&[u8]> {
        self.nalus_of(32)
    }
    pub fn sps(&self) -> Vec<&[u8]> {
        self.nalus_of(33)
    }
    pub fn pps(&self) -> Vec<&[u8]> {
        self.nalus_of(34)
    }
}
