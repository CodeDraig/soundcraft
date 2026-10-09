//! More Edit menu commands: Cut/Copy/Clear Special variants, Paste Special, selection helpers,
//! Space Clips, Snap to, Copy/Move Selection to playlists, TCE, and automation coalesce/trim/glide.

use super::*;
use crate::{TransportRequest, cmd};
use serde_json::json;
use soundcraft_model::{EditMode, Session};
use soundcraft_time::Range;

mod automation_clipboard;
use automation_clipboard::{Lanes, auto_copy, has_automation_clipboard, paste_to_current_automation};
mod clip_gain;
use clip_gain::{coalesce_clip_gain_to_volume, coalesce_volume_to_clip_gain, copy_clip_gain, has_clip_gain_clipboard, paste_clip_gain};
mod clip_effects;
use clip_effects::{clip_effects, has_clip_effects_clipboard, paste_clip_effects};
mod merge_midi;
use merge_midi::paste_merge_midi;
mod merge_markers;
use merge_markers::{has_marker_clipboard, paste_merge_markers};
mod selection;
use selection::{duplicate_extend, extend_members, marker_lane};
mod placement;
use placement::{snap, space_clips};
mod playlists;
use playlists::{Dest, to_playlist};
mod stretch;
use stretch::tce;
mod automation_range;
use automation_range::trim_glide;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "edit.cut_pan_automation",
            "Pan Automation",
            ["Edit", "Cut Special"],
            None,
            "{tracks?, start?, end?} — cuts pan automation only to the clipboard",
            has_range,
            |e, p| auto_copy(e, p, Lanes::Pan, true, "edit.cut_pan_automation")
        ),
        cmd!(
            "edit.cut_plugin_automation",
            "Plugin Automation",
            ["Edit", "Cut Special"],
            None,
            "{tracks?, start?, end?} — cuts plug-in automation only",
            has_range,
            |e, p| auto_copy(e, p, Lanes::Plugin, true, "edit.cut_plugin_automation")
        ),
        cmd!(noundo "edit.copy_all_automation", "All Automation", ["Edit", "Copy Special"], None, "{tracks?, start?, end?} — copies automation only (no clips)", has_range, |e, p| auto_copy(e, p, Lanes::All, false, "edit.copy_all_automation")),
        cmd!(noundo "edit.copy_pan_automation", "Pan Automation", ["Edit", "Copy Special"], None, "{tracks?, start?, end?}", has_range, |e, p| auto_copy(e, p, Lanes::Pan, false, "edit.copy_pan_automation")),
        cmd!(noundo "edit.copy_plugin_automation", "Plugin Automation", ["Edit", "Copy Special"], None, "{tracks?, start?, end?}", has_range, |e, p| auto_copy(e, p, Lanes::Plugin, false, "edit.copy_plugin_automation")),
        cmd!(noundo "edit.copy_clip_gain", "Clip Gain", ["Edit", "Copy Special"], None, "{tracks?, start?, end?} — copies the clip gain curve under the selection", has_range, copy_clip_gain),
        cmd!(
            "edit.paste_clip_gain",
            "Paste Clip Gain",
            [],
            None,
            "{tracks?, at?} — pastes copied clip gain onto the clips under the paste range",
            has_clip_gain_clipboard,
            paste_clip_gain
        ),
        cmd!(noundo "edit.copy_clip_effects", "Clip Effects", ["Edit", "Copy Special"], None, "{clips?} — copies the first clip's clip-effect settings", has_selection, |e, p| clip_effects(e, p, true, false, "edit.copy_clip_effects")),
        cmd!(
            "edit.cut_clip_effects",
            "Clip Effects",
            ["Edit", "Cut Special"],
            None,
            "{clips?} — copies then removes clip-effect settings",
            has_selection,
            |e, p| clip_effects(e, p, true, true, "edit.cut_clip_effects")
        ),
        cmd!(
            "edit.clear_clip_effects",
            "Clip Effects",
            ["Edit", "Clear Special"],
            None,
            "{clips?} — removes clip-effect settings (and bypass)",
            has_selection,
            |e, p| clip_effects(e, p, false, true, "edit.clear_clip_effects")
        ),
        cmd!("edit.paste_clip_effects", "Paste Clip Effects", [], None, "{clips?}", has_clip_effects_clipboard, paste_clip_effects),
        cmd!(
            "edit.paste_merge_midi",
            "Merge MIDI",
            ["Edit", "Paste Special"],
            None,
            "{tracks?, at?} — merges copied MIDI notes into existing MIDI clips instead of replacing them",
            has_clipboard,
            paste_merge_midi
        ),
        cmd!(
            "edit.paste_merge_markers",
            "Merge Markers",
            ["Edit", "Paste Special"],
            None,
            "{at?} — adds the copied markers at the insertion point, skipping any that already exist there",
            has_marker_clipboard,
            paste_merge_markers
        ),
        cmd!(
            "edit.paste_to_current_automation",
            "To Current Automation Type",
            ["Edit", "Paste Special"],
            None,
            "{tracks?, at?, param?} — pastes copied automation into the shown lane (track view) or `param`, rescaled to its range",
            has_automation_clipboard,
            paste_to_current_automation
        ),
        cmd!(noundo "edit.play_timeline", "Play Timeline", ["Edit", "Selection"], None, "{} — plays the timeline selection", always, |e, _| {
            let s = e.session();
            let r = if s.edit.timeline_selection.is_empty() { s.edit.selection } else { s.edit.timeline_selection };
            e.transport_requests.push(TransportRequest::Locate(r.start));
            e.transport_requests.push(TransportRequest::Play);
            Ok(json!({"start": r.start, "end": r.end}))
        }),
        cmd!(
            "edit.duplicate_extend",
            "Duplicate and Extend Edit",
            ["Edit", "Selection"],
            None,
            "{tracks?, start?, end?} — duplicates the selection and extends the selection over the copy",
            has_range,
            duplicate_extend
        ),
        cmd!(noundo "edit.extend_up_to_members", "Extend Edit Up to Members", ["Edit", "Selection"], None, "{} — adds the parent folder of each selected member and its members", has_selection, |e, _| extend_members(e, true)),
        cmd!(noundo "edit.extend_down_to_members", "Extend Edit Down to Members", ["Edit", "Selection"], None, "{} — adds the members of selected folder tracks (recursively)", has_selection, |e, _| extend_members(e, false)),
        cmd!(noundo "edit.marker_lane_move_up", "Move Edit Selection To Marker Lane Above", ["Edit", "Selection"], None, "{} — the selection's marker ruler lane (1..5) moves up", always, |e, _| marker_lane(e, -1, false)),
        cmd!(noundo "edit.marker_lane_move_down", "Move Edit Selection To Marker Lane Below", ["Edit", "Selection"], None, "{}", always, |e, _| marker_lane(e, 1, false)),
        cmd!(noundo "edit.marker_lane_extend_up", "Extend Edit Selection To Marker Lane Above", ["Edit", "Selection"], None, "{} — adds the marker ruler lane above to the selection", always, |e, _| marker_lane(e, -1, true)),
        cmd!(noundo "edit.marker_lane_extend_down", "Extend Edit Selection To Marker Lane Below", ["Edit", "Selection"], None, "{}", always, |e, _| marker_lane(e, 1, true)),
        cmd!(
            "edit.space_clips",
            "Space Clips…",
            ["Edit"],
            None,
            "{clips?, gap?: samples|{seconds} (default 0), mode?: end_to_start|start_to_start}",
            has_selection,
            space_clips
        ),
        cmd!(
            "edit.snap_next",
            "Next",
            ["Edit", "Snap to"],
            None,
            "{clips?} — moves each clip so its end meets the start of the next clip",
            has_selection,
            |e, p| snap(e, p, true)
        ),
        cmd!(
            "edit.snap_previous",
            "Previous",
            ["Edit", "Snap to"],
            None,
            "{clips?} — moves each clip so its start meets the end of the previous clip",
            has_selection,
            |e, p| snap(e, p, false)
        ),
        cmd!(
            "edit.copy_to_target_playlist",
            "Target Playlist",
            ["Edit", "Copy Selection to..."],
            None,
            "{tracks?, start?, end?} — copies the selection into each track's target playlist",
            has_range,
            |e, p| to_playlist(e, p, Dest::Target, false, "edit.copy_to_target_playlist")
        ),
        cmd!(
            "edit.copy_to_duplicate_playlist",
            "Duplicate of Main Playlist",
            ["Edit", "Copy Selection to..."],
            None,
            "{tracks?, start?, end?}",
            has_range,
            |e, p| to_playlist(e, p, Dest::Duplicate, false, "edit.copy_to_duplicate_playlist")
        ),
        cmd!(
            "edit.move_to_target_playlist",
            "Target Playlist",
            ["Edit", "Move Selection to..."],
            None,
            "{tracks?, start?, end?} — copies to the target playlist and clears it from the main",
            has_range,
            |e, p| to_playlist(e, p, Dest::Target, true, "edit.move_to_target_playlist")
        ),
        cmd!("edit.move_to_new_playlist", "New Playlist", ["Edit", "Move Selection to..."], None, "{tracks?, start?, end?}", has_range, |e, p| {
            to_playlist(e, p, Dest::New, true, "edit.move_to_new_playlist")
        }),
        cmd!(
            "edit.move_to_duplicate_playlist",
            "Duplicate of Main Playlist",
            ["Edit", "Move Selection to..."],
            None,
            "{tracks?, start?, end?}",
            has_range,
            |e, p| to_playlist(e, p, Dest::Duplicate, true, "edit.move_to_duplicate_playlist")
        ),
        cmd!(
            "edit.tce_to_timeline",
            "TCE Edit to Timeline Selection",
            ["Edit"],
            None,
            "{clip?, start?, end?} — time-stretches the clip to fill the timeline selection (pitch preserved)",
            has_selection,
            tce
        ),
        cmd!(
            "edit.automation_coalesce_volume_to_clip_gain",
            "Coalesce Volume Automation to Clip Gain",
            ["Edit", "Automation"],
            None,
            "{tracks?} — folds the whole volume curve into clip gain; the fader goes to 0 dB",
            has_selection,
            coalesce_volume_to_clip_gain
        ),
        cmd!(
            "edit.automation_coalesce_clip_gain_to_volume",
            "Coalesce Clip Gain to Volume Automation",
            ["Edit", "Automation"],
            None,
            "{tracks?} — folds all clip gain into volume automation",
            has_selection,
            coalesce_clip_gain_to_volume
        ),
        cmd!(
            "edit.automation_trim_to_all",
            "Trim to All Enabled",
            ["Edit", "Automation"],
            None,
            "{tracks?, start?, end?} — offsets every enabled lane in the selection so it starts at the current control value",
            has_range,
            |e, p| trim_glide(e, p, false, false, "edit.automation_trim_to_all")
        ),
        cmd!(
            "edit.automation_glide_to_all",
            "Glide to All Enabled",
            ["Edit", "Automation"],
            None,
            "{tracks?, start?, end?} — ramps every enabled lane from its value at the start to the current value at the end",
            has_range,
            |e, p| trim_glide(e, p, true, false, "edit.automation_glide_to_all")
        ),
        cmd!("edit.automation_glide_pan", "Glide Pan Only", ["Edit", "Automation"], None, "{tracks?, start?, end?}", has_range, |e, p| trim_glide(
            e,
            p,
            true,
            true,
            "edit.automation_glide_pan"
        )),
    ]
}

fn shuffle(s: &Session) -> bool {
    s.edit.edit_mode == EditMode::Shuffle
}

fn nonempty_range(e: &Engine, p: &Value, id: &str) -> Result<Range> {
    let r = range_param(e, id, p)?;
    if r.is_empty() { Err(bad(id, "the selection is empty")) } else { Ok(r) }
}

#[cfg(test)]
mod tests {
    use super::super::more_util::clip_db;
    use crate::edit as ops;
    use soundcraft_model::{AutoParam, Clip, ClipContent, ClipId};
    use soundcraft_time::Samples;

    use super::*;
    use soundcraft_model::{ChannelFormat, SourceId, TrackKind};

    #[test]
    fn merge_markers_pastes_copied_markers_once() {
        let mut e = crate::demo::demo_engine();
        let first = e.session().markers.first().cloned().expect("demo has markers");
        let r = json!({"start": first.start.saturating_sub(10).max(0), "end": first.start + 10});
        e.execute("edit.select", &json!({"tracks": ["Kick"], "start": r["start"], "end": r["end"]})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        let before = e.session().markers.len();
        let at = 5_000_000;
        let out = e.execute("edit.paste_merge_markers", &json!({"at": at})).unwrap();
        assert_eq!(out["added"], 1);
        assert_eq!(e.session().markers.len(), before + 1);
        assert!(e.session().markers.iter().any(|m| m.name == first.name && m.start == at + first.start - r["start"].as_i64().unwrap()));
        // Pasting again at the same place adds nothing.
        assert_eq!(e.execute("edit.paste_merge_markers", &json!({"at": at})).unwrap()["added"], 0);
    }

    fn session_with_clips(starts: &[(Samples, Samples)]) -> (Engine, soundcraft_model::TrackId, Vec<ClipId>) {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let mut ids = Vec::new();
        for (st, len) in starts {
            let id = s.new_clip_id();
            ops::place_clip(&mut s, t, Clip::audio(id, "c", SourceId(1), 0, *st, *len));
            ids.push(id);
        }
        s.edit.selected_tracks = vec![t];
        (Engine::new(s), t, ids)
    }

    fn starts(e: &Engine, t: soundcraft_model::TrackId) -> Vec<Samples> {
        e.session().track(t).map(|tr| tr.clips().iter().map(|c| c.start).collect()).unwrap_or_default()
    }

    #[test]
    fn space_clips_butts_with_gap() {
        let (mut e, t, ids) = session_with_clips(&[(0, 100), (500, 100), (2000, 50)]);
        e.execute("edit.space_clips", &json!({"clips": ids.iter().map(|c| c.0).collect::<Vec<_>>(), "gap": 10})).unwrap();
        assert_eq!(starts(&e, t), vec![0, 110, 220]);
        assert!(e.can_undo());
    }

    #[test]
    fn snap_next_and_previous() {
        let (mut e, t, ids) = session_with_clips(&[(0, 100), (300, 100), (1000, 100)]);
        e.execute("edit.snap_previous", &json!({"clip": ids[1].0})).unwrap();
        assert_eq!(starts(&e, t), vec![0, 100, 1000]);
        e.execute("edit.snap_next", &json!({"clip": ids[1].0})).unwrap();
        assert_eq!(starts(&e, t), vec![0, 900, 1000]);
    }

    #[test]
    fn copy_and_paste_automation_only() {
        let (mut e, t, _) = session_with_clips(&[(0, 1000)]);
        e.execute("automation.set_point", &json!({"track": t.0, "param": "pan", "at": 100, "value": -1.0})).unwrap();
        e.execute("automation.set_point", &json!({"track": t.0, "param": "pan", "at": 200, "value": 1.0})).unwrap();
        e.execute("automation.set_point", &json!({"track": t.0, "param": "volume", "at": 150, "value": -6.0})).unwrap();
        e.execute("edit.cut_pan_automation", &json!({"start": 50, "end": 250})).unwrap();
        assert_eq!(e.clipboard.automation[0].len(), 1, "only the pan lane is cut");
        assert!(e.session().track(t).unwrap().lane(&AutoParam::Pan(0)).unwrap().points.is_empty());
        assert_eq!(e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().points.len(), 1);
        // Paste pan into the volume lane, rescaled from -1..1 to -144..12.
        e.execute("track.view", &json!({"view": "volume"})).unwrap();
        e.execute("edit.paste_to_current_automation", &json!({"at": 1000})).unwrap();
        let vol = e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().clone();
        assert!((vol.value_at(1150, 0.0) - 12.0).abs() < 1e-3, "{:?}", vol.points);
        assert!((vol.value_at(1050, 0.0) + 144.0).abs() < 1e-3);
    }

    #[test]
    fn clip_gain_copy_paste() {
        let (mut e, t, ids) = session_with_clips(&[(0, 1000), (2000, 1000)]);
        e.execute("clip.gain", &json!({"clip": ids[0].0, "db": -6.0})).unwrap();
        e.execute("edit.copy_clip_gain", &json!({"start": 0, "end": 1000})).unwrap();
        e.execute("edit.paste_clip_gain", &json!({"at": 2000})).unwrap();
        let c = e.session().track(t).unwrap().clips()[1].clone();
        assert!((clip_db(&c, 500) + 6.0).abs() < 1e-3, "{c:?}");
    }

    #[test]
    fn move_to_new_playlist_clears_main() {
        let (mut e, t, _) = session_with_clips(&[(0, 1000)]);
        e.execute("edit.move_to_new_playlist", &json!({"start": 200, "end": 400})).unwrap();
        let tr = e.session().track(t).unwrap();
        assert_eq!(tr.playlists.len(), 2);
        assert_eq!(tr.active_playlist, 0);
        assert_eq!(tr.playlists[1].clips.len(), 1);
        assert_eq!(tr.playlists[1].clips[0].range(), Range::new(200, 400));
        assert!(tr.clips().iter().all(|c| !c.range().overlaps(&Range::new(200, 400))));
        // Target playlist needs a designation first.
        assert!(e.execute("edit.copy_to_target_playlist", &json!({"start": 0, "end": 100})).is_err());
        e.execute("track.playlist_select", &json!({"index": 1})).unwrap();
        e.execute("track.designate_target_playlist", &json!({})).unwrap();
        e.execute("track.playlist_select", &json!({"index": 0})).unwrap();
        e.execute("edit.copy_to_target_playlist", &json!({"start": 0, "end": 100})).unwrap();
        assert_eq!(e.session().track(t).unwrap().playlists[1].clips.len(), 2);
    }

    #[test]
    fn coalesce_volume_into_clip_gain_and_back() {
        let (mut e, t, _) = session_with_clips(&[(0, 1000)]);
        e.execute("automation.set_point", &json!({"track": t.0, "at": 0, "value": -10.0})).unwrap();
        e.execute("automation.set_point", &json!({"track": t.0, "at": 1000, "value": 0.0})).unwrap();
        e.execute("edit.automation_coalesce_volume_to_clip_gain", &json!({})).unwrap();
        let c = e.session().track(t).unwrap().clips()[0].clone();
        assert!((clip_db(&c, 500) + 5.0).abs() < 1e-3);
        assert!(e.session().track(t).unwrap().lane(&AutoParam::Volume).is_none());
        e.execute("edit.automation_coalesce_clip_gain_to_volume", &json!({})).unwrap();
        let tr = e.session().track(t).unwrap();
        assert!((tr.lane(&AutoParam::Volume).unwrap().value_at(500, 0.0) + 5.0).abs() < 0.1);
        assert_eq!(tr.clips()[0].gain_db, 0.0);
    }

    #[test]
    fn glide_and_trim() {
        let (mut e, t, _) = session_with_clips(&[(0, 1000)]);
        e.execute("automation.set_point", &json!({"track": t.0, "at": 0, "value": -10.0})).unwrap();
        e.execute("mix.volume", &json!({"db": 0.0})).unwrap();
        e.execute("edit.automation_glide_to_all", &json!({"start": 100, "end": 300})).unwrap();
        let l = e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().clone();
        assert!((l.value_at(200, 0.0) + 5.0).abs() < 0.1);
        assert!((l.value_at(600, 0.0) + 10.0).abs() < 1e-3, "after the glide the old curve resumes");
        e.execute("edit.automation_trim_to_all", &json!({"start": 400, "end": 500})).unwrap();
        let l = e.session().track(t).unwrap().lane(&AutoParam::Volume).unwrap().clone();
        assert!((l.value_at(450, 0.0)).abs() < 1e-3);
        assert!((l.value_at(700, 0.0) + 10.0).abs() < 1e-3);
    }

    #[test]
    fn merge_midi_keeps_existing_notes() {
        let mut e = crate::demo::demo_engine();
        let keys = e.session().track_by_name("Keys").unwrap().id;
        let clip = e.session().track(keys).unwrap().clips()[0].clone();
        let before = match &clip.content {
            ClipContent::Midi { sequence } => sequence.notes.len(),
            _ => 0,
        };
        let r = Range::new(clip.start, clip.start + 48_000);
        e.execute("edit.select", &json!({"tracks": [keys.0], "start": r.start, "end": r.end})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        let copied = match &e.clipboard.tracks[0][0].content {
            ClipContent::Midi { sequence } => sequence.notes.len(),
            _ => 0,
        };
        e.execute("edit.paste_merge_midi", &json!({"at": clip.start + 96_000})).unwrap();
        let after = match &e.session().track(keys).unwrap().clips()[0].content {
            ClipContent::Midi { sequence } => sequence.notes.len(),
            _ => 0,
        };
        assert_eq!(after, before + copied);
    }

    #[test]
    fn tce_fits_timeline() {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let tone: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let buf = soundcraft_audio_io::AudioBuffer { sample_rate: 48_000, channels: vec![tone] };
        let src = crate::io::add_source(&mut s, "tone", buf, None, soundcraft_audio_io::FileFormat::Wav);
        let id = s.new_clip_id();
        ops::place_clip(&mut s, t, Clip::audio(id, "tone", src, 0, 0, 24_000));
        s.edit.selected_tracks = vec![t];
        s.edit.selected_clips = vec![id];
        s.edit.timeline_selection = Range::new(1000, 31_000);
        let mut e = Engine::new(s);
        let r = e.execute("edit.tce_to_timeline", &json!({})).unwrap();
        assert!((r["ratio"].as_f64().unwrap() - 1.25).abs() < 1e-9);
        let nc = e.session().find_clip(id).unwrap().1.clone();
        assert_eq!((nc.start, nc.length), (1000, 30_000));
        let nsrc = nc.source().unwrap();
        assert_ne!(nsrc, src);
        assert_eq!(e.session().source(nsrc).unwrap().frames, 30_000);
        assert!(e.session().pool.get(nsrc).unwrap().buffer.channels[0].iter().any(|v| v.abs() > 0.1));
    }

    #[test]
    fn extend_to_members() {
        let mut e = Engine::default();
        e.execute("track.new", &json!({"count": 2})).unwrap();
        e.execute("track.move_to_new_folder", &json!({})).unwrap();
        let folder = e.session().tracks.iter().find(|t| t.is_folder()).unwrap().id;
        e.execute("edit.select", &json!({"tracks": [folder.0]})).unwrap();
        e.execute("edit.extend_down_to_members", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_tracks.len(), 3);
        let member = e.session().tracks.iter().find(|t| t.folder == Some(folder)).unwrap().id;
        e.execute("edit.select", &json!({"tracks": [member.0]})).unwrap();
        e.execute("edit.extend_up_to_members", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_tracks.len(), 3);
    }

    #[test]
    fn marker_lane_selection() {
        let mut e = Engine::default();
        e.execute("edit.marker_lane_move_down", &json!({})).unwrap();
        e.execute("edit.marker_lane_extend_down", &json!({})).unwrap();
        let r = e.execute("edit.marker_lane_extend_up", &json!({})).unwrap();
        assert_eq!(r["lanes"], json!([1, 3]));
        let r = e.execute("edit.marker_lane_move_up", &json!({})).unwrap();
        assert_eq!(r["lanes"], json!([1, 1]));
    }

    #[test]
    fn duplicate_and_extend() {
        let (mut e, t, _) = session_with_clips(&[(0, 100)]);
        e.execute("edit.duplicate_extend", &json!({"start": 0, "end": 100})).unwrap();
        assert_eq!(starts(&e, t), vec![0, 100]);
        assert_eq!(e.session().edit.selection, Range::new(0, 200));
    }
}
