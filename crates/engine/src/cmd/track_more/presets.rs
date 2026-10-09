//! Track preset persistence.

use super::super::*;
use serde_json::json;
use soundcraft_model::Insert;
use soundcraft_model::TrackKind;

const PRESET_VERSION: u32 = 1;

const PRESET_FORMAT: &str = "soundcraft-track-preset";

/// Every field is emitted by the current track-preset writer. Nullable fields require keys.
#[derive(serde::Serialize, serde::Deserialize)]
struct TrackPreset {
    format: String,
    version: u32,
    name: String,
    kind: String,
    width: String,
    color: [u8; 3],
    mixer: soundcraft_model::Mixer,
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    instrument: Option<Insert>,
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    elastic: Option<String>,
    comments: String,
}

pub(super) fn save_preset(e: &mut Engine, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").filter(|x| !x.trim().is_empty()).ok_or_else(|| bad("track.save_preset", "`path` required"))?.to_string();
    let t = tracks_required(e, "track.save_preset", p)?;
    let first = *t.first().ok_or_else(|| bad("track.save_preset", "no track"))?;
    let tr = e.session().track(first).ok_or_else(|| bad("track.save_preset", "no track"))?;
    let preset = TrackPreset {
        format: PRESET_FORMAT.into(),
        version: PRESET_VERSION,
        name: tr.name.clone(),
        kind: tr.kind.id().into(),
        width: tr.format.label().into(),
        color: tr.color,
        mixer: tr.mixer.clone(),
        instrument: tr.instrument.clone(),
        elastic: tr.elastic.clone(),
        comments: tr.comments.clone(),
    };
    let preset = serde_json::to_value(preset).map_err(|err| EngineError::Io(err.to_string()))?;
    let text = serde_json::to_string_pretty(&preset).map_err(|err| EngineError::Io(err.to_string()))?;
    std::fs::write(&path, text).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    Ok(json!({"path": path, "track": first}))
}

pub(super) fn load_preset(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "track.load_preset";
    let path = str_param(p, "path").ok_or_else(|| bad(id, "`path` required"))?.to_string();
    let tracks = tracks_required(e, id, p)?;
    let text = std::fs::read_to_string(&path).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    let v: Value = serde_json::from_str(&text).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    if v.get("format").and_then(Value::as_str) != Some(PRESET_FORMAT) {
        return Err(EngineError::Io(format!("{path}: not a SoundCraft track preset")));
    }
    let preset: TrackPreset = serde_json::from_value(v).map_err(|err| EngineError::Io(format!("{path}: {err}")))?;
    if preset.version != PRESET_VERSION {
        return Err(EngineError::Io(format!("{path}: unsupported track preset version {}", preset.version)));
    }
    let mut mixer = preset.mixer;
    mixer.sanitize();
    let instrument = preset.instrument;
    let s = e.session_mut();
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            // Routing is session-specific: keep the track's own input/output, VCA and arm state.
            let keep = (tr.mixer.input.clone(), tr.mixer.output.clone(), tr.mixer.vca, tr.mixer.record_arm, tr.mixer.pan.clone());
            tr.mixer = mixer.clone();
            tr.mixer.input = keep.0;
            tr.mixer.output = keep.1;
            tr.mixer.vca = keep.2;
            tr.mixer.record_arm = keep.3;
            if tr.mixer.pan.len() != keep.4.len() {
                tr.mixer.pan = keep.4;
            }
            if tr.kind == TrackKind::Instrument && instrument.is_some() {
                tr.instrument = instrument.clone();
            }
        }
    }
    Ok(json!({"tracks": tracks}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_preset_rejects_missing_fields_without_mutating_the_session() {
        let dir = std::env::temp_dir().join(format!("soundcraft-current-preset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("current.json");
        let p = json!({"path": path.to_string_lossy(), "track": "Bass"});
        let mut e = crate::demo::demo_engine();
        save_preset(&mut e, &p).unwrap();
        let current: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let before = e.session().clone();
        for field in current.as_object().unwrap().keys() {
            let mut incomplete = current.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            std::fs::write(&path, incomplete.to_string()).unwrap();
            assert!(load_preset(&mut e, &p).is_err(), "accepted missing {field}");
            assert_eq!(e.session(), &before);
        }
        for version in [0, 2] {
            let mut unsupported = current.clone();
            unsupported["version"] = json!(version);
            std::fs::write(&path, unsupported.to_string()).unwrap();
            assert!(load_preset(&mut e, &p).is_err());
            assert_eq!(e.session(), &before);
        }
        std::fs::write(&path, current.to_string()).unwrap();
        load_preset(&mut e, &p).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
