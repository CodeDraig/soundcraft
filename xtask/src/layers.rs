//! Workspace dependency-layer validation.

use super::root;
use std::process::Command;

/// Layer table: crate name (minus `soundcraft-`) → layer. Apps and xtask are exempt.
const LAYERS: &[(&str, u8)] = &[
    ("time", 0),
    ("audio-io", 0),
    ("midi", 0),
    ("video", 0),
    ("dsp", 1),
    ("clap-host", 1),
    ("vst3-host", 1),
    ("au-host", 1),
    ("model", 2),
    ("mix", 3),
    ("engine", 4),
    ("playback", 4),
    ("automation", 5),
    ("ui-egui", 6),
];

/// Standalone format crates: no workspace dependencies at all (publishable on their own).
const STANDALONE: &[&str] = &["time", "audio-io", "midi", "video", "dsp"];

const UI_CRATES: &[&str] = &["egui", "eframe", "winit", "egui_kittest", "rfd", "egui-wgpu", "egui_glow"];

const EXEMPT: &[&str] = &["soundcraft", "soundcraft-cli", "soundcraft-web", "xtask"];

pub(super) fn layers() -> Result<(), String> {
    let out =
        Command::new("cargo").args(["metadata", "--format-version", "1", "--no-deps"]).current_dir(root()).output().map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut problems = Vec::new();
    for p in v["packages"].as_array().cloned().unwrap_or_default() {
        let name = p["name"].as_str().unwrap_or("").to_string();
        if EXEMPT.contains(&name.as_str()) {
            continue;
        }
        let short = name.trim_start_matches("soundcraft-");
        let Some(&(_, layer)) = LAYERS.iter().find(|(n, _)| *n == short) else {
            problems.push(format!("{name}: not registered in xtask LAYERS"));
            continue;
        };
        for d in p["dependencies"].as_array().cloned().unwrap_or_default() {
            let dn = d["name"].as_str().unwrap_or("");
            let kind = d["kind"].as_str().unwrap_or("normal");
            if UI_CRATES.contains(&dn) && layer < 6 && kind != "dev" {
                problems.push(format!("{name} (L{layer}) depends on UI crate {dn}"));
            }
            if let Some(ds) = dn.strip_prefix("soundcraft-") {
                if STANDALONE.contains(&short) && kind != "dev" {
                    problems.push(format!("{name} is standalone but depends on {dn}"));
                }
                if let Some(&(_, dl)) = LAYERS.iter().find(|(n, _)| *n == ds)
                    && dl > layer
                    && kind != "dev"
                {
                    problems.push(format!("{name} (L{layer}) depends upward on {dn} (L{dl})"));
                }
            }
        }
    }
    if problems.is_empty() {
        println!("layers: ok");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}
