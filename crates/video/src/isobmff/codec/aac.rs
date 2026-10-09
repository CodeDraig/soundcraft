//! AAC AudioSpecificConfig and ES descriptors.

use super::CodecConfig;
use crate::bitstream::BitReader;
use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::bytes::Cur;
use crate::isobmff::error::Error;
use crate::isobmff::error::Result;

/// AAC (MPEG-4 audio) configuration from `esds`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AacConfig {
    /// AudioSpecificConfig bytes (DecoderSpecificInfo).
    pub asc: Vec<u8>,
    /// Audio object type (2 = AAC-LC, 5 = SBR/HE-AAC, 29 = PS …) as signalled first in the ASC.
    pub object_type: u8,
    /// Sampling rate signalled by the ASC (core rate; 0 if the ASC is missing).
    pub sample_rate: u32,
    /// Channel configuration from the ASC (0 = defined elsewhere).
    pub channel_config: u8,
    /// DecoderConfigDescriptor fields.
    pub max_bitrate: u32,
    pub avg_bitrate: u32,
    pub buffer_size: u32,
}

const AAC_RATES: [u32; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];

impl AacConfig {
    /// Build from an AudioSpecificConfig, decoding its header fields.
    pub fn from_asc(asc: Vec<u8>) -> Self {
        let (object_type, sample_rate, channel_config) = parse_asc(&asc).unwrap_or((0, 0, 0));
        AacConfig { asc, object_type, sample_rate, channel_config, max_bitrate: 0, avg_bitrate: 0, buffer_size: 0 }
    }
    /// Channel count implied by the channel configuration (0 if unspecified).
    pub fn channels(&self) -> u32 {
        match self.channel_config {
            1..=6 => self.channel_config as u32,
            7 => 8,
            _ => 0,
        }
    }
}

/// Decode (audio object type, sampling frequency, channel configuration) from an AudioSpecificConfig.
pub fn parse_asc(asc: &[u8]) -> Option<(u8, u32, u8)> {
    let mut r = BitReader::new(asc);
    let aot = |r: &mut BitReader| -> Option<u8> {
        let t = r.read_bits(5).ok()? as u8;
        if t == 31 { Some(32 + r.read_bits(6).ok()? as u8) } else { Some(t) }
    };
    let rate = |r: &mut BitReader| -> Option<u32> {
        let i = r.read_bits(4).ok()? as usize;
        if i == 15 { r.read_bits(24).ok() } else { AAC_RATES.get(i).copied() }
    };
    let t = aot(&mut r)?;
    let sr = rate(&mut r)?;
    let ch = r.read_bits(4).ok()? as u8;
    Some((t, sr, ch))
}

fn descriptor(c: &mut Cur) -> Result<(u8, usize)> {
    let tag = c.u8()?;
    let mut len = 0usize;
    for _ in 0..4 {
        let b = c.u8()?;
        len = (len << 7) | (b & 0x7F) as usize;
        if b & 0x80 == 0 {
            break;
        }
    }
    Ok((tag, len))
}

pub(super) fn parse_esds(p: &[u8]) -> Result<CodecConfig> {
    let mut c = Cur::new(p, "esds");
    c.full_header()?;
    let (tag, len) = descriptor(&mut c)?;
    if tag != 3 {
        return Err(Error::Invalid(format!("esds: expected ES_Descriptor, got tag {tag}")));
    }
    let es = c.bytes(len.min(c.remaining()))?;
    let mut c = Cur::new(es, "ES_Descriptor");
    c.u16()?;
    let fl = c.u8()?;
    if fl & 0x80 != 0 {
        c.u16()?;
    }
    if fl & 0x40 != 0 {
        let n = c.u8()? as usize;
        c.skip(n)?;
    }
    if fl & 0x20 != 0 {
        c.u16()?;
    }
    while c.remaining() > 0 {
        let (tag, len) = descriptor(&mut c)?;
        let body = c.bytes(len.min(c.remaining()))?;
        if tag != 4 {
            continue;
        }
        let mut d = Cur::new(body, "DecoderConfigDescriptor");
        let oti = d.u8()?;
        let _stream_type = d.u8()?;
        let buffer_size = d.u24()?;
        let max_bitrate = d.u32()?;
        let avg_bitrate = d.u32()?;
        let mut asc = Vec::new();
        while d.remaining() > 0 {
            let (t, l) = descriptor(&mut d)?;
            let b = d.bytes(l.min(d.remaining()))?;
            if t == 5 {
                asc = b.to_vec();
                break;
            }
        }
        return Ok(match oti {
            0x69 | 0x6B => CodecConfig::Mp3,
            _ => {
                let mut a = AacConfig::from_asc(asc);
                a.max_bitrate = max_bitrate;
                a.avg_bitrate = avg_bitrate;
                a.buffer_size = buffer_size;
                CodecConfig::Aac(a)
            }
        });
    }
    Err(Error::Invalid("esds without DecoderConfigDescriptor".into()))
}

// ---------------------------------------------------------------------------------------------
// Serialization (muxer)

fn put_descriptor(b: &mut BoxBuf, tag: u8, body: &[u8]) {
    b.u8(tag);
    let n = body.len() as u32;
    // Always use the 4-byte length form for simplicity (as many encoders do).
    b.u8(0x80 | ((n >> 21) & 0x7F) as u8);
    b.u8(0x80 | ((n >> 14) & 0x7F) as u8);
    b.u8(0x80 | ((n >> 7) & 0x7F) as u8);
    b.u8((n & 0x7F) as u8);
    b.bytes(body);
}

pub(crate) fn esds_payload(oti: u8, asc: &[u8], max_bitrate: u32, avg_bitrate: u32, buffer_size: u32) -> Vec<u8> {
    let mut dcd = BoxBuf::new();
    dcd.u8(oti);
    dcd.u8(0x15); // streamType audio (5) << 2 | upstream 0 | reserved 1
    dcd.u8((buffer_size >> 16) as u8);
    dcd.u16(buffer_size as u16);
    dcd.u32(max_bitrate);
    dcd.u32(avg_bitrate);
    if !asc.is_empty() {
        put_descriptor(&mut dcd, 5, asc);
    }
    let mut es = BoxBuf::new();
    es.u16(1); // ES_ID
    es.u8(0);
    put_descriptor(&mut es, 4, &dcd.buf);
    put_descriptor(&mut es, 6, &[2]); // SLConfigDescriptor, predefined = 2 (MP4)
    let mut b = BoxBuf::new();
    b.u32(0);
    put_descriptor(&mut b, 3, &es.buf);
    b.buf
}
