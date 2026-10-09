//! Decoded audio source registration and placement.

use crate::Engine;
use crate::EngineError;
use crate::Result;
use serde_json::Value;
use serde_json::json;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_audio_io::FileFormat;
use soundcraft_model::ChannelFormat;
use soundcraft_model::Clip;
use soundcraft_model::Session;
use soundcraft_model::Source;
use soundcraft_model::SourceAudio;
use soundcraft_model::SourceId;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;
use soundcraft_time::Samples;
use std::path::Path;
use std::sync::Arc;

/// Largest import we accept (samples per channel × channels), ~4 GiB of f32.
const MAX_IMPORT_SAMPLES: usize = 1 << 30;

/// Register decoded audio as a new source. Resamples to the session rate if needed.
pub fn add_source(s: &mut Session, name: &str, mut buf: AudioBuffer, path: Option<&str>, format: FileFormat) -> SourceId {
    let sr = s.sample_rate.hz();
    if buf.sample_rate != sr && buf.sample_rate > 0 {
        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, sr);
        buf.sample_rate = sr;
    }
    let id = SourceId(s.alloc());
    let stem = Path::new(name).file_stem().and_then(|x| x.to_str()).unwrap_or(name).to_string();
    s.sources.push(Source {
        id,
        name: stem.clone(),
        path: path.map_or_else(|| format!("Audio Files/{stem}.wav"), str::to_string),
        channels: u16::try_from(buf.num_channels()).unwrap_or(u16::MAX),
        frames: buf.frames() as u64,
        sample_rate: sr,
        format,
        time_reference: 0,
        unsaved: true,
    });
    s.pool.insert(id, Arc::new(SourceAudio::new(buf)));
    id
}

/// Decode and place an audio file. With no target track, a new track of matching width is made.
pub fn import_audio_bytes(e: &mut Engine, name: &str, bytes: &[u8], path: Option<&str>, target: Option<TrackId>, at: Samples) -> Result<Value> {
    let ext = Path::new(name).extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase);
    let (info, buf) = soundcraft_audio_io::decode(bytes, ext.as_deref()).map_err(|err| EngineError::Io(format!("{name}: {err}")))?;
    if buf.frames().saturating_mul(buf.num_channels().max(1)) > MAX_IMPORT_SAMPLES {
        return Err(EngineError::Io(format!("{name}: file is too large to import")));
    }
    if buf.frames() == 0 {
        return Err(EngineError::Io(format!("{name}: no audio")));
    }
    let channels = buf.num_channels();
    let s = e.session_mut();
    let src = add_source(s, name, buf, path, info.format);
    if let (Some(bwf), Some(sx)) = (info.bwf.as_ref(), s.sources.last_mut()) {
        sx.time_reference = bwf.time_reference;
    }
    let frames = s.source(src).map_or(0, |x| i64::try_from(x.frames).unwrap_or(0));
    let track = match target {
        Some(t) => t,
        None => {
            let stem = Path::new(name).file_stem().and_then(|x| x.to_str()).unwrap_or("Audio").to_string();
            s.add_track(TrackKind::Audio, ChannelFormat::for_channels(channels.min(2)), Some(&stem))
        }
    };
    let cid = s.new_clip_id();
    let stem = s.source(src).map(|x| x.name.clone()).unwrap_or_default();
    crate::edit::place_clip(s, track, Clip::audio(cid, stem, src, 0, at, frames));
    Ok(json!({"track": track, "clip": cid, "source": src, "frames": frames, "channels": channels}))
}
