//! QuickTime PCM sample descriptions.

use super::AudioParams;

/// Uncompressed PCM description.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PcmConfig {
    pub bits: u16,
    pub float: bool,
    pub big_endian: bool,
    /// Signed integer samples (8-bit `raw ` is unsigned).
    pub signed: bool,
    pub channels: u32,
    pub sample_rate: f64,
}

impl PcmConfig {
    /// Bytes per interleaved audio frame (all channels).
    pub fn bytes_per_frame(&self) -> u32 {
        (self.bits as u32).div_ceil(8) * self.channels.max(1)
    }
}

pub(super) fn qt_pcm(f: &[u8; 4], a: &AudioParams) -> PcmConfig {
    let mut p = PcmConfig { channels: a.channels, sample_rate: a.sample_rate, signed: true, ..Default::default() };
    match f {
        b"lpcm" => {
            p.bits = a.const_bits_per_channel as u16;
            p.float = a.lpcm_flags & 1 != 0;
            p.big_endian = a.lpcm_flags & 2 != 0;
            p.signed = a.lpcm_flags & 4 != 0 || p.float;
            if p.bits == 0 {
                p.bits = a.sample_size;
            }
        }
        b"sowt" => {
            p.bits = if a.sample_size == 0 { 16 } else { a.sample_size };
        }
        b"twos" => {
            p.bits = if a.sample_size == 0 { 16 } else { a.sample_size };
            p.big_endian = true;
        }
        b"raw " | b"NONE" => {
            p.bits = if a.sample_size == 0 { 8 } else { a.sample_size };
            p.signed = p.bits != 8;
            p.big_endian = true;
        }
        b"in24" | b"in32" | b"fl32" | b"fl64" => {
            p.bits = match f {
                b"in24" => 24,
                b"in32" | b"fl32" => 32,
                _ => 64,
            };
            p.float = f[0] == b'f';
            p.big_endian = !a.little_endian.unwrap_or(false);
        }
        _ => {}
    }
    p
}
