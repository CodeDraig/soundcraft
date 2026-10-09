//! Desktop UI preferences and their platform-specific location.

use soundcraft_ui_egui::UiState;

pub(super) fn prefs_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support/SoundCraft"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("SoundCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
            .map(|c| c.join("soundcraft"))
    };
    base.map(|b| b.join("ui.json"))
}

pub(super) fn load_prefs() -> Option<UiState> {
    if std::env::var_os("SOUNDCRAFT_NO_PREFS").is_some() {
        return None;
    }
    let bytes = std::fs::read(prefs_path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(super) fn save_prefs(ui: &UiState) {
    if std::env::var_os("SOUNDCRAFT_NO_PREFS").is_some() {
        return;
    }
    if let Some(p) = prefs_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut ui = ui.clone();
        ui.plugin_windows.clear();
        ui.status.clear();
        if let Ok(bytes) = serde_json::to_vec_pretty(&ui) {
            let _ = std::fs::write(&p, bytes);
        }
    }
}
