//! Muxers: progressive MP4/MOV ([`Mp4Writer`], optional faststart) and fragmented MP4 ([`FragmentedWriter`]).

use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::codec::{CodecConfig, SampleEntry};
use crate::isobmff::demux::Edit;
use crate::isobmff::error::{Error, Result};

mod progressive;
pub use progressive::Mp4Writer;
mod fragmented;
pub use fragmented::FragmentedWriter;

/// Output flavour: determines brands and some sample-entry encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brand {
    /// ISO MP4: `isom` major brand, compatible `isom iso2 avc1 mp41 mp42`.
    Mp4,
    /// QuickTime MOV: `qt  ` major brand.
    Mov,
}

/// Movie-level writer options.
#[derive(Clone, Debug)]
pub struct WriterOptions {
    pub brand: Brand,
    /// Movie timescale (`mvhd`); default 1000.
    pub movie_timescale: u32,
    /// Text metadata written to `moov/udta` (keys like `©nam`, `©ART`, `©cmt`, `©too`).
    pub metadata: Vec<(String, String)>,
}

impl WriterOptions {
    pub fn new(brand: Brand) -> Self {
        WriterOptions { brand, movie_timescale: 1000, metadata: Vec::new() }
    }
}

/// Per-track configuration.
#[derive(Clone, Debug)]
pub struct TrackConfig {
    pub entry: SampleEntry,
    /// Media timescale. For PCM it must equal the sample rate (one tick per audio frame).
    pub timescale: u32,
    /// ISO 639-2/T language (`und` by default).
    pub language: String,
    /// Explicit edit list (segment durations in the movie timescale). Takes precedence over `media_start`.
    pub edits: Vec<Edit>,
    /// Convenience: create one edit starting at this media time (e.g. AAC encoder delay / priming,
    /// or the first pts of a B-frame stream) spanning the rest of the media.
    pub media_start: Option<i64>,
    /// `hdlr` name; a sensible default is used when `None`.
    pub handler_name: Option<String>,
}

impl TrackConfig {
    pub fn new(entry: SampleEntry, timescale: u32) -> Self {
        TrackConfig { entry, timescale, language: "und".into(), edits: Vec::new(), media_start: None, handler_name: None }
    }
}

/// A sample handed to the muxers.
#[derive(Clone, Copy, Debug)]
pub struct WriteSample<'a> {
    pub data: &'a [u8],
    /// Duration in the media timescale. Ignored for PCM (each frame lasts one tick).
    pub duration: u32,
    /// pts − dts in the media timescale.
    pub composition_offset: i32,
    pub is_sync: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Handler {
    Video,
    Audio,
    Timecode,
    Other,
}

fn handler_of(e: &SampleEntry) -> Handler {
    if matches!(e.codec, CodecConfig::Timecode(_)) {
        Handler::Timecode
    } else if e.video.is_some() {
        Handler::Video
    } else if e.audio.is_some() {
        Handler::Audio
    } else {
        Handler::Other
    }
}

/// Accumulated sample tables for one track.
#[derive(Clone, Debug, Default)]
struct Tables {
    stts: Vec<(u32, u32)>,
    ctts: Vec<(u32, i32)>,
    sizes: Vec<u32>,
    /// Constant sample size (PCM) — then `sizes` is unused.
    const_size: Option<u32>,
    sample_count: u64,
    /// 1-based sync sample numbers.
    sync: Vec<u32>,
    all_sync: bool,
    /// (offset relative to file start before shifting, sample count)
    chunks: Vec<(u64, u32)>,
    media_duration: u64,
    /// End of the last sample on the composition timeline (decode time + composition offset +
    /// duration, maximum over samples).
    comp_end: u64,
}

impl Tables {
    fn new() -> Self {
        Tables { all_sync: true, ..Default::default() }
    }
    fn push_run<T: PartialEq + Copy>(runs: &mut Vec<(u32, T)>, count: u32, v: T) {
        if let Some(last) = runs.last_mut()
            && last.1 == v
            && last.0.checked_add(count).is_some()
        {
            last.0 += count;
            return;
        }
        runs.push((count, v));
    }
}

struct WTrack {
    cfg: TrackConfig,
    id: u32,
    handler: Handler,
    pcm_frame_bytes: Option<u32>,
    t: Tables,
    /// Track id of a linked timecode track (`tref/tmcd`).
    tmcd_ref: Option<u32>,
    /// For timecode tracks: (video track index, start frame).
    tmcd_source: Option<(usize, u32)>,
}

fn pack_language(lang: &str) -> u16 {
    let b = lang.as_bytes();
    if b.len() == 3 && b.iter().all(|c| c.is_ascii_lowercase()) {
        (((b[0] - 0x60) as u16) << 10) | (((b[1] - 0x60) as u16) << 5) | ((b[2] - 0x60) as u16)
    } else {
        0x55C4 // "und"
    }
}

fn rescale(v: u64, from: u32, to: u32) -> u64 {
    if from == 0 {
        return 0;
    }
    (v as u128 * to as u128 / from as u128).min(u64::MAX as u128) as u64
}

const MATRIX: [i32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000];

fn write_ftyp(b: &mut BoxBuf, brand: Brand) {
    let m = b.start(b"ftyp");
    match brand {
        Brand::Mp4 => {
            b.bytes(b"isom");
            b.u32(0x200);
            for c in [b"isom", b"iso2", b"avc1", b"mp41", b"mp42"] {
                b.bytes(c);
            }
        }
        Brand::Mov => {
            b.bytes(b"qt  ");
            b.u32(0x200);
            b.bytes(b"qt  ");
        }
    }
    b.end(m);
}

fn movie_duration(tracks: &[WTrack], movie_ts: u32) -> u64 {
    tracks.iter().map(|t| presentation_duration(t, movie_ts)).max().unwrap_or(0)
}

fn effective_edits(t: &WTrack, movie_ts: u32) -> Vec<Edit> {
    if !t.cfg.edits.is_empty() {
        return t.cfg.edits.clone();
    }
    if let Some(start) = t.cfg.media_start {
        // presentation runs from `start` to the end of the last sample in composition order (with
        // B-frame reordering that is past the end of the decode timeline)
        let rest = t.t.comp_end.max(t.t.media_duration).saturating_sub(start.max(0) as u64);
        return vec![Edit { segment_duration: rescale(rest, t.cfg.timescale, movie_ts), media_time: start, media_rate: 0x10000 }];
    }
    Vec::new()
}

fn presentation_duration(t: &WTrack, movie_ts: u32) -> u64 {
    let e = effective_edits(t, movie_ts);
    if e.is_empty() { rescale(t.t.media_duration, t.cfg.timescale, movie_ts) } else { e.iter().map(|e| e.segment_duration).sum() }
}

fn write_mvhd(b: &mut BoxBuf, timescale: u32, duration: u64, next_id: u32) {
    let v1 = duration > u32::MAX as u64;
    let m = b.start_full(b"mvhd", v1 as u8, 0);
    if v1 {
        b.u64(0);
        b.u64(0);
        b.u32(timescale);
        b.u64(duration);
    } else {
        b.u32(0);
        b.u32(0);
        b.u32(timescale);
        b.u32(duration as u32);
    }
    b.u32(0x10000);
    b.u16(0x100);
    b.zeros(10);
    for v in MATRIX {
        b.i32(v);
    }
    b.zeros(24);
    b.u32(next_id);
    b.end(m);
}

fn write_trak(b: &mut BoxBuf, t: &WTrack, movie_ts: u32, qt: bool, shift: u64, force_co64: bool) -> Result<()> {
    let trak = b.start(b"trak");
    let dur = presentation_duration(t, movie_ts);
    let v1 = dur > u32::MAX as u64;
    let m = b.start_full(b"tkhd", v1 as u8, 3);
    if v1 {
        b.u64(0);
        b.u64(0);
        b.u32(t.id);
        b.u32(0);
        b.u64(dur);
    } else {
        b.u32(0);
        b.u32(0);
        b.u32(t.id);
        b.u32(0);
        b.u32(dur as u32);
    }
    b.zeros(8);
    b.i16(0);
    b.u16(0);
    b.i16(if t.handler == Handler::Audio { 0x100 } else { 0 });
    b.u16(0);
    for v in MATRIX {
        b.i32(v);
    }
    let (w, h) = match &t.cfg.entry.video {
        Some(v) => {
            let w = match v.pixel_aspect {
                Some((hs, vs)) if hs > 0 && vs > 0 => (v.width as u64 * hs as u64 / vs as u64) as u32,
                _ => v.width as u32,
            };
            (w, v.height as u32)
        }
        None => (0, 0),
    };
    b.u32(w << 16);
    b.u32(h << 16);
    b.end(m);
    let edits = effective_edits(t, movie_ts);
    if !edits.is_empty() {
        let e = b.start(b"edts");
        let v1 = edits.iter().any(|e| e.segment_duration > u32::MAX as u64 || e.media_time > i32::MAX as i64 || e.media_time < i32::MIN as i64);
        let l = b.start_full(b"elst", v1 as u8, 0);
        b.u32(edits.len() as u32);
        for ed in &edits {
            if v1 {
                b.u64(ed.segment_duration);
                b.i64(ed.media_time);
            } else {
                b.u32(ed.segment_duration as u32);
                b.i32(ed.media_time as i32);
            }
            b.i32(ed.media_rate);
        }
        b.end(l);
        b.end(e);
    }
    if let Some(id) = t.tmcd_ref {
        let r = b.start(b"tref");
        let m = b.start(b"tmcd");
        b.u32(id);
        b.end(m);
        b.end(r);
    }
    let mdia = b.start(b"mdia");
    let mv1 = t.t.media_duration > u32::MAX as u64;
    let m = b.start_full(b"mdhd", mv1 as u8, 0);
    if mv1 {
        b.u64(0);
        b.u64(0);
        b.u32(t.cfg.timescale);
        b.u64(t.t.media_duration);
    } else {
        b.u32(0);
        b.u32(0);
        b.u32(t.cfg.timescale);
        b.u32(t.t.media_duration as u32);
    }
    b.u16(pack_language(&t.cfg.language));
    b.u16(0);
    b.end(m);
    let (htype, default_name) = match t.handler {
        Handler::Video => (b"vide", "VideoHandler"),
        Handler::Audio => (b"soun", "SoundHandler"),
        Handler::Timecode => (b"tmcd", "TimeCodeHandler"),
        Handler::Other => (b"meta", "MetaHandler"),
    };
    let name = t.cfg.handler_name.clone().unwrap_or_else(|| default_name.into());
    let m = b.start_full(b"hdlr", 0, 0);
    b.bytes(if qt { b"mhlr" } else { &[0, 0, 0, 0] });
    b.bytes(htype);
    b.zeros(12);
    if qt {
        let n = name.len().min(255);
        b.u8(n as u8);
        b.bytes(&name.as_bytes()[..n]);
    } else {
        b.bytes(name.as_bytes());
        b.u8(0);
    }
    b.end(m);
    let minf = b.start(b"minf");
    match t.handler {
        Handler::Video => {
            let m = b.start_full(b"vmhd", 0, 1);
            b.zeros(8);
            b.end(m);
        }
        Handler::Audio => {
            let m = b.start_full(b"smhd", 0, 0);
            b.zeros(4);
            b.end(m);
        }
        Handler::Timecode => {
            let g = b.start(b"gmhd");
            let m = b.start_full(b"gmin", 0, 0);
            b.u16(0x40);
            b.u16(0x8000);
            b.u16(0x8000);
            b.u16(0x8000);
            b.i16(0);
            b.u16(0);
            b.end(m);
            let tm = b.start(b"tmcd");
            let m = b.start_full(b"tcmi", 0, 0);
            b.u16(0);
            b.u16(0);
            b.u16(12);
            b.u16(0);
            b.bytes(&[0xFF; 6]);
            b.zeros(6);
            b.u8(0);
            b.end(m);
            b.end(tm);
            b.end(g);
        }
        Handler::Other => {
            let m = b.start_full(b"nmhd", 0, 0);
            b.end(m);
        }
    }
    if qt {
        // QuickTime data handler reference.
        let m = b.start_full(b"hdlr", 0, 0);
        b.bytes(b"dhlr");
        b.bytes(b"url ");
        b.zeros(12);
        b.u8(0);
        b.end(m);
    }
    let d = b.start(b"dinf");
    let r = b.start_full(b"dref", 0, 0);
    b.u32(1);
    let u = b.start_full(b"url ", 0, 1);
    b.end(u);
    b.end(r);
    b.end(d);
    let stbl = b.start(b"stbl");
    let m = b.start_full(b"stsd", 0, 0);
    b.u32(1);
    t.cfg.entry.write(b, qt)?;
    b.end(m);
    let tb = &t.t;
    let m = b.start_full(b"stts", 0, 0);
    b.u32(tb.stts.len() as u32);
    for &(c, d) in &tb.stts {
        b.u32(c);
        b.u32(d);
    }
    b.end(m);
    if tb.ctts.iter().any(|&(_, o)| o != 0) {
        let neg = tb.ctts.iter().any(|&(_, o)| o < 0);
        let m = b.start_full(b"ctts", neg as u8, 0);
        b.u32(tb.ctts.len() as u32);
        for &(c, o) in &tb.ctts {
            b.u32(c);
            b.i32(o);
        }
        b.end(m);
    }
    if !tb.all_sync {
        let m = b.start_full(b"stss", 0, 0);
        b.u32(tb.sync.len() as u32);
        for &s in &tb.sync {
            b.u32(s);
        }
        b.end(m);
    }
    let m = b.start_full(b"stsc", 0, 0);
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for (i, &(_, n)) in tb.chunks.iter().enumerate() {
        if runs.last().is_none_or(|r| r.1 != n) {
            runs.push((i as u32 + 1, n));
        }
    }
    b.u32(runs.len() as u32);
    for (first, n) in runs {
        b.u32(first);
        b.u32(n);
        b.u32(1);
    }
    b.end(m);
    let m = b.start_full(b"stsz", 0, 0);
    match tb.const_size {
        Some(s) => {
            b.u32(s);
            b.u32(tb.sample_count.min(u32::MAX as u64) as u32);
        }
        None => {
            let first = tb.sizes.first().copied().unwrap_or(0);
            // A uniform size of 0 would mean "table follows" (ISO/IEC 14496-12 §8.7.3), so
            // all-empty samples must still be written as a table.
            if first != 0 && !tb.sizes.is_empty() && tb.sizes.iter().all(|&s| s == first) {
                b.u32(first);
                b.u32(tb.sizes.len() as u32);
            } else {
                b.u32(0);
                b.u32(tb.sizes.len() as u32);
                for &s in &tb.sizes {
                    b.u32(s);
                }
            }
        }
    }
    b.end(m);
    let co64 = force_co64 || tb.chunks.iter().any(|&(o, _)| o + shift > u32::MAX as u64);
    let m = b.start_full(if co64 { b"co64" } else { b"stco" }, 0, 0);
    b.u32(tb.chunks.len() as u32);
    for &(o, _) in &tb.chunks {
        if co64 {
            b.u64(o + shift);
        } else {
            b.u32((o + shift) as u32);
        }
    }
    b.end(m);
    b.end(stbl);
    b.end(minf);
    b.end(mdia);
    b.end(trak);
    Ok(())
}

fn write_udta(b: &mut BoxBuf, md: &[(String, String)]) {
    let items: Vec<([u8; 4], &str)> = md
        .iter()
        .filter_map(|(k, v)| {
            let mut chars = k.chars();
            if chars.next()? != '©' {
                return None;
            }
            let rest: Vec<u8> = chars.map(|c| c as u32 as u8).collect();
            (rest.len() == 3).then(|| ([0xA9, rest[0], rest[1], rest[2]], v.as_str()))
        })
        .collect();
    if items.is_empty() {
        return;
    }
    let u = b.start(b"udta");
    for (k, v) in items {
        let m = b.start(&k);
        b.u16(v.len() as u16);
        b.u16(0x55C4);
        b.bytes(v.as_bytes());
        b.end(m);
    }
    b.end(u);
}

fn validate_track(cfg: &TrackConfig) -> Result<Option<u32>> {
    if cfg.timescale == 0 {
        return Err(Error::Mux("timescale must be non-zero".into()));
    }
    if let CodecConfig::Pcm(p) = &cfg.entry.codec {
        if p.bits == 0 || p.channels == 0 {
            return Err(Error::Mux("PCM needs bits and channels".into()));
        }
        if (p.sample_rate - cfg.timescale as f64).abs() > 0.5 {
            return Err(Error::Mux("PCM track timescale must equal the sample rate".into()));
        }
        return Ok(Some(p.bytes_per_frame()));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobmff::codec::{AvcConfig, PcmConfig};
    use std::io::Cursor;

    fn avc_entry() -> SampleEntry {
        SampleEntry::avc(AvcConfig::new(vec![vec![0x67, 0x42, 0, 0x1E]], vec![vec![0x68, 0xCE]], 4), 64, 48)
    }

    #[test]
    fn language_pack() {
        assert_eq!(pack_language("und"), 0x55C4);
        assert_eq!(pack_language("eng"), 0x15C7);
        assert_eq!(pack_language("EN"), 0x55C4);
    }

    #[test]
    fn simple_roundtrip_and_faststart() {
        for fast in [false, true] {
            let mut w = Mp4Writer::new(Cursor::new(Vec::new()), WriterOptions::new(Brand::Mp4)).unwrap();
            let v = w.add_track(TrackConfig::new(avc_entry(), 12800)).unwrap();
            let pcm = PcmConfig { bits: 16, float: false, big_endian: false, signed: true, channels: 2, sample_rate: 48000.0 };
            let a = w.add_track(TrackConfig::new(SampleEntry::pcm(pcm), 48000)).unwrap();
            for i in 0..10u8 {
                let data = vec![i; 100 + i as usize];
                w.write_sample(
                    v,
                    WriteSample { data: &data, duration: 512, composition_offset: if i % 2 == 0 { 1024 } else { 0 }, is_sync: i % 5 == 0 },
                )
                .unwrap();
                w.write_sample(a, WriteSample { data: &[i; 400], duration: 0, composition_offset: 0, is_sync: true }).unwrap();
            }
            let out = if fast { w.finish_faststart() } else { w.finish() }.unwrap().into_inner();
            let f = crate::isobmff::open(out.as_slice()).unwrap();
            assert_eq!(f.tracks.len(), 2);
            let t = &f.tracks[0];
            assert_eq!(t.samples.len(), 10);
            for (i, s) in t.samples.iter().enumerate() {
                assert_eq!(s.size as usize, 100 + i);
                assert_eq!(s.dts, 512 * i as i64);
                assert_eq!(s.is_sync, i % 5 == 0);
                assert_eq!(f.read_sample(out.as_slice(), 0, i).unwrap(), vec![i as u8; 100 + i]);
            }
            let at = &f.tracks[1];
            assert!(at.pcm_chunked);
            assert_eq!(at.samples.len(), 10);
            assert_eq!(at.samples[3].size, 400);
            assert_eq!(at.samples[3].duration, 100);
            assert_eq!(f.read_sample(out.as_slice(), 1, 3).unwrap(), vec![3u8; 400]);
            if fast {
                assert!(f.moov_offset < t.samples[0].offset);
            }
        }
    }

    #[test]
    fn fragmented_roundtrip() {
        let mut w = FragmentedWriter::new(Vec::new(), WriterOptions::new(Brand::Mp4));
        let v = w.add_track(TrackConfig::new(avc_entry(), 1000)).unwrap();
        for i in 0..6u8 {
            w.write_sample(v, WriteSample { data: &[i; 10], duration: 40, composition_offset: -(i as i32 % 2), is_sync: i % 3 == 0 }).unwrap();
            if i % 3 == 2 {
                w.flush_fragment().unwrap();
            }
        }
        let out = w.finish().unwrap();
        let f = crate::isobmff::open(out.as_slice()).unwrap();
        assert!(f.fragmented);
        assert_eq!(f.fragment_count, 2);
        let t = &f.tracks[0];
        assert_eq!(t.samples.len(), 6);
        for (i, s) in t.samples.iter().enumerate() {
            assert_eq!(s.dts, 40 * i as i64);
            assert_eq!(s.pts, s.dts - (i as i64 % 2));
            assert_eq!(s.is_sync, i % 3 == 0);
            assert_eq!(f.read_sample(out.as_slice(), 0, i).unwrap(), vec![i as u8; 10]);
        }
        assert_eq!(t.sync_sample_before(5), 3);
    }
}
