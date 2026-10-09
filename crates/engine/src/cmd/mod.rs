//! The command registry. One module per menu area; each exposes `specs()`.

use crate::{Engine, EngineError, Result};
use serde_json::Value;
use std::sync::OnceLock;

mod audio_midi;
mod audiosuite;
mod clap;
mod clip;
mod clip_more;
mod edit;
mod edit_more;
mod event;
mod event_more;
mod file;
mod midi;
mod mix;
mod more_util;
mod options;
mod plugin_state;
mod query;
mod setup_more;
mod surround;
mod track;
mod track_more;
mod transport;
mod video;
mod view;
mod view_more;
mod vst3;

mod enablement;
pub use enablement::{always, can_redo, can_undo, has_clipboard, has_range, has_selection, has_tracks};
mod params;
pub use params::{
    MAX_POSITION, bad, bool_or, clip_ids_param, f32_or, f64_or, i64_or, position_param, range_param, str_param, track_param, tracks_param,
    tracks_required,
};

pub type Run = fn(&mut Engine, &Value) -> Result<Value>;
/// `Err(reason)` = disabled.
pub type Enabled = fn(&Engine) -> std::result::Result<(), String>;

/// A registered command.
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Menu path, e.g. `["Edit", "Trim"]`; empty = not in menus.
    pub menu: &'static [&'static str],
    pub shortcut: Option<&'static str>,
    /// Human/agent parameter documentation.
    pub params: &'static str,
    pub enabled: Enabled,
    pub run: Run,
    pub journal: bool,
    pub undoable: bool,
}

/// Serializable description of a command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandInfo {
    pub id: String,
    pub label: String,
    pub menu: Vec<String>,
    pub shortcut: Option<String>,
    pub params: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

impl CommandSpec {
    pub fn info(&self, e: &Engine) -> CommandInfo {
        let en = (self.enabled)(e);
        CommandInfo {
            id: self.id.into(),
            label: self.label.into(),
            menu: self.menu.iter().map(|s| s.to_string()).collect(),
            shortcut: self.shortcut.map(str::to_string),
            params: self.params.into(),
            enabled: en.is_ok(),
            disabled_reason: en.err(),
        }
    }
}

/// `cmd!(id, label, [menu…], shortcut, params, enabled, run)`; `query`/`noundo` variants.
#[macro_export]
macro_rules! cmd {
    (query $id:literal, $label:literal, [$($m:literal),*], $sc:expr, $params:literal, $en:expr, $run:expr) => {
        $crate::cmd::CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: $sc, params: $params, enabled: $en, run: $run, journal: false, undoable: false }
    };
    (noundo $id:literal, $label:literal, [$($m:literal),*], $sc:expr, $params:literal, $en:expr, $run:expr) => {
        $crate::cmd::CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: $sc, params: $params, enabled: $en, run: $run, journal: true, undoable: false }
    };
    ($id:literal, $label:literal, [$($m:literal),*], $sc:expr, $params:literal, $en:expr, $run:expr) => {
        $crate::cmd::CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: $sc, params: $params, enabled: $en, run: $run, journal: true, undoable: true }
    };
}

pub fn command_specs() -> &'static [CommandSpec] {
    static SPECS: OnceLock<Vec<CommandSpec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        let mut v = Vec::new();
        v.extend(file::specs());
        v.extend(edit::specs());
        v.extend(edit_more::specs());
        v.extend(view::specs());
        v.extend(track::specs());
        v.extend(clip::specs());
        v.extend(event::specs());
        v.extend(mix::specs());
        v.extend(options::specs());
        v.extend(transport::specs());
        v.extend(query::specs());
        v.extend(audiosuite::specs());
        v.extend(audio_midi::specs());
        v.extend(midi::specs());
        v.extend(view_more::specs());
        v.extend(track_more::specs());
        v.extend(clip_more::specs());
        v.extend(event_more::specs());
        v.extend(setup_more::specs());
        v.extend(clap::specs());
        v.extend(vst3::specs());
        v.extend(surround::specs());
        v.extend(plugin_state::specs());
        v.extend(video::specs());
        v
    })
}

pub fn find_command(id: &str) -> Option<&'static CommandSpec> {
    command_specs().iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_documented() {
        let mut seen = std::collections::HashSet::new();
        for c in command_specs() {
            assert!(seen.insert(c.id), "duplicate command id {}", c.id);
            assert!(!c.label.is_empty());
            assert!(c.id.contains('.'), "ids are namespaced: {}", c.id);
        }
        assert!(command_specs().len() > 150, "only {} commands", command_specs().len());
    }

    #[test]
    fn every_command_survives_empty_params() {
        // No command may panic with `{}` params on an empty or demo session.
        for demo in [false, true] {
            for c in command_specs() {
                if matches!(c.id, "app.quit") {
                    continue;
                }
                let mut e = if demo { crate::demo::demo_engine() } else { Engine::default() };
                let r = e.execute(c.id, &serde_json::json!({}));
                if let Err(EngineError::Internal(id, msg)) = r {
                    panic!("{id} panicked: {msg}");
                }
            }
        }
    }
}
