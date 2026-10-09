//! More Track menu commands: Convert Aux to Routing Folder, Extract MIDI, track presets, insert /
//! send bypass subsets, MIDI real-time properties, trim/VCA coalescing and target playlists.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_dsp::Category;

mod presets;
use presets::{load_preset, save_preset};
mod conversion;
use conversion::{convert_aux, extract_midi};
mod bypass;
use bypass::{bypass_where, mute_sends_where};
mod midi_properties;
use midi_properties::write_rtp;
mod automation;
use automation::{coalesce_trim, coalesce_vca, trim_write};
mod playlists;
use playlists::{designate_target, show_target, toggle_recent};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "track.object",
            "Object / Bed",
            [],
            None,
            "{tracks?, object?: bool} — routes the tracks as immersive objects (true) or to the bed; toggles when `object` is omitted",
            has_selection,
            |e, p| {
                let tracks = tracks_required(e, "track.object", p)?;
                let want = p.get("object").and_then(Value::as_bool);
                let s = e.session_mut();
                let mut out = Vec::new();
                for t in tracks {
                    let key = format!("object.{}", t.0);
                    let on = want.unwrap_or(!s.edit.flag(&key));
                    s.edit.set_flag(&key, on);
                    out.push(json!({"track": t.0, "object": on}));
                }
                Ok(json!({"tracks": out}))
            }
        ),
        cmd!(
            "track.convert_aux_to_folder",
            "Convert Aux to Routing Folder",
            ["Track"],
            None,
            "{tracks?} — aux inputs become routing folders; tracks feeding their input bus become members",
            has_selection,
            convert_aux
        ),
        cmd!(
            "track.extract_midi",
            "Extract MIDI to New Track",
            ["Track"],
            None,
            "{tracks?} — copies the MIDI clips of each MIDI/instrument track to a new MIDI track",
            has_selection,
            extract_midi
        ),
        cmd!(noundo "track.save_preset", "Save Track Preset...", ["Track"], None, "{path, track?} — writes the track's type, width, mixer, inserts, sends and instrument as JSON", has_selection, save_preset),
        cmd!(
            "track.load_preset",
            "Load Track Preset",
            [],
            None,
            "{path, tracks?} — applies a saved track preset's mixer, inserts, sends and instrument",
            has_selection,
            load_preset
        ),
        cmd!("track.bypass_inserts_ae", "Inserts A-E", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, |e, p| {
            bypass_where(e, p, |i, _| i < 5)
        }),
        cmd!("track.bypass_inserts_fj", "Inserts F-J", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, |e, p| {
            bypass_where(e, p, |i, _| (5..10).contains(&i))
        }),
        cmd!("track.bypass_eq", "EQ", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool} — EQ plug-ins only", has_selection, |e, p| {
            bypass_where(e, p, |_, c| c == Some(Category::Eq))
        }),
        cmd!("track.bypass_dynamics", "Dynamics", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, |e, p| bypass_where(
            e,
            p,
            |_, c| c == Some(Category::Dynamics)
        )),
        cmd!("track.bypass_reverb", "Reverb", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, |e, p| bypass_where(
            e,
            p,
            |_, c| c == Some(Category::Reverb)
        )),
        cmd!("track.bypass_delay", "Delay", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, |e, p| bypass_where(
            e,
            p,
            |_, c| c == Some(Category::Delay)
        )),
        cmd!("track.bypass_modulation", "Modulation", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, |e, p| {
            bypass_where(e, p, |_, c| c == Some(Category::Modulation))
        }),
        cmd!(
            "track.mute_sends_ae",
            "Sends A-E",
            ["Track", "Mute Sends"],
            None,
            "{tracks?, mute?: bool} (toggles when omitted)",
            has_selection,
            |e, p| mute_sends_where(e, p, 0, 5)
        ),
        cmd!(
            "track.mute_sends_fj",
            "Sends F-J",
            ["Track", "Mute Sends"],
            None,
            "{tracks?, mute?: bool} (toggles when omitted)",
            has_selection,
            |e, p| mute_sends_where(e, p, 5, 10)
        ),
        cmd!(
            "track.write_midi_rtp",
            "Write MIDI Real-Time Properties",
            ["Track"],
            None,
            "{tracks?} — writes the track's real-time properties (velocity, transpose, duration, delay) into its notes and resets them",
            has_selection,
            write_rtp
        ),
        cmd!(
            "track.trim_automation_write",
            "Write Trim Automation",
            [],
            None,
            "{tracks?, start?, end?, db} — writes a relative volume trim (dB) over a range",
            has_selection,
            trim_write
        ),
        cmd!(
            "track.coalesce_trim",
            "Coalesce Trim Automation",
            ["Track"],
            None,
            "{tracks?} — adds trim automation into volume automation and removes the trim",
            has_selection,
            coalesce_trim
        ),
        cmd!(
            "track.coalesce_vca",
            "Coalesce VCA Controller Automation",
            ["Track"],
            None,
            "{tracks?} — for selected VCA masters: folds VCA level/automation into every member's volume automation, then resets the VCA",
            has_tracks,
            coalesce_vca
        ),
        cmd!(
            "track.designate_target_playlist",
            "Designate as Target Playlist",
            ["Track"],
            None,
            "{tracks?, index?} — the active playlist (or `index`) becomes the target for Copy/Move Selection to",
            has_selection,
            designate_target
        ),
        cmd!(
            "track.show_target_playlist",
            "Show Target Playlist",
            ["Track"],
            None,
            "{tracks?} — makes the target playlist the main playlist",
            has_selection,
            show_target
        ),
        cmd!(
            "track.toggle_recent_playlist",
            "Toggle Recent Playlist",
            ["Track"],
            Some("Ctrl+Alt+Shift+Left"),
            "{tracks?} — swaps to the previously shown playlist",
            has_selection,
            toggle_recent
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::more_util::track_key;
    use super::automation::trim_param;
    use soundcraft_model::{AutoParam, ClipContent, Route, TrackId, TrackKind};

    use super::*;

    #[test]
    fn bypass_by_category_and_range() {
        let mut e = crate::demo::demo_engine();
        let kick = e.session().track_by_name("Kick").unwrap().id;
        e.execute("edit.select", &json!({"tracks": [kick.0]})).unwrap();
        e.execute("track.bypass_dynamics", &json!({})).unwrap();
        let tr = e.session().track(kick).unwrap();
        assert!(tr.mixer.inserts[1].as_ref().unwrap().bypass, "compressor bypassed");
        assert!(!tr.mixer.inserts[0].as_ref().unwrap().bypass, "EQ untouched");
        e.execute("track.bypass_eq", &json!({})).unwrap();
        assert!(e.session().track(kick).unwrap().mixer.inserts[0].as_ref().unwrap().bypass);
        e.execute("track.bypass_inserts_ae", &json!({})).unwrap();
        let tr = e.session().track(kick).unwrap();
        assert!(tr.mixer.inserts.iter().flatten().all(|i| !i.bypass), "A-E toggles back off when all are bypassed");
    }

    #[test]
    fn mute_sends_subset() {
        let mut e = crate::demo::demo_engine();
        e.execute("edit.select", &json!({"tracks": ["Snare"]})).unwrap();
        e.execute("track.mute_sends_fj", &json!({})).unwrap();
        assert!(!e.session().track_by_name("Snare").unwrap().mixer.sends[0].as_ref().unwrap().mute);
        e.execute("track.mute_sends_ae", &json!({})).unwrap();
        assert!(e.session().track_by_name("Snare").unwrap().mixer.sends[0].as_ref().unwrap().mute);
    }

    #[test]
    fn extract_midi_copies_clips() {
        let mut e = crate::demo::demo_engine();
        e.execute("edit.select", &json!({"tracks": ["Keys"]})).unwrap();
        let r = e.execute("track.extract_midi", &json!({})).unwrap();
        let nt = TrackId(r["tracks"][0]["track"].as_u64().unwrap());
        let tr = e.session().track(nt).unwrap();
        assert_eq!(tr.kind, TrackKind::Midi);
        assert_eq!(tr.clips().len(), e.session().track_by_name("Keys").unwrap().clips().len());
        assert_ne!(tr.clips()[0].id, e.session().track_by_name("Keys").unwrap().clips()[0].id);
    }

    #[test]
    fn convert_aux_adopts_members() {
        let mut e = crate::demo::demo_engine();
        let s = e.session_mut();
        let verb = s.track_by_name("Verb").unwrap().id;
        let bus = match s.track(verb).unwrap().mixer.input.clone() {
            Route::Bus(b) => b,
            _ => panic!("demo verb is fed by a bus"),
        };
        let pad = s.track_by_name("Pad").unwrap().id;
        s.track_mut(pad).unwrap().mixer.output = Route::Bus(bus);
        e.execute("edit.select", &json!({"tracks": [verb.0]})).unwrap();
        e.execute("track.convert_aux_to_folder", &json!({})).unwrap();
        assert_eq!(e.session().track(verb).unwrap().kind, TrackKind::Folder);
        assert_eq!(e.session().track(pad).unwrap().folder, Some(verb));
    }

    #[test]
    fn preset_round_trip() {
        let dir = std::env::temp_dir().join(format!("sc-preset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bass.json").to_string_lossy().into_owned();
        let mut e = crate::demo::demo_engine();
        e.execute("edit.select", &json!({"tracks": ["Bass"]})).unwrap();
        e.execute("track.save_preset", &json!({"path": path})).unwrap();
        e.execute("edit.select", &json!({"tracks": ["Hats"]})).unwrap();
        e.execute("track.load_preset", &json!({"path": path})).unwrap();
        let hats = e.session().track_by_name("Hats").unwrap();
        assert_eq!(hats.mixer.inserts[0].as_ref().map(|i| i.plugin.as_str()), Some("compressor"));
        assert_eq!(hats.mixer.volume_db, -5.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rtp_written_into_notes() {
        let mut e = crate::demo::demo_engine();
        let keys = e.session().track_by_name("Keys").unwrap().id;
        let first = |e: &Engine| match &e.session().track(keys).unwrap().clips()[0].content {
            ClipContent::Midi { sequence } => sequence.notes[0],
            _ => panic!(),
        };
        let before = first(&e);
        let s = e.session_mut();
        s.edit.values.insert(track_key("rtp", keys, "transpose"), 12.0);
        s.edit.values.insert(track_key("rtp", keys, "velocity"), -10.0);
        e.execute("edit.select", &json!({"tracks": [keys.0]})).unwrap();
        e.execute("track.write_midi_rtp", &json!({})).unwrap();
        let after = first(&e);
        assert_eq!(after.pitch, before.pitch + 12);
        assert_eq!(after.velocity, before.velocity - 10);
        assert!(!e.session().edit.values.keys().any(|k| k.starts_with(&format!("rtp.{}.", keys.0))));
    }

    #[test]
    fn trim_and_vca_coalesce() {
        let mut e = Engine::default();
        e.execute("track.new", &json!({"count": 2})).unwrap();
        e.execute("track.new", &json!({"kind": "vca"})).unwrap();
        let ids: Vec<TrackId> = e.session().tracks.iter().map(|t| t.id).collect();
        let (a, b, v) = (ids[0], ids[1], ids[2]);
        let s = e.session_mut();
        s.track_mut(a).unwrap().mixer.vca = Some(v.0);
        s.track_mut(b).unwrap().mixer.vca = Some(v.0);
        s.track_mut(b).unwrap().mixer.volume_db = -3.0;
        let l = s.track_mut(v).unwrap().lane_mut(&AutoParam::Volume);
        l.set_point(0, 0.0);
        l.set_point(1000, -6.0);
        e.execute("edit.select", &json!({"tracks": [v.0]})).unwrap();
        e.execute("track.coalesce_vca", &json!({})).unwrap();
        let s = e.session();
        assert!((s.track(a).unwrap().lane(&AutoParam::Volume).unwrap().value_at(1000, 0.0) + 6.0).abs() < 1e-4);
        assert!((s.track(b).unwrap().lane(&AutoParam::Volume).unwrap().value_at(500, 0.0) + 6.0).abs() < 1e-4);
        assert!(s.track(v).unwrap().lane(&AutoParam::Volume).is_none());
        // Trim.
        e.execute("edit.select", &json!({"tracks": [a.0]})).unwrap();
        e.execute("track.trim_automation_write", &json!({"start": 2000, "end": 3000, "db": -4.0})).unwrap();
        e.execute("track.coalesce_trim", &json!({})).unwrap();
        let tr = e.session().track(a).unwrap();
        assert!((tr.lane(&AutoParam::Volume).unwrap().value_at(2500, 0.0) + 10.0).abs() < 1e-4);
        assert!(tr.lane(&trim_param()).is_none());
    }

    #[test]
    fn target_and_recent_playlists() {
        let mut e = crate::demo::demo_engine();
        e.execute("edit.select", &json!({"tracks": ["Kick"]})).unwrap();
        e.execute("track.playlist_new", &json!({})).unwrap();
        e.execute("track.designate_target_playlist", &json!({})).unwrap();
        e.execute("track.playlist_select", &json!({"index": 0})).unwrap();
        e.execute("track.show_target_playlist", &json!({})).unwrap();
        assert_eq!(e.session().track_by_name("Kick").unwrap().active_playlist, 1);
        e.execute("track.toggle_recent_playlist", &json!({})).unwrap();
        assert_eq!(e.session().track_by_name("Kick").unwrap().active_playlist, 0);
        e.execute("track.toggle_recent_playlist", &json!({})).unwrap();
        assert_eq!(e.session().track_by_name("Kick").unwrap().active_playlist, 1);
    }
}
