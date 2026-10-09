//! Expanding a track sample table into timestamped sample locations.

use super::{MAX_SAMPLES, Sample, SampleGroup, Track};
use crate::isobmff::bytes::Cur;
use crate::isobmff::bytes::FourCc;
use crate::isobmff::bytes::boxes;
use crate::isobmff::codec::CodecConfig;
use crate::isobmff::codec::parse_sample_entry;
use crate::isobmff::error::Error;
use crate::isobmff::error::Result;
use crate::isobmff::error::invalid;
use std::collections::HashMap;

struct SttsCursor<'a> {
    runs: &'a [(u32, u32)],
    run: usize,
    used: u32,
}

impl SttsCursor<'_> {
    /// Advance by `n` samples, returning the summed duration.
    fn take(&mut self, mut n: u64) -> u64 {
        let mut total = 0u64;
        while n > 0 && self.run < self.runs.len() {
            let (count, delta) = self.runs[self.run];
            let avail = (count - self.used) as u64;
            let k = avail.min(n);
            total = total.saturating_add(k.saturating_mul(delta as u64));
            n -= k;
            self.used += k as u32;
            if self.used >= count {
                self.run += 1;
                self.used = 0;
            }
        }
        total
    }
    fn next_value(&mut self) -> u32 {
        while self.run < self.runs.len() && self.runs[self.run].0 == 0 {
            self.run += 1;
        }
        let Some(&(_, d)) = self.runs.get(self.run) else { return 0 };
        self.take(1);
        d
    }
}

pub(super) fn parse_stbl(stbl: &[u8], t: &mut Track, is_qt: bool, file_len: u64) -> Result<()> {
    let mut stts: Vec<(u32, u32)> = Vec::new();
    let mut ctts: Vec<(u32, u32)> = Vec::new();
    let mut stss: Option<Vec<u32>> = None;
    let mut stsc: Vec<(u32, u32, u32)> = Vec::new();
    let mut const_size = 0u32;
    let mut sample_count = 0usize;
    let mut sizes: Vec<u32> = Vec::new();
    let mut chunks: Vec<u64> = Vec::new();
    let mut sbgp: Vec<(FourCc, Vec<(u32, u32)>)> = Vec::new();
    let mut sgpd: HashMap<FourCc, Vec<Vec<u8>>> = HashMap::new();
    for b in boxes(stbl) {
        let b = b?;
        let mut c = Cur::new(b.payload, "stbl child");
        match &b.kind.0 {
            b"stsd" => {
                c.full_header()?;
                let n = c.count(8)?;
                for e in boxes(c.rest()).take(n) {
                    let e = e?;
                    t.entries.push(parse_sample_entry(e.kind, e.payload, t.handler, is_qt)?);
                }
            }
            b"stts" => {
                c.full_header()?;
                let n = c.count(8)?;
                stts = (0..n).map(|_| Ok((c.u32()?, c.u32()?))).collect::<Result<_>>()?;
            }
            b"ctts" => {
                c.full_header()?;
                let n = c.count(8)?;
                ctts = (0..n).map(|_| Ok((c.u32()?, c.u32()?))).collect::<Result<_>>()?;
            }
            b"stss" => {
                c.full_header()?;
                let n = c.count(4)?;
                let mut v: Vec<u32> = (0..n).map(|_| c.u32()).collect::<Result<_>>()?;
                v.sort_unstable();
                stss = Some(v);
            }
            b"stsc" => {
                c.full_header()?;
                let n = c.count(12)?;
                stsc = (0..n).map(|_| Ok((c.u32()?, c.u32()?, c.u32()?))).collect::<Result<_>>()?;
            }
            b"stsz" => {
                c.full_header()?;
                const_size = c.u32()?;
                let n = c.u32()? as usize;
                if const_size == 0 {
                    c.check_count(n, 4)?;
                    sizes = (0..n).map(|_| c.u32()).collect::<Result<_>>()?;
                    sample_count = n;
                } else {
                    // Samples of constant size must fit in the file: clamp absurd counts.
                    sample_count = (n as u64).min(file_len / const_size as u64) as usize;
                }
            }
            b"stz2" => {
                c.full_header()?;
                let field = c.u32()? & 0xFF;
                let n = c.u32()? as usize;
                let bits = match field {
                    4 | 8 | 16 => field as usize,
                    _ => return invalid(format!("stz2 field size {field}")),
                };
                if n.checked_mul(bits).is_none_or(|b| b.div_ceil(8) > c.remaining()) {
                    return Err(Error::Truncated("stz2"));
                }
                let d = c.rest();
                sizes = (0..n)
                    .map(|i| match bits {
                        4 => ((d[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 0xF) as u32,
                        8 => d[i] as u32,
                        _ => u16::from_be_bytes([d[2 * i], d[2 * i + 1]]) as u32,
                    })
                    .collect();
                sample_count = n;
            }
            b"stco" => {
                c.full_header()?;
                let n = c.count(4)?;
                chunks = (0..n).map(|_| c.u32().map(|v| v as u64)).collect::<Result<_>>()?;
            }
            b"co64" => {
                c.full_header()?;
                let n = c.count(8)?;
                chunks = (0..n).map(|_| c.u64()).collect::<Result<_>>()?;
            }
            b"sdtp" => {
                c.full_header()?;
                t.sdtp = c.rest().to_vec();
            }
            b"sbgp" => {
                let (v, _) = c.full_header()?;
                let gt = c.fourcc()?;
                if v == 1 {
                    c.u32()?;
                }
                let n = c.count(8)?;
                let runs = (0..n).map(|_| Ok((c.u32()?, c.u32()?))).collect::<Result<_>>()?;
                sbgp.push((gt, runs));
            }
            b"sgpd" => {
                let (v, _) = c.full_header()?;
                let gt = c.fourcc()?;
                let default_len = if v >= 1 { c.u32()? } else { 0 };
                if v >= 2 {
                    c.u32()?;
                }
                let n = c.count(0)?;
                let mut descs = Vec::new();
                for _ in 0..n {
                    let l = if v >= 1 && default_len == 0 { c.u32()? } else { default_len };
                    if l == 0 {
                        // Version 0 without known length: keep the remainder as a single entry.
                        descs.push(c.rest().to_vec());
                        break;
                    }
                    descs.push(c.bytes(l as usize)?.to_vec());
                }
                sgpd.insert(gt, descs);
            }
            _ => {}
        }
    }
    t.sample_groups =
        sbgp.into_iter().map(|(gt, runs)| SampleGroup { grouping_type: gt, runs, descriptions: sgpd.remove(&gt).unwrap_or_default() }).collect();
    if sample_count == 0 || chunks.is_empty() || stsc.is_empty() {
        return Ok(());
    }
    if stsc[0].0 == 0 || stsc.windows(2).any(|w| w[1].0 <= w[0].0) {
        return invalid("stsc first_chunk values are not increasing from 1");
    }
    let pcm = match t.entries.first().map(|e| &e.codec) {
        Some(CodecConfig::Pcm(p)) if const_size != 0 => Some(*p),
        _ => None,
    };
    let mut stts_c = SttsCursor { runs: &stts, run: 0, used: 0 };
    let mut ctts_c = SttsCursor { runs: &ctts, run: 0, used: 0 };
    let mut samples = Vec::new();
    let mut sidx = 0usize;
    let mut dts = 0i64;
    let mut stsc_i = 0usize;
    if let Some(p) = pcm {
        t.pcm_chunked = true;
        let a = t.audio().cloned().unwrap_or_default();
        let frame_bytes = |frames: u64| -> u64 {
            if a.qt_version == 1 && a.bytes_per_frame > 0 && a.samples_per_packet > 0 {
                frames / a.samples_per_packet as u64 * a.bytes_per_frame as u64
            } else if const_size > 1 {
                frames * const_size as u64
            } else {
                frames * p.bytes_per_frame() as u64
            }
        };
        if chunks.len() > MAX_SAMPLES {
            return Err(Error::TooLarge("chunk count"));
        }
        samples.reserve(chunks.len());
        for (ci, &off) in chunks.iter().enumerate() {
            while stsc_i + 1 < stsc.len() && stsc[stsc_i + 1].0 as usize <= ci + 1 {
                stsc_i += 1;
            }
            let spc = (stsc[stsc_i].1 as usize).min(sample_count - sidx);
            if spc == 0 {
                continue;
            }
            let dur = stts_c.take(spc as u64);
            let size = frame_bytes(spc as u64).min(u32::MAX as u64) as u32;
            samples.push(Sample {
                offset: off,
                size,
                dts,
                pts: dts,
                duration: dur.min(u32::MAX as u64) as u32,
                is_sync: true,
                description_index: stsc[stsc_i].2,
            });
            dts = dts.saturating_add(dur.min(i64::MAX as u64) as i64);
            sidx += spc;
            if sidx >= sample_count {
                break;
            }
        }
    } else {
        if sample_count > MAX_SAMPLES {
            return Err(Error::TooLarge("sample count"));
        }
        samples.reserve(sample_count);
        let mut stss_i = 0usize;
        'chunks: for (ci, &off) in chunks.iter().enumerate() {
            while stsc_i + 1 < stsc.len() && stsc[stsc_i + 1].0 as usize <= ci + 1 {
                stsc_i += 1;
            }
            let (_, spc, sdi) = stsc[stsc_i];
            let mut pos = off;
            for _ in 0..spc {
                if sidx >= sample_count {
                    break 'chunks;
                }
                let size = if const_size != 0 { const_size } else { sizes[sidx] };
                let duration = stts_c.next_value();
                let cts = ctts_c.next_value() as i32 as i64;
                let is_sync = match &stss {
                    None => true,
                    Some(v) => {
                        while stss_i < v.len() && (v[stss_i] as usize) < sidx + 1 {
                            stss_i += 1;
                        }
                        stss_i < v.len() && v[stss_i] as usize == sidx + 1
                    }
                };
                samples.push(Sample { offset: pos, size, dts, pts: dts.saturating_add(cts), duration, is_sync, description_index: sdi });
                pos = pos.saturating_add(size as u64);
                dts = dts.saturating_add(duration as i64);
                sidx += 1;
            }
        }
    }
    t.samples = samples;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SttsCursor;

    #[test]
    fn stts_cursor() {
        let runs = [(2, 10), (0, 99), (3, 5)];
        let mut c = SttsCursor { runs: &runs, run: 0, used: 0 };
        assert_eq!(c.next_value(), 10);
        assert_eq!(c.take(3), 10 + 5 + 5);
        assert_eq!(c.next_value(), 5);
        assert_eq!(c.next_value(), 0);
    }
}
