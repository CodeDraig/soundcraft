//! Bounded clip stretching into a new source.

use super::super::*;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_audio_io::FileFormat;
use soundcraft_model::Clip;
use soundcraft_model::ClipContent;
use soundcraft_model::Session;
use soundcraft_model::SourceId;
use soundcraft_model::TrackId;
use soundcraft_time::Samples;

/// Longest audio we render in one stretch (one hour at the session rate).
fn max_render(s: &Session) -> Samples {
    s.sample_rate.samples(3600.0)
}

/// Render the audio a clip plays, time-stretched (pitch preserved) to `out_len` samples, into a new
/// source. Returns `Ok(None)` for MIDI clips or missing audio.
pub fn render_stretched(s: &mut Session, cmd: &str, c: &Clip, out_len: Samples) -> Result<Option<SourceId>> {
    let ClipContent::Audio { source, offset } = c.content else { return Ok(None) };
    let Some(audio) = s.pool.get(source).cloned() else { return Ok(None) };
    if out_len <= 0 || out_len > max_render(s) {
        return Err(bad(cmd, "the target length must be between 1 sample and one hour"));
    }
    let stretch = if c.stretch.is_finite() && c.stretch > 0.0 { c.stretch } else { 1.0 };
    let src_len = ((c.length as f64) / stretch).round().max(1.0);
    let ratio = out_len as f64 / src_len;
    if !(0.1..=10.0).contains(&ratio) {
        return Err(bad(cmd, format!("stretch ratio {ratio:.3} is outside 0.1..10")));
    }
    let a = usize::try_from(offset.max(0)).unwrap_or(0);
    let n = src_len as usize;
    let ch: Vec<Vec<f32>> = audio
        .buffer
        .channels
        .iter()
        .map(|x| {
            let mut v: Vec<f32> = x.iter().skip(a).take(n).copied().collect();
            v.resize(n, 0.0);
            v
        })
        .collect();
    let sr = s.sample_rate;
    let mut out = soundcraft_dsp::offline::time_stretch(&ch, ratio, sr.as_f64() as f32);
    let want = usize::try_from(out_len).unwrap_or(0);
    for o in &mut out {
        o.resize(want, 0.0);
    }
    let buf = AudioBuffer { sample_rate: sr.hz(), channels: out };
    Ok(Some(crate::io::add_source(s, &format!("{}-TCE", c.name), buf, None, FileFormat::Wav)))
}

/// Replace clip `old` on track `t` with `new` (overwriting whatever `new` covers).
pub fn replace_clip(s: &mut Session, t: TrackId, old: soundcraft_model::ClipId, new: Clip) {
    if let Some(pl) = s.track_mut(t).and_then(|tr| tr.playlist_mut()) {
        pl.clips.retain(|c| c.id != old);
    }
    crate::edit::place_clip(s, t, new);
}
