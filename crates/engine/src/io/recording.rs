//! Placement of recorded input on armed tracks.

use super::add_source;
use crate::Engine;
use crate::EngineError;
use crate::Result;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_audio_io::FileFormat;
use soundcraft_model::Clip;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;
use soundcraft_time::Samples;

/// Turn a recorded take into clips on the record-armed tracks (one undo step: "Record").
/// `channels` is the planar input capture at `rate`; each track takes the input channels its
/// input route names ("In 3" → channel 2), defaulting to the first channels.
pub fn add_recording(e: &mut Engine, start: Samples, channels: Vec<Vec<f32>>, rate: u32) -> Result<Vec<soundcraft_model::ClipId>> {
    let frames = channels.first().map_or(0, Vec::len);
    if frames == 0 {
        return Ok(Vec::new());
    }
    let armed: Vec<(TrackId, usize, usize, String)> = e
        .session()
        .tracks
        .iter()
        .filter(|t| t.mixer.record_arm && t.kind == TrackKind::Audio && !t.inactive)
        .map(|t| {
            let first = match &t.mixer.input {
                soundcraft_model::Route::Hardware(h) => {
                    h.trim_start_matches("In ").split(['-', ' ']).next().and_then(|n| n.parse::<usize>().ok()).map_or(0, |n| n.saturating_sub(1))
                }
                _ => 0,
            };
            (t.id, first, t.channels(), t.name.clone())
        })
        .collect();
    if armed.is_empty() {
        return Err(EngineError::BadParams("record".into(), "no tracks are record-enabled".into()));
    }
    let before = e.session_arc();
    let mut out = Vec::new();
    let take_no = e.session().sources.len() + 1;
    for (tid, first, n, name) in armed {
        let chans: Vec<Vec<f32>> = (0..n).map(|k| channels.get((first + k) % channels.len().max(1)).cloned().unwrap_or_default()).collect();
        let buf = AudioBuffer { sample_rate: rate, channels: chans };
        let clip_name = format!("{name}_{take_no:02}");
        let s = e.session_mut();
        let src = add_source(s, &clip_name, buf, None, FileFormat::Wav);
        let len = s.source(src).map_or(0, |x| i64::try_from(x.frames).unwrap_or(0));
        let cid = s.new_clip_id();
        crate::edit::place_clip(s, tid, Clip::audio(cid, clip_name, src, 0, start, len));
        out.push(cid);
    }
    e.push_undo("Record", before);
    Ok(out)
}
