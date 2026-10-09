//! Seekable movie writing, sample tables and faststart relocation.

use super::{
    Brand, Tables, TrackConfig, WTrack, WriteSample, WriterOptions, handler_of, movie_duration, rescale, validate_track, write_ftyp, write_mvhd,
    write_trak, write_udta,
};
use crate::isobmff::bytes::BoxBuf;
use crate::isobmff::codec::CodecConfig;
use crate::isobmff::codec::SampleEntry;
use crate::isobmff::codec::TimecodeConfig;
use crate::isobmff::error::Error;
use crate::isobmff::error::Result;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;

/// Progressive MP4/MOV writer: `ftyp`, then one `mdat` streamed to `W`, then `moov` at [`finish`](Self::finish)
/// (or moved in front of `mdat` by [`finish_faststart`](Self::finish_faststart)).
pub struct Mp4Writer<W: Write + Seek> {
    w: W,
    opts: WriterOptions,
    tracks: Vec<WTrack>,
    /// Offset of the placeholder box (8 bytes) preceding the mdat header.
    pad_pos: u64,
    mdat_pos: u64,
    pos: u64,
    last_track: Option<usize>,
}

impl<W: Write + Seek> Mp4Writer<W> {
    /// Start a file: writes `ftyp`, a placeholder box and the `mdat` header.
    pub fn new(mut w: W, opts: WriterOptions) -> Result<Self> {
        let start = w.stream_position()?;
        let mut b = BoxBuf::new();
        write_ftyp(&mut b, opts.brand);
        let pad_pos = start + b.len() as u64;
        b.leaf(if opts.brand == Brand::Mov { b"wide" } else { b"free" }, &[]);
        let mdat_pos = start + b.len() as u64;
        b.leaf(b"mdat", &[]);
        w.write_all(&b.buf)?;
        let pos = start + b.len() as u64;
        Ok(Mp4Writer { w, opts, tracks: Vec::new(), pad_pos, mdat_pos, pos, last_track: None })
    }

    /// Add a track; returns its index (track ids are index + 1).
    pub fn add_track(&mut self, cfg: TrackConfig) -> Result<usize> {
        let pcm = validate_track(&cfg)?;
        let handler = handler_of(&cfg.entry);
        let mut t = Tables::new();
        t.const_size = pcm;
        self.tracks.push(WTrack { id: self.tracks.len() as u32 + 1, handler, pcm_frame_bytes: pcm, t, cfg, tmcd_ref: None, tmcd_source: None });
        Ok(self.tracks.len() - 1)
    }

    /// Add a QuickTime timecode track referenced by `video_track`. Its single sample (the start
    /// frame number) is written at finish, spanning the video track's duration.
    pub fn add_timecode_track(&mut self, video_track: usize, tc: TimecodeConfig, start_frame: u32) -> Result<usize> {
        if video_track >= self.tracks.len() {
            return Err(Error::NoSuchTrack(video_track));
        }
        if tc.timescale == 0 || tc.frame_duration == 0 || tc.frames_per_second == 0 {
            return Err(Error::Mux("timecode needs timescale, frame duration and fps".into()));
        }
        let entry = SampleEntry {
            format: crate::isobmff::bytes::FourCc(*b"tmcd"),
            data_reference_index: 1,
            codec: CodecConfig::Timecode(tc.clone()),
            video: None,
            audio: None,
            bitrate: None,
        };
        let idx = self.add_track(TrackConfig::new(entry, tc.timescale))?;
        self.tracks[idx].tmcd_source = Some((video_track, start_frame));
        let id = self.tracks[idx].id;
        self.tracks[video_track].tmcd_ref = Some(id);
        Ok(idx)
    }

    /// Append one sample to track `track` (decode order). For PCM tracks `data` may hold any whole
    /// number of audio frames.
    pub fn write_sample(&mut self, track: usize, s: WriteSample) -> Result<()> {
        let tr = self.tracks.get(track).ok_or(Error::NoSuchTrack(track))?;
        if tr.tmcd_source.is_some() {
            return Err(Error::Mux("timecode track samples are written automatically".into()));
        }
        let (count, dur) = match tr.pcm_frame_bytes {
            Some(fb) => {
                if !s.data.len().is_multiple_of(fb as usize) {
                    return Err(Error::Mux("PCM data is not a whole number of frames".into()));
                }
                ((s.data.len() / fb as usize) as u32, 1u32)
            }
            None => {
                if s.data.len() > u32::MAX as usize {
                    return Err(Error::Mux("sample larger than 4 GiB".into()));
                }
                (1, s.duration)
            }
        };
        if count == 0 {
            return Ok(());
        }
        self.w.write_all(s.data)?;
        let offset = self.pos;
        self.pos += s.data.len() as u64;
        let contiguous = self.last_track == Some(track);
        self.last_track = Some(track);
        let tr = &mut self.tracks[track];
        let t = &mut tr.t;
        Tables::push_run(&mut t.stts, count, dur);
        Tables::push_run(&mut t.ctts, count, if tr.pcm_frame_bytes.is_some() { 0 } else { s.composition_offset });
        if tr.pcm_frame_bytes.is_none() {
            t.sizes.push(s.data.len() as u32);
            if s.is_sync {
                t.sync.push(t.sample_count as u32 + 1);
            } else {
                t.all_sync = false;
            }
        }
        t.sample_count += count as u64;
        let ct = if tr.pcm_frame_bytes.is_some() { 0 } else { s.composition_offset as i64 };
        t.comp_end = t.comp_end.max((t.media_duration as i64 + ct + count as i64 * dur as i64).max(0) as u64);
        t.media_duration += count as u64 * dur as u64;
        match t.chunks.last_mut() {
            Some(c) if contiguous => c.1 += count,
            _ => t.chunks.push((offset, count)),
        }
        Ok(())
    }

    fn finish_mdat(&mut self) -> Result<()> {
        // Timecode samples.
        for i in 0..self.tracks.len() {
            let Some((vi, start)) = self.tracks[i].tmcd_source else { continue };
            let v = &self.tracks[vi];
            let vd = v.t.media_duration;
            let vts = v.cfg.timescale;
            let tts = self.tracks[i].cfg.timescale;
            let tc_dur = match &self.tracks[i].cfg.entry.codec {
                CodecConfig::Timecode(tc) => tc.frame_duration,
                _ => 1,
            };
            let d = rescale(vd, vts, tts).max(tc_dur as u64).min(u32::MAX as u64) as u32;
            self.w.write_all(&start.to_be_bytes())?;
            let off = self.pos;
            self.pos += 4;
            self.last_track = None;
            let t = &mut self.tracks[i].t;
            t.stts.push((1, d));
            t.sizes.push(4);
            t.sync.push(1);
            t.sample_count = 1;
            t.media_duration = d as u64;
            t.chunks.push((off, 1));
        }
        let size = self.pos - self.mdat_pos;
        if size <= u32::MAX as u64 {
            self.w.seek(SeekFrom::Start(self.mdat_pos))?;
            self.w.write_all(&(size as u32).to_be_bytes())?;
        } else {
            // Merge the placeholder into a 16-byte large-size mdat header.
            let mut h = [0u8; 16];
            h[..4].copy_from_slice(&1u32.to_be_bytes());
            h[4..8].copy_from_slice(b"mdat");
            h[8..].copy_from_slice(&(self.pos - self.pad_pos).to_be_bytes());
            self.w.seek(SeekFrom::Start(self.pad_pos))?;
            self.w.write_all(&h)?;
        }
        self.w.seek(SeekFrom::Start(self.pos))?;
        Ok(())
    }

    fn build_moov(&self, shift: u64, force_co64: bool) -> Result<Vec<u8>> {
        let qt = self.opts.brand == Brand::Mov;
        let ts = self.opts.movie_timescale.max(1);
        let mut b = BoxBuf::new();
        let m = b.start(b"moov");
        write_mvhd(&mut b, ts, movie_duration(&self.tracks, ts), self.tracks.len() as u32 + 1);
        for t in &self.tracks {
            write_trak(&mut b, t, ts, qt, shift, force_co64)?;
        }
        write_udta(&mut b, &self.opts.metadata);
        b.end(m);
        Ok(b.buf)
    }

    /// Finish the file with `moov` after `mdat`. Returns the underlying writer.
    pub fn finish(mut self) -> Result<W> {
        self.finish_mdat()?;
        let moov = self.build_moov(0, false)?;
        self.w.write_all(&moov)?;
        self.w.flush()?;
        Ok(self.w)
    }
}

impl<W: Read + Write + Seek> Mp4Writer<W> {
    /// Finish with `moov` in front of `mdat` ("fast start"): media data is shifted forward in place.
    pub fn finish_faststart(mut self) -> Result<W> {
        self.finish_mdat()?;
        let mut moov = self.build_moov(0, false)?;
        let mut force = false;
        loop {
            let shift = moov.len() as u64;
            let next = self.build_moov(shift, force)?;
            if next.len() == moov.len() {
                moov = next;
                break;
            }
            // Offsets crossed the 32-bit boundary: switch to co64 (size only grows, so this converges).
            force = true;
            moov = next;
        }
        let shift = moov.len() as u64;
        let start = self.pad_pos;
        let end = self.pos;
        let mut buf = vec![0u8; 1 << 20];
        let mut cur = end;
        while cur > start {
            let n = (cur - start).min(buf.len() as u64) as usize;
            cur -= n as u64;
            self.w.seek(SeekFrom::Start(cur))?;
            self.w.read_exact(&mut buf[..n])?;
            self.w.seek(SeekFrom::Start(cur + shift))?;
            self.w.write_all(&buf[..n])?;
        }
        self.w.seek(SeekFrom::Start(start))?;
        self.w.write_all(&moov)?;
        self.w.seek(SeekFrom::Start(end + shift))?;
        self.w.flush()?;
        Ok(self.w)
    }
}
