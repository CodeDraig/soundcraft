//! Importing session tracks and media with remapped identifiers.

use super::super::*;
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_model::Route;
use soundcraft_model::Session;
use soundcraft_model::Source;
use soundcraft_model::SourceAudio;
use soundcraft_model::SourceId;
use soundcraft_model::TrackId;
use soundcraft_model::TrackKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Decode a source file of another session (paths relative to that session's folder).
fn load_source(dir: &Path, src: &Source, sr: u32) -> Option<soundcraft_audio_io::AudioBuffer> {
    let p = if Path::new(&src.path).is_absolute() { PathBuf::from(&src.path) } else { dir.join(&src.path) };
    let bytes = std::fs::read(&p).ok()?;
    let (_, mut buf) = soundcraft_audio_io::decode(&bytes, p.extension().and_then(|x| x.to_str())).ok()?;
    if buf.sample_rate != sr && buf.sample_rate > 0 {
        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, sr);
        buf.sample_rate = sr;
    }
    Some(buf)
}

pub(super) fn import_session_data(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "file.import_session_data";
    let path = str_param(p, "path").filter(|x| !x.trim().is_empty()).ok_or_else(|| bad(id, "`path` required"))?.to_string();
    let text_v = std::fs::read_to_string(&path).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    let other = Session::from_json(&text_v).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    let dir = Path::new(&path).parent().map(Path::to_path_buf).unwrap_or_default();
    let names: Option<Vec<String>> =
        p.get("tracks").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
    let picked: Vec<soundcraft_model::Track> = match &names {
        Some(ns) => {
            let mut v = Vec::new();
            for n in ns {
                v.push(other.track_by_name(n).cloned().ok_or_else(|| bad(id, format!("`{path}` has no track named `{n}`")))?);
            }
            v
        }
        None => other.tracks.iter().filter(|t| t.kind != TrackKind::Master).cloned().collect(),
    };
    if picked.is_empty() {
        return Err(bad(id, "nothing to import"));
    }
    let mut needed: Vec<SourceId> =
        picked.iter().flat_map(|t| t.playlists.iter().flat_map(|pl| pl.clips.iter().filter_map(|c| c.source()))).collect();
    needed.sort_by_key(|x| x.0);
    needed.dedup();
    let sr = e.session().sample_rate.hz();
    let decoded: Vec<(Source, Option<soundcraft_audio_io::AudioBuffer>)> =
        needed.iter().filter_map(|sid| other.source(*sid).cloned()).map(|src| (src.clone(), load_source(&dir, &src, sr))).collect();
    let s = e.session_mut();
    let mut map: std::collections::BTreeMap<u64, SourceId> = Default::default();
    let mut missing = Vec::new();
    for (src, buf) in decoded {
        let nid = SourceId(s.alloc());
        let abs = if Path::new(&src.path).is_absolute() { PathBuf::from(&src.path) } else { dir.join(&src.path) };
        let mut ns = src.clone();
        ns.id = nid;
        ns.path = abs.to_string_lossy().into_owned();
        match buf {
            Some(b) => {
                ns.frames = b.frames() as u64;
                ns.sample_rate = sr;
                ns.unsaved = true;
                s.pool.insert(nid, Arc::new(SourceAudio::new(b)));
            }
            None => {
                ns.unsaved = false;
                missing.push(ns.path.clone());
            }
        }
        s.sources.push(ns);
        map.insert(src.id.0, nid);
    }
    let mut out = Vec::new();
    for mut tr in picked {
        tr.id = TrackId(s.alloc());
        tr.name = s.unique_track_name(&tr.name);
        tr.folder = None;
        tr.mixer.vca = None;
        for pl in &mut tr.playlists {
            for c in &mut pl.clips {
                c.id = s.new_clip_id();
                if let ClipContent::Audio { source, .. } = &mut c.content
                    && let Some(n) = map.get(&source.0)
                {
                    *source = *n;
                }
            }
        }
        // Busses are matched by name (created when missing).
        let remap = |s: &mut Session, r: &Route| -> Route {
            match r {
                Route::Bus(b) => other.bus(*b).map_or(Route::None, |ob| Route::Bus(s.add_bus(&ob.name, ob.format))),
                x => x.clone(),
            }
        };
        tr.mixer.input = remap(s, &tr.mixer.input);
        tr.mixer.output = remap(s, &tr.mixer.output);
        for slot in tr.mixer.sends.iter_mut().flatten() {
            slot.target = remap(s, &slot.target);
        }
        let pos = s.tracks.iter().position(|t| t.kind == TrackKind::Master).unwrap_or(s.tracks.len());
        out.push(tr.id);
        s.tracks.insert(pos, tr);
    }
    s.edit.selected_tracks = out.clone();
    Ok(json!({"tracks": out, "sources": map.len(), "missing": missing}))
}
