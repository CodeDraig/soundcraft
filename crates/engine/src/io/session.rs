//! Session file and media persistence.

use super::{relink_videos, sanitize_name};
use crate::Engine;
use crate::EngineError;
use crate::Result;
use soundcraft_audio_io::EncodeOptions;
use soundcraft_audio_io::FileFormat;
use soundcraft_model::Session;
use soundcraft_model::SourceAudio;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Save the session to `path` (a `.scraft` file). Audio that only exists in memory is written
/// as 32-bit float WAV into `Audio Files/` next to it. Returns how many audio files were written.
pub fn save_session(e: &mut Engine, path: &str) -> Result<usize> {
    let (fresh, written, p) = write_session(e.session(), path, false)?;
    // Keep the in-memory document in sync (paths, unsaved flags) without an undo step.
    *e.session_mut() = fresh;
    e.path = Some(p.to_string_lossy().into_owned());
    e.mark_clean();
    Ok(written)
}

/// Write a copy of the session (and any audio it needs) without touching the open document —
/// used for autosave / crash recovery. Audio files in the target folder are overwritten in place.
pub fn save_copy(e: &Engine, path: &str) -> Result<usize> {
    write_session(e.session(), path, true).map(|(_, n, _)| n)
}

/// Write `session` to `path`; returns the session with updated media paths, the number of audio
/// files written and the final path.
fn write_session(session: &Session, path: &str, overwrite: bool) -> Result<(Session, usize, PathBuf)> {
    let p = PathBuf::from(path);
    let p = if p.extension().is_none() { p.with_extension(soundcraft_model::SESSION_EXTENSION) } else { p };
    let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
    let audio_dir = dir.join("Audio Files");
    let mut s = session.clone();
    let mut written = 0;
    for src in &mut s.sources {
        let rel_target = format!("Audio Files/{}.wav", sanitize_name(&src.name));
        let exists_rel = !Path::new(&src.path).is_absolute() && dir.join(&src.path).exists();
        if exists_rel && !src.unsaved {
            continue;
        }
        let Some(audio) = s.pool.get(src.id) else { continue };
        std::fs::create_dir_all(&audio_dir).map_err(|err| EngineError::Io(format!("{}: {err}", audio_dir.display())))?;
        let mut target = rel_target.clone();
        if overwrite {
            // One file per source id so autosaves never collide or pile up.
            target = format!("Audio Files/{}_{}.wav", sanitize_name(&src.name), src.id.0);
            if dir.join(&target).exists() {
                src.path = target;
                src.unsaved = false;
                continue;
            }
        } else {
            let mut k = 1;
            while dir.join(&target).exists() && !(exists_rel && target == src.path) {
                target = format!("Audio Files/{}_{k:02}.wav", sanitize_name(&src.name));
                k += 1;
                if k > 999 {
                    break;
                }
            }
        }
        let opts = EncodeOptions { format: FileFormat::Wav, bit_depth: soundcraft_audio_io::BitDepth::Float32, dither: false, bwf: None };
        let bytes = soundcraft_audio_io::encode(&audio.buffer, &opts).map_err(|err| EngineError::Io(err.to_string()))?;
        std::fs::write(dir.join(&target), bytes).map_err(|err| EngineError::Io(format!("{target}: {err}")))?;
        src.path = target;
        src.unsaved = false;
        src.format = FileFormat::Wav;
        written += 1;
    }
    if !overwrite && let Some(stem) = p.file_stem().and_then(|x| x.to_str()) {
        s.name = stem.to_string();
    }
    let text = s.to_json().map_err(|err| EngineError::Io(err.to_string()))?;
    // Atomic-ish write: temp file then rename.
    let tmp = p.with_extension("scraft.tmp");
    std::fs::write(&tmp, text).map_err(|err| EngineError::Io(format!("{}: {err}", tmp.display())))?;
    std::fs::rename(&tmp, &p).map_err(|err| EngineError::Io(format!("{}: {err}", p.display())))?;
    Ok((s, written, p))
}

/// Open a `.scraft` session and decode its audio files (missing files are reported, not fatal).
pub fn open_session(e: &mut Engine, path: &str) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    let mut s = Session::from_json(&text).map_err(|err| EngineError::Io(err.to_string()))?;
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let mut missing = Vec::new();
    for src in s.sources.clone() {
        let p = if Path::new(&src.path).is_absolute() { PathBuf::from(&src.path) } else { dir.join(&src.path) };
        match std::fs::read(&p) {
            Ok(bytes) => match soundcraft_audio_io::decode(&bytes, p.extension().and_then(|x| x.to_str())) {
                Ok((_, mut buf)) => {
                    if buf.sample_rate != s.sample_rate.hz() && buf.sample_rate > 0 {
                        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, s.sample_rate.hz());
                        buf.sample_rate = s.sample_rate.hz();
                    }
                    s.pool.insert(src.id, Arc::new(SourceAudio::new(buf)));
                }
                Err(err) => missing.push(format!("{}: {err}", p.display())),
            },
            Err(_) => missing.push(p.display().to_string()),
        }
    }
    missing.extend(relink_videos(&mut s, &dir));
    e.replace_session(s);
    e.path = Some(path.to_string());
    for m in &missing {
        e.message(format!("missing media: {m}"));
    }
    Ok(missing)
}

#[cfg(test)]
mod save_tests {
    use super::*;

    #[test]
    fn autosave_copy_leaves_document_untouched_and_reuses_audio() {
        let mut e = crate::demo::demo_engine();
        e.execute("mix.volume", &serde_json::json!({"track": "Kick", "db": -7.0})).unwrap();
        let dir = std::env::temp_dir().join(format!("soundcraft-autosave-{}", std::process::id()));
        let path = dir.join("Midnight Groove.scraft");
        let p = path.to_string_lossy().into_owned();
        let n1 = save_copy(&e, &p).unwrap();
        let n2 = save_copy(&e, &p).unwrap();
        assert!(n1 >= 5);
        assert_eq!(n2, 0, "second autosave must not rewrite audio");
        assert!(e.is_dirty() && e.path.is_none());
        let mut r = Engine::default();
        open_session(&mut r, &p).unwrap();
        assert_eq!(r.session().track_by_name("Kick").unwrap().mixer.volume_db, -7.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
