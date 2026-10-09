//! Mix and stem rendering with loudness reporting.

use super::sanitize_name;
use crate::Engine;
use crate::EngineError;
use crate::Result;
use soundcraft_audio_io::AudioBuffer;
use soundcraft_audio_io::EncodeOptions;
use soundcraft_audio_io::FileFormat;
use soundcraft_model::ChannelFormat;
use soundcraft_model::Session;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;
use soundcraft_time::Range;
use std::path::Path;

/// Render the main mix in its own format (every main channel, SMPTE/WAV order, with the WAV
/// speaker mask of the main format), or an ITU stereo fold-down when `fold_stereo`.
pub fn bounce_bytes_with(e: &Engine, r: Range, opts: &EncodeOptions, normalize: bool, fold_stereo: bool) -> Result<(Vec<u8>, (f32, f32, f32))> {
    let s = e.session();
    if r.len() > s.sample_rate.samples(4.0 * 3600.0) {
        return Err(EngineError::BadParams("file.bounce_mix".into(), "bounces are limited to 4 hours".into()));
    }
    let (mut ch, fmt) = render_main(s, r, fold_stereo);
    if normalize {
        soundcraft_dsp::offline::normalize(&mut ch, -0.1, false);
    }
    let peak = soundcraft_dsp::offline::peak_db(&ch);
    let mut lm = soundcraft_dsp::meter::LoudnessMeter::new(s.sample_rate.as_f64() as f32, ch.len());
    let frames = ch.first().map_or(0, Vec::len);
    lm.process(&ch, frames);
    let lufs = lm.integrated_lufs();
    let true_peak = lm.true_peak_db();
    let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: ch };
    let bytes = soundcraft_audio_io::encode_with_channel_mask(&buf, opts, fmt.channel_mask()).map_err(|err| EngineError::Io(err.to_string()))?;
    Ok((bytes, (peak, lufs, true_peak)))
}

/// The main mix over `r` and its format (folded to stereo on request).
fn render_main(s: &Session, r: Range, fold_stereo: bool) -> (Vec<Vec<f32>>, ChannelFormat) {
    let ch = soundcraft_mix::render_range(s, r, 1024);
    let fmt = if ch.len() <= 2 { ChannelFormat::Stereo } else { s.main_format() };
    if fold_stereo && ch.len() > 2 { (soundcraft_mix::fold_down_stereo(fmt, &ch), ChannelFormat::Stereo) } else { (ch, fmt) }
}

/// Bounce one file per audio/instrument track, including its sends and auxes.
/// Use the main format or fold to stereo when `fold_stereo`. Returns written paths.
pub fn bounce_stems_with(e: &Engine, dir: &str, r: Range, opts: &EncodeOptions, fold_stereo: bool) -> Result<Vec<String>> {
    let s = e.session();
    std::fs::create_dir_all(dir).map_err(|err| EngineError::Io(format!("{dir}: {err}")))?;
    let ext = match opts.format {
        FileFormat::Aiff => "aif",
        FileFormat::Flac => "flac",
        _ => "wav",
    };
    let mut out = Vec::new();
    let stems: Vec<(TrackId, String)> = s
        .tracks
        .iter()
        .filter(|t| matches!(t.kind, TrackKind::Audio | TrackKind::Instrument | TrackKind::Midi) && !t.inactive && !t.mixer.mute)
        .map(|t| (t.id, t.name.clone()))
        .collect();
    for (id, name) in stems {
        let mut solo = s.clone();
        for t in &mut solo.tracks {
            t.mixer.solo = t.id == id;
        }
        let (ch, fmt) = render_main(&solo, r, fold_stereo);
        let buf = AudioBuffer { sample_rate: s.sample_rate.hz(), channels: ch };
        let bytes = soundcraft_audio_io::encode_with_channel_mask(&buf, opts, fmt.channel_mask()).map_err(|err| EngineError::Io(err.to_string()))?;
        let path = Path::new(dir).join(format!("{} - {}.{ext}", sanitize_name(&s.name), sanitize_name(&name)));
        std::fs::write(&path, bytes).map_err(|err| EngineError::Io(format!("{}: {err}", path.display())))?;
        out.push(path.to_string_lossy().into_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod stem_tests {
    #[test]
    fn stems_one_file_per_track() {
        let mut e = crate::demo::demo_engine();
        let dir = std::env::temp_dir().join(format!("soundcraft-stems-{}", std::process::id()));
        let r = e
            .execute("file.bounce_stems", &serde_json::json!({"dir": dir.to_string_lossy(), "start": {"seconds": 10.0}, "end": {"seconds": 11.0}}))
            .unwrap();
        let n = r["files"].as_array().map_or(0, Vec::len);
        assert_eq!(n, 7, "{r}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
