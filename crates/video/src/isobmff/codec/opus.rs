//! Opus decoder configuration records.

use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Result;

/// `dOps` — Opus specific box.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct OpusConfig {
    pub output_channels: u8,
    pub pre_skip: u16,
    pub input_sample_rate: u32,
    pub output_gain: i16,
    pub channel_mapping_family: u8,
    /// StreamCount, CoupledCount and ChannelMapping bytes when family != 0.
    pub channel_mapping: Vec<u8>,
}

impl OpusConfig {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut c = Cur::new(data, "dOps");
        let _version = c.u8()?;
        Ok(OpusConfig {
            output_channels: c.u8()?,
            pre_skip: c.u16()?,
            input_sample_rate: c.u32()?,
            output_gain: c.i16()?,
            channel_mapping_family: c.u8()?,
            channel_mapping: c.rest().to_vec(),
        })
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u8(0);
        b.u8(self.output_channels);
        b.u16(self.pre_skip);
        b.u32(self.input_sample_rate);
        b.i16(self.output_gain);
        b.u8(self.channel_mapping_family);
        b.bytes(&self.channel_mapping);
        b.buf
    }
}
