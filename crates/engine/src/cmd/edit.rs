//! Edit menu commands.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{EditMode, Session};

mod clipboard;
use clipboard::{clear, clear_automation, cut_copy, paste, reset_clip_gain};
mod selection;
use selection::{extend_tracks, restore_selection, scale_selection, select, select_all, shift_selection};
mod placement;
use placement::{duplicate, insert_silence, move_clips, nudge, repeat, repeat_to_fill, shift};
mod boundaries;
use boundaries::{heal, separate, separate_at_transients, separate_on_grid, trim_edge, trim_to_file, trim_to_fill, trim_to_selection};
mod fades;
use fades::{fade_to, fades_create, fades_delete};
mod rendering;
use rendering::{consolidate, copy_to_new_playlist, mute_clips, strip_silence};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "edit.undo", "Undo", ["Edit"], Some("Cmd+Z"), "{}", can_undo, |e, _| Ok(json!({"undone": e.undo()}))),
        cmd!(noundo "edit.redo", "Redo", ["Edit"], Some("Cmd+Shift+Z"), "{}", can_redo, |e, _| Ok(json!({"redone": e.redo()}))),
        cmd!("edit.restore_last_selection", "Restore Last Selection", ["Edit"], Some("Cmd+Alt+Z"), "{}", always, restore_selection),
        cmd!("edit.cut", "Cut", ["Edit"], Some("Cmd+X"), "{tracks?, start?, end?}", has_range, |e, p| cut_copy(e, p, true, "edit.cut")),
        cmd!(noundo "edit.copy", "Copy", ["Edit"], Some("Cmd+C"), "{tracks?, start?, end?}", has_range, |e, p| cut_copy(e, p, false, "edit.copy")),
        cmd!("edit.paste", "Paste", ["Edit"], Some("Cmd+V"), "{tracks?, at?}", has_clipboard, paste),
        cmd!("edit.clear", "Clear", ["Edit"], Some("Cmd+B"), "{tracks?, start?, end?}", has_range, clear),
        cmd!("edit.cut_clip_gain", "Clip Gain", ["Edit", "Cut Special"], Some("Cmd+Ctrl+X"), "{}", has_range, |e, p| reset_clip_gain(
            e,
            p,
            "edit.cut_clip_gain"
        )),
        cmd!("edit.clear_clip_gain", "Clip Gain", ["Edit", "Clear Special"], Some("Cmd+Ctrl+B"), "{}", has_range, |e, p| reset_clip_gain(
            e,
            p,
            "edit.clear_clip_gain"
        )),
        cmd!("edit.cut_all_automation", "All Automation", ["Edit", "Cut Special"], None, "{}", has_range, |e, p| clear_automation(e, p, None)),
        cmd!("edit.clear_all_automation", "All Automation", ["Edit", "Clear Special"], None, "{}", has_range, |e, p| clear_automation(e, p, None)),
        cmd!("edit.clear_pan_automation", "Pan Automation", ["Edit", "Clear Special"], None, "{}", has_range, |e, p| clear_automation(
            e,
            p,
            Some("pan")
        )),
        cmd!("edit.clear_plugin_automation", "Plugin Automation", ["Edit", "Clear Special"], None, "{}", has_range, |e, p| clear_automation(
            e,
            p,
            Some("plugin")
        )),
        cmd!(
            "edit.paste_repeat_to_fill",
            "Repeat to Fill Selection",
            ["Edit", "Paste Special"],
            Some("Cmd+Alt+V"),
            "{}",
            has_clipboard,
            repeat_to_fill
        ),
        cmd!(noundo "edit.select_all", "Select All", ["Edit"], Some("Cmd+A"), "{}", always, select_all),
        cmd!(noundo "edit.select", "Set Edit Selection", [], None, "{tracks?: [id|name], start?, end?, clips?: [id], exact?: bool (ignore edit groups)}", always, select),
        cmd!(noundo "edit.select_none", "Deselect All", [], None, "{}", always, |e, _| {
            let s = e.session_mut();
            s.edit.selected_tracks.clear();
            s.edit.selected_clips.clear();
            s.edit.selection.end = s.edit.selection.start;
            Ok(json!({}))
        }),
        cmd!(noundo "edit.selection_to_timeline", "Change Timeline to Match Edit", ["Edit", "Selection"], Some("Alt+Shift+5"), "{}", always, |e, _| {
            let s = e.session_mut();
            s.edit.timeline_selection = s.edit.selection;
            Ok(json!({}))
        }),
        cmd!(noundo "edit.timeline_to_selection", "Change Edit to Match Timeline", ["Edit", "Selection"], Some("Alt+Shift+6"), "{}", always, |e, _| {
            let s = e.session_mut();
            s.edit.selection = s.edit.timeline_selection;
            Ok(json!({}))
        }),
        cmd!(noundo "edit.move_selection_left", "Move Edit Left", ["Edit", "Selection"], Some("Cmd+Alt+L"), "{}", always, |e, _| shift_selection(e, -1)),
        cmd!(noundo "edit.move_selection_right", "Move Edit Right", ["Edit", "Selection"], Some("Cmd+Alt+'"), "{}", always, |e, _| shift_selection(e, 1)),
        cmd!(noundo "edit.halve_selection", "Halve Edit", ["Edit", "Selection"], None, "{}", always, |e, _| scale_selection(e, 0.5)),
        cmd!(noundo "edit.double_selection", "Double Edit", ["Edit", "Selection"], None, "{}", always, |e, _| scale_selection(e, 2.0)),
        cmd!(noundo "edit.extend_selection_up", "Extend Edit Up", ["Edit", "Selection"], None, "{}", has_tracks, |e, _| extend_tracks(e, -1)),
        cmd!(noundo "edit.extend_selection_down", "Extend Edit Down", ["Edit", "Selection"], None, "{}", has_tracks, |e, _| extend_tracks(e, 1)),
        cmd!(noundo "edit.remove_selection_top", "Remove Edit from Top", ["Edit", "Selection"], None, "{}", has_tracks, |e, _| {
            let s = e.session_mut();
            if !s.edit.selected_tracks.is_empty() {
                s.edit.selected_tracks.remove(0);
            }
            Ok(json!({}))
        }),
        cmd!(noundo "edit.remove_selection_bottom", "Remove Edit from Bottom", ["Edit", "Selection"], None, "{}", has_tracks, |e, _| {
            e.session_mut().edit.selected_tracks.pop();
            Ok(json!({}))
        }),
        cmd!("edit.duplicate", "Duplicate", ["Edit"], Some("Cmd+D"), "{tracks?, start?, end?}", has_range, duplicate),
        cmd!("edit.repeat", "Repeat...", ["Edit"], Some("Alt+R"), "{count: n}", has_range, repeat),
        cmd!("edit.shift", "Shift...", ["Edit"], Some("Alt+H"), "{by: samples|{seconds}, later?: bool}", has_range, shift),
        cmd!("edit.insert_silence", "Insert Silence", ["Edit"], Some("Cmd+Shift+E"), "{tracks?, start?, end?}", has_range, insert_silence),
        cmd!("edit.separate", "At Selection", ["Edit", "Separate"], Some("Cmd+E"), "{tracks?, at?|start/end}", has_selection, separate),
        cmd!("edit.separate_on_grid", "On Grid", ["Edit", "Separate"], None, "{tracks?, start?, end?}", has_range, separate_on_grid),
        cmd!("edit.separate_at_transients", "At Transients", ["Edit", "Separate"], None, "{sensitivity?: 0..1}", has_range, separate_at_transients),
        cmd!("edit.heal", "Heal Separation", ["Edit"], Some("Cmd+H"), "{tracks?, start?, end?}", has_selection, heal),
        cmd!("edit.trim_to_selection", "To Selection", ["Edit", "Trim"], Some("Cmd+T"), "{tracks?, start?, end?}", has_range, trim_to_selection),
        cmd!("edit.trim_start_to_insertion", "Start to Insertion", ["Edit", "Trim"], Some("Alt+Shift+7"), "{tracks?, at?}", has_selection, |e, p| {
            trim_edge(e, p, true)
        }),
        cmd!("edit.trim_end_to_insertion", "End to Insertion", ["Edit", "Trim"], Some("Alt+Shift+8"), "{tracks?, at?}", has_selection, |e, p| {
            trim_edge(e, p, false)
        }),
        cmd!("edit.trim_to_file_boundaries", "To File Boundaries", ["Edit", "Trim"], None, "{clips?}", has_selection, trim_to_file),
        cmd!("edit.trim_to_fill_selection", "To Fill Selection", ["Edit", "Trim"], None, "{}", has_range, trim_to_fill),
        cmd!("edit.mute_clips", "Mute", ["Edit"], Some("Cmd+M"), "{clips?}", has_selection, mute_clips),
        cmd!("edit.copy_to_new_playlist", "New Playlist", ["Edit", "Copy Selection to..."], None, "{}", has_range, copy_to_new_playlist),
        cmd!(
            "edit.strip_silence",
            "Strip Silence",
            ["Edit"],
            Some("Cmd+U"),
            "{threshold_db?: -48, min_length_ms?: 50, pad_before_ms?: 5, pad_after_ms?: 20}",
            has_range,
            strip_silence
        ),
        cmd!(
            "edit.fades_create",
            "Create...",
            ["Edit", "Fades"],
            Some("Cmd+F"),
            "{shape?: linear|equal power|s-curve|exponential|logarithmic, tracks?, start?, end?}",
            has_range,
            fades_create
        ),
        cmd!("edit.fades_delete", "Delete", ["Edit", "Fades"], None, "{tracks?, start?, end?}", has_selection, fades_delete),
        cmd!("edit.fade_to_start", "Fade to Start", ["Edit", "Fades"], Some("Cmd+Ctrl+D"), "{}", has_selection, |e, p| fade_to(e, p, true)),
        cmd!("edit.fade_to_end", "Fade to End", ["Edit", "Fades"], Some("Cmd+Ctrl+G"), "{}", has_selection, |e, p| fade_to(e, p, false)),
        cmd!("edit.nudge", "Nudge", [], Some("Plus/Minus"), "{direction: 1|-1, count?: n}", has_selection, nudge),
        cmd!(noundo "edit.mode", "Edit Mode", [], Some("F1-F4"), "{mode: shuffle|slip|spot|grid|relative_grid}", always, |e, p| {
            let m = str_param(p, "mode").and_then(EditMode::from_id).ok_or_else(|| bad("edit.mode", "mode must be shuffle|slip|spot|grid|relative_grid"))?;
            e.session_mut().edit.edit_mode = m;
            Ok(json!({"mode": m}))
        }),
        cmd!(noundo "edit.tool", "Edit Tool", [], Some("F5-F10"), "{tool: zoom|trim|selector|grabber|scrubber|pencil|smart}", always, |e, p| {
            let t = str_param(p, "tool").and_then(soundcraft_model::Tool::from_id).ok_or_else(|| bad("edit.tool", "unknown tool"))?;
            e.session_mut().edit.tool = t;
            Ok(json!({"tool": t}))
        }),
        cmd!(
            "edit.move_clips",
            "Move Clips",
            [],
            None,
            "{clips?: [id], by?: samples, to?: position (first clip start), track?: target}",
            has_selection,
            move_clips
        ),
        cmd!("edit.consolidate", "Consolidate", ["Edit"], Some("Alt+Shift+3"), "{tracks?, start?, end?}", has_range, consolidate),
    ]
}

fn shuffle(s: &Session) -> bool {
    s.edit.edit_mode == EditMode::Shuffle
}
