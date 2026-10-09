//! Periodic autosaving and recovery-file discovery.

use super::SoundApp;

impl SoundApp {
    /// Every two minutes, copy a modified session to the autosave folder (not while recording).
    pub(super) fn autosave(&mut self, now: f64) {
        let Some(dir) = self.autosave_dir.clone() else { return };
        if self.last_autosave == 0.0 {
            self.last_autosave = now;
        }
        if now - self.last_autosave < 120.0 || !self.engine.is_dirty() || self.engine.transport.recording {
            return;
        }
        self.last_autosave = now;
        self.capture_plugin_states();
        let name = soundcraft_engine::io::sanitize_name(&self.engine.session().name);
        let path = dir.join(&name).join(format!("{name}.scraft"));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match soundcraft_engine::io::save_copy(&self.engine, &path.to_string_lossy()) {
            Ok(_) => log::info!("autosaved to {}", path.display()),
            Err(e) => log::warn!("autosave failed: {e}"),
        }
    }

    /// The most recent autosave, if any.
    pub fn latest_autosave(&self) -> Option<std::path::PathBuf> {
        let dir = self.autosave_dir.as_ref()?;
        let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for d in std::fs::read_dir(dir).ok()?.flatten() {
            for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
                let p = f.path();
                if p.extension().and_then(|x| x.to_str()) == Some("scraft")
                    && let Ok(m) = f.metadata().and_then(|m| m.modified())
                    && best.as_ref().is_none_or(|(t, _)| m > *t)
                {
                    best = Some((m, p));
                }
            }
        }
        best.map(|(_, p)| p)
    }
}
