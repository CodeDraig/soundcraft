//! Web-target build validation.

use super::{cargo, root};

/// Crates that must compile for the web.
const WASM: &[&str] = &[
    "soundcraft-time",
    "soundcraft-audio-io",
    "soundcraft-midi",
    "soundcraft-video",
    "soundcraft-dsp",
    "soundcraft-model",
    "soundcraft-mix",
    "soundcraft-engine",
    "soundcraft-playback",
    "soundcraft-automation",
    "soundcraft-ui-egui",
];

pub(super) fn wasm() -> Result<(), String> {
    let mut args = vec!["check", "--target", "wasm32-unknown-unknown"];
    for c in WASM {
        args.push("-p");
        args.push(c);
    }
    if root().join("apps/soundcraft-web/Cargo.toml").exists() {
        args.push("-p");
        args.push("soundcraft-web");
    }
    cargo(&args)
}
