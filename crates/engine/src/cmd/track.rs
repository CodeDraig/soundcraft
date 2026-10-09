//! Track menu commands.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::TrackKind;

mod groups;
use groups::{delete_group, group, toggle_group};
mod structure;
use structure::{change_width, delete, duplicate, move_to_folder, move_track, new_track, split_mono};
mod presentation;
use presentation::{color, comments, height, rename, view};
mod playlists;
use playlists::{new_playlist, select_playlist};
mod routing;
use routing::{bypass_inserts, mute_sends, route};
mod rendering;
use rendering::{commit, create_click};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "track.new",
            "New...",
            ["Track"],
            Some("Cmd+Shift+N"),
            "{count?: 1, format?: mono|stereo|5.1…, kind?: audio|aux|master|midi|instrument|vca|folder, name?, timebase?: samples|ticks, instrument?: plugin id}",
            always,
            new_track
        ),
        cmd!("track.group", "Group...", ["Track"], Some("Cmd+G"), "{name?, tracks?, edit?: true, mix?: true}", has_selection, group),
        cmd!("track.ungroup", "Delete Group", [], None, "{group: id|name}", always, delete_group),
        cmd!("track.group_toggle", "Toggle Group Active", [], None, "{group: id|name}", always, toggle_group),
        cmd!(
            "track.duplicate",
            "Duplicate...",
            ["Track"],
            Some("Alt+Shift+D"),
            "{tracks?, playlists?: true, automation?: true, inserts?: true, sends?: true}",
            has_selection,
            duplicate
        ),
        cmd!("track.split_into_mono", "Split into Mono", ["Track"], None, "{tracks?}", has_selection, split_mono),
        cmd!("track.make_inactive", "Make Inactive", ["Track"], None, "{tracks?, inactive?: bool}", has_selection, |e, p| flag(
            e,
            p,
            "track.make_inactive",
            |t, v| t.inactive = v,
            |t| t.inactive
        )),
        cmd!(
            "track.move_to_new_folder",
            "Move to New Folder...",
            ["Track"],
            Some("Cmd+Alt+Shift+N"),
            "{tracks?, name?}",
            has_selection,
            move_to_folder
        ),
        cmd!(
            "track.change_width",
            "Change Track Width",
            ["Track", "Change Track Width"],
            None,
            "{tracks?, format: mono|stereo|…}",
            has_selection,
            change_width
        ),
        cmd!("track.delete", "Delete", ["Track"], None, "{tracks?}", has_selection, delete),
        cmd!("track.rename", "Rename Track", [], None, "{track, name}", has_tracks, rename),
        cmd!("track.comments", "Track Comments", [], None, "{track, comments}", has_tracks, comments),
        cmd!("track.color", "Track Color", [], None, "{tracks?, color: [r,g,b] | palette index}", has_selection, color),
        cmd!("track.height", "Track Height", [], None, "{tracks?, height: micro|mini|small|medium|large|jumbo|extreme}", has_selection, height),
        cmd!("track.view", "Track View", [], None, "{tracks?, view: waveform|blocks|notes|volume|pan|mute|<automation id>}", has_selection, view),
        cmd!("track.hide", "Hide Track", [], None, "{tracks?, hidden?: bool}", has_selection, |e, p| flag(
            e,
            p,
            "track.hide",
            |t, v| t.hidden = v,
            |t| t.hidden
        )),
        cmd!("track.pin", "Pin Track", [], None, "{tracks?}", has_selection, |e, p| flag(e, p, "track.pin", |t, v| t.pinned = v, |t| t.pinned)),
        cmd!("track.move", "Move Track", [], None, "{track, to: index}", has_tracks, move_track),
        cmd!("track.freeze", "Freeze", ["Track"], None, "{tracks?}", has_selection, |e, p| flag(
            e,
            p,
            "track.freeze",
            |t, v| t.frozen = v,
            |t| t.frozen
        )),
        cmd!("track.commit", "Commit...", ["Track"], Some("Alt+Shift+C"), "{tracks?}", has_selection, commit),
        cmd!("track.bounce", "Bounce...", ["Track"], Some("Cmd+Alt+Shift+B"), "{tracks?}", has_selection, commit),
        cmd!("track.bypass_inserts", "All", ["Track", "Bypass Inserts"], None, "{tracks?, bypass?: bool}", has_selection, bypass_inserts),
        cmd!("track.mute_sends", "All", ["Track", "Mute Sends"], None, "{tracks?, mute?: bool}", has_selection, mute_sends),
        cmd!("track.set_record_to_input_only", "Set Record Tracks to Input Only", ["Track"], Some("Alt+K"), "{}", always, |e, _| {
            let s = e.session_mut();
            let mut n = 0;
            for t in &mut s.tracks {
                if t.mixer.record_arm {
                    t.mixer.input_monitor = true;
                    n += 1;
                }
            }
            Ok(json!({"tracks": n}))
        }),
        cmd!(noundo "track.scroll_to", "Scroll to Track...", ["Track"], Some("Cmd+Alt+F"), "{track}", has_tracks, |e, p| {
            let t = track_param(e, "track.scroll_to", p, "track")?.ok_or_else(|| bad("track.scroll_to", "`track` required"))?;
            let s = e.session_mut();
            s.edit.selected_tracks = vec![t];
            Ok(json!({"track": t}))
        }),
        cmd!("track.clear_clip_indicators", "Clear All Clip Indicators", ["Track"], Some("Alt+C"), "{}", always, |_, _| Ok(json!({"cleared": true}))),
        cmd!("track.clear_trim_automation", "Clear Trim Automation", ["Track"], None, "{tracks?}", has_selection, |_, _| Ok(json!({}))),
        cmd!("track.create_click", "Create Click Track", ["Track"], None, "{}", always, create_click),
        cmd!("track.playlist_new", "New Playlist", [], None, "{track?, name?}", has_selection, |e, p| new_playlist(e, p, false)),
        cmd!("track.playlist_duplicate", "Duplicate Playlist", [], None, "{track?, name?}", has_selection, |e, p| new_playlist(e, p, true)),
        cmd!("track.playlist_select", "Select Playlist", [], None, "{track?, index | name}", has_selection, select_playlist),
        cmd!("track.playlist_delete_unused", "Delete Unused Playlists", [], None, "{track?}", has_selection, |e, p| {
            let tracks = tracks_required(e, "track.playlist_delete_unused", p)?;
            let s = e.session_mut();
            let mut n = 0;
            for t in tracks {
                if let Some(tr) = s.track_mut(t)
                    && let Some(active) = tr.playlists.get(tr.active_playlist).cloned()
                {
                    n += tr.playlists.len() - 1;
                    tr.playlists = vec![active];
                    tr.active_playlist = 0;
                }
            }
            Ok(json!({"deleted": n}))
        }),
        cmd!(noundo "track.folder_toggle", "Open/Close Folder", [], None, "{track, open?: bool}", has_tracks, |e, p| {
            let t = track_param(e, "track.folder_toggle", p, "track")?.ok_or_else(|| bad("track.folder_toggle", "`track` required"))?;
            let v = p.get("open").and_then(Value::as_bool);
            let s = e.session_mut();
            let tr = s.track_mut(t).ok_or_else(|| bad("track.folder_toggle", "no track"))?;
            tr.folder_open = v.unwrap_or(!tr.folder_open);
            Ok(json!({"open": tr.folder_open}))
        }),
        cmd!("track.vca", "Assign to VCA", [], None, "{tracks?, vca: VCA track id|name | null}", has_selection, |e, p| {
            let tracks = tracks_required(e, "track.vca", p)?;
            let vca = track_param(e, "track.vca", p, "vca")?;
            if let Some(v) = vca
                && e.session().track(v).is_none_or(|t| t.kind != TrackKind::Vca)
            {
                return Err(bad("track.vca", "`vca` must be a VCA Master track"));
            }
            let s = e.session_mut();
            for t in &tracks {
                if let Some(tr) = s.track_mut(*t) {
                    tr.mixer.vca = vca.map(|v| v.0);
                }
            }
            Ok(json!({"vca": vca}))
        }),
        cmd!(
            "track.playlist_promote",
            "Promote to Main Playlist",
            [],
            None,
            "{track, playlist: index, start?, end?} (copy that range from an alternate playlist into the active one)",
            has_tracks,
            |e, p| {
                let t = track_param(e, "track.playlist_promote", p, "track")?.ok_or_else(|| bad("track.playlist_promote", "`track` required"))?;
                let idx =
                    p.get("playlist").and_then(Value::as_u64).ok_or_else(|| bad("track.playlist_promote", "`playlist` index required"))? as usize;
                let r = range_param(e, "track.playlist_promote", p)?;
                if r.is_empty() {
                    return Err(bad("track.playlist_promote", "give `start` and `end`"));
                }
                let s = e.session_mut();
                let (src, active) = {
                    let tr = s.track(t).ok_or_else(|| bad("track.playlist_promote", "no track"))?;
                    (tr.playlists.get(idx).cloned().ok_or_else(|| bad("track.playlist_promote", "no such playlist"))?, tr.active_playlist)
                };
                if idx == active {
                    return Err(bad("track.playlist_promote", "that playlist is already active"));
                }
                // Copy the clips of the source playlist inside the range (via a temporary active switch).
                if let Some(tr) = s.track_mut(t) {
                    tr.active_playlist = idx;
                }
                let clips = crate::edit::copy_range(s, t, r);
                if let Some(tr) = s.track_mut(t) {
                    tr.active_playlist = active;
                }
                let _ = src;
                let ids = crate::edit::paste(s, t, r.start, &clips, r.len(), false);
                Ok(json!({"clips": ids}))
            }
        ),
        cmd!("track.input", "Track Input", [], None, "{tracks?, input: none|bus name|hardware name}", has_selection, |e, p| route(e, p, true)),
        cmd!("track.output", "Track Output", [], None, "{tracks?, output: main|none|bus name}", has_selection, |e, p| route(e, p, false)),
        cmd!("track.timebase", "Track Timebase", [], None, "{tracks?, ticks: bool}", has_selection, |e, p| flag(
            e,
            p,
            "track.timebase",
            |t, v| t.ticks_timebase = v,
            |t| t.ticks_timebase
        )),
        cmd!(
            "track.elastic",
            "Elastic Audio",
            [],
            None,
            "{tracks?, algorithm?: polyphonic|rhythmic|monophonic|varispeed|x-form|none}",
            has_selection,
            |e, p| {
                let tracks = tracks_required(e, "track.elastic", p)?;
                let alg = str_param(p, "algorithm").filter(|a| *a != "none").map(str::to_string);
                let s = e.session_mut();
                for t in tracks {
                    if let Some(tr) = s.track_mut(t) {
                        tr.elastic = alg.clone();
                    }
                }
                Ok(json!({"algorithm": alg}))
            }
        ),
    ]
}

fn flag(
    e: &mut Engine,
    p: &Value,
    cmd: &str,
    set: fn(&mut soundcraft_model::Track, bool),
    get: fn(&soundcraft_model::Track) -> bool,
) -> Result<Value> {
    let tracks = tracks_required(e, cmd, p)?;
    let explicit = p.get("inactive").or_else(|| p.get("hidden")).or_else(|| p.get("value")).or_else(|| p.get("ticks")).and_then(Value::as_bool);
    let s = e.session_mut();
    let target = explicit.unwrap_or_else(|| !tracks.iter().all(|t| s.track(*t).is_some_and(get)));
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            set(tr, target);
        }
    }
    Ok(json!({"value": target, "tracks": tracks}))
}

#[cfg(test)]
mod tests {
    use soundcraft_model::Route;

    use super::*;

    #[test]
    fn comp_from_alternate_playlist() {
        let mut e = crate::demo::demo_engine();
        let kick = e.session().track_by_name("Kick").map(|t| t.id.0).unwrap();
        // Duplicate the playlist, then clear a range on the new active playlist and promote it back from the first.
        e.execute("track.playlist_duplicate", &json!({"track": kick})).unwrap();
        e.execute("edit.clear", &json!({"tracks": [kick], "start": {"seconds": 10.0}, "end": {"seconds": 12.0}})).unwrap();
        let gap = |e: &Engine| {
            e.session().track(soundcraft_model::TrackId(kick)).unwrap().playlist().unwrap().clip_at(e.session().sample_rate.samples(11.0)).is_none()
        };
        assert!(gap(&e));
        e.execute("track.playlist_promote", &json!({"track": kick, "playlist": 0, "start": {"seconds": 10.0}, "end": {"seconds": 12.0}})).unwrap();
        assert!(!gap(&e));
        assert!(e.execute("track.playlist_promote", &json!({"track": kick, "playlist": 1, "start": 0, "end": 10})).is_err());
    }

    #[test]
    fn folder_routes_members_through_its_bus() {
        let mut e = crate::demo::demo_engine();
        let r = e.execute("track.move_to_new_folder", &json!({"tracks": ["Kick", "Snare"], "name": "Drums"})).unwrap();
        assert!(r["bus"].is_u64());
        let s = e.session();
        let kick = s.track_by_name("Kick").unwrap();
        assert!(matches!(kick.mixer.output, Route::Bus(_)));
        assert!(s.tracks.iter().any(|t| t.kind == TrackKind::Folder && t.mixer.input == kick.mixer.output));
    }
}
