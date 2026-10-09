//! More Clip menu commands: Regroup, Unloop, Clip Effects bypass/render, Conform to Tempo and
//! Remove Pitch Shift.

use super::*;
use crate::cmd;

mod looping;
use looping::{regroup, unloop};
mod effects;
use effects::{effects_bypass, effects_render, effects_set};
mod elastic;
use elastic::{conform, remove_pitch_shift};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "clip.regroup",
            "Regroup",
            ["Clip"],
            Some("Cmd+Alt+R"),
            "{clips?} — puts the clips back into one clip group (reusing their most common group)",
            has_selection,
            regroup
        ),
        cmd!(
            "clip.unloop",
            "Unloop...",
            ["Clip"],
            None,
            "{clips?, mode?: remove|flatten} — remove = keep only the original clip; flatten = keep the copies as ordinary clips",
            has_selection,
            unloop
        ),
        cmd!(
            "clip.effects_set",
            "Set Clip Effects",
            [],
            None,
            "{clips?, params: {name: number}} — stores clip-effect settings on the clips",
            has_selection,
            effects_set
        ),
        cmd!(
            "clip.effects_bypass",
            "Bypass",
            ["Clip", "Clip Effects"],
            None,
            "{clips?, value?: bool} (toggles when omitted)",
            has_selection,
            effects_bypass
        ),
        cmd!(
            "clip.effects_render",
            "Render",
            ["Clip", "Clip Effects"],
            None,
            "{clips?} — renders clip gain and fades into new audio and clears the clip effects",
            has_selection,
            effects_render
        ),
        cmd!(
            "clip.conform_to_tempo",
            "Conform to Tempo",
            ["Clip"],
            None,
            "{clips?, source_bpm?} — stretches audio clips so their length in bars follows the session tempo; the first conform records the clip's tempo",
            has_selection,
            conform
        ),
        cmd!(
            "clip.remove_pitch_shift",
            "Remove Pitch Shift",
            ["Clip"],
            None,
            "{clips?} — re-renders vari-speed (stretched) clips with pitch preserved",
            has_selection,
            remove_pitch_shift
        ),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use soundcraft_model::{Clip, ClipId, Session, TrackId};
    use soundcraft_time::Samples;

    use super::*;
    use soundcraft_model::{ChannelFormat, SourceId, TrackKind};

    fn engine_with(clips: &[(Samples, Samples, Samples)]) -> (Engine, TrackId, Vec<ClipId>) {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let mut ids = Vec::new();
        for (st, len, off) in clips {
            let id = s.new_clip_id();
            crate::edit::place_clip(&mut s, t, Clip::audio(id, "c", SourceId(9), *off, *st, *len));
            ids.push(id);
        }
        s.edit.selected_tracks = vec![t];
        (Engine::new(s), t, ids)
    }

    #[test]
    fn unloop_removes_copies() {
        let (mut e, t, ids) = engine_with(&[(0, 100, 0), (100, 100, 0), (200, 100, 0), (300, 100, 50)]);
        e.execute("clip.unloop", &json!({"clip": ids[1].0})).unwrap();
        let left: Vec<ClipId> = e.session().track(t).unwrap().clips().iter().map(|c| c.id).collect();
        assert_eq!(left, vec![ids[0], ids[3]]);
        assert!(e.execute("clip.unloop", &json!({"clip": ids[3].0})).is_err());
    }

    #[test]
    fn regroup_reuses_group() {
        let (mut e, _, ids) = engine_with(&[(0, 100, 0), (500, 100, 0), (900, 100, 0)]);
        let all: Vec<u64> = ids.iter().map(|c| c.0).collect();
        e.execute("clip.group", &json!({"clips": all})).unwrap();
        let g = e.session().find_clip(ids[0]).unwrap().1.group;
        e.execute("clip.ungroup", &json!({"clips": [ids[2].0]})).unwrap();
        e.execute("clip.regroup", &json!({"clips": all})).unwrap();
        assert!(ids.iter().all(|id| e.session().find_clip(*id).unwrap().1.group == g));
    }

    #[test]
    fn effects_bypass_and_clear() {
        let (mut e, _, ids) = engine_with(&[(0, 100, 0)]);
        e.execute("clip.effects_set", &json!({"clip": ids[0].0, "params": {"eq_gain_db": 3.0}})).unwrap();
        e.execute("clip.effects_bypass", &json!({"clip": ids[0].0})).unwrap();
        assert!(e.session().edit.flag(&format!("clip_fx.bypass.{}", ids[0].0)));
        e.execute("edit.copy_clip_effects", &json!({"clip": ids[0].0})).unwrap();
        assert_eq!(e.clipboard.clip_effects, vec![("eq_gain_db".to_string(), 3.0)]);
        e.execute("edit.clear_clip_effects", &json!({"clip": ids[0].0})).unwrap();
        assert!(e.session().edit.values.is_empty());
        e.execute("edit.paste_clip_effects", &json!({"clip": ids[0].0})).unwrap();
        assert_eq!(e.session().edit.values.len(), 1);
    }

    #[test]
    fn conform_follows_tempo() {
        let (mut e, _, ids) = engine_with(&[(0, 48_000, 0)]);
        e.execute("clip.conform_to_tempo", &json!({"clip": ids[0].0})).unwrap();
        assert_eq!(e.session().find_clip(ids[0]).unwrap().1.length, 48_000);
        e.execute("event.tempo", &json!({"bpm": 60.0})).unwrap();
        e.execute("clip.conform_to_tempo", &json!({"clip": ids[0].0})).unwrap();
        let c = e.session().find_clip(ids[0]).unwrap().1.clone();
        assert_eq!(c.length, 96_000);
        assert!((c.stretch - 2.0).abs() < 1e-9);
    }

    #[test]
    fn remove_pitch_shift_renders() {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let tone: Vec<f32> = (0..12_000).map(|i| (i as f32 * 0.07).sin() * 0.5).collect();
        let buf = soundcraft_audio_io::AudioBuffer { sample_rate: 48_000, channels: vec![tone] };
        let src = crate::io::add_source(&mut s, "tone", buf, None, soundcraft_audio_io::FileFormat::Wav);
        let id = s.new_clip_id();
        let mut c = Clip::audio(id, "tone", src, 0, 0, 18_000);
        c.stretch = 1.5;
        crate::edit::place_clip(&mut s, t, c);
        s.edit.selected_clips = vec![id];
        s.edit.selected_tracks = vec![t];
        let mut e = Engine::new(s);
        let r = e.execute("clip.remove_pitch_shift", &json!({})).unwrap();
        assert_eq!(r["clips"], 1);
        let c = e.session().find_clip(id).unwrap().1.clone();
        assert_eq!(c.stretch, 1.0);
        assert_eq!(c.length, 18_000);
        assert_ne!(c.source(), Some(src));
    }
}
