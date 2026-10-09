//! Menu catalog tree, drawing and checked state.

use super::{MENU_WINDOWS, invoke_menu, mix_section_id, ruler_id, ui_aliases};
use crate::MainWindow;
use crate::SoundApp;
use serde_json::json;
use soundcraft_engine::catalog;

struct MenuNode {
    label: String,
    path: String,
    children: Vec<MenuNode>,
}

fn tree() -> Vec<MenuNode> {
    let mut roots: Vec<MenuNode> = Vec::new();
    for line in catalog::catalog() {
        let parts: Vec<&str> = line.split(" > ").collect();
        let mut level = &mut roots;
        let mut path = String::new();
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                path.push_str(" > ");
            }
            path.push_str(p);
            let idx = match level.iter().position(|n| n.label == *p) {
                Some(i) => i,
                None => {
                    level.push(MenuNode { label: p.to_string(), path: path.clone(), children: Vec::new() });
                    level.len() - 1
                }
            };
            let Some(node) = level.get_mut(idx) else { break };
            level = &mut node.children;
        }
    }
    roots
}

pub fn menu_bar(app: &mut SoundApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::DARK;
    egui::Panel::top("menu_bar")
        .exact_size(24.0)
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(22, 22, 23)).inner_margin(egui::Margin::symmetric(8, 2)))
        .show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("SoundCraft", |ui| {
                    if ui.button("About SoundCraft").clicked() {
                        app.ui.show_about = true;
                    }
                    if ui.button("Session Info").clicked() {
                        app.ui.show_session_info = true;
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        let _ = app.run("app.quit", json!({}));
                    }
                });
                let extra = ui_aliases();
                for root in tree() {
                    ui.menu_button(&root.label, |ui| {
                        for c in &root.children {
                            menu_node(app, ui, c, &extra);
                        }
                        if root.label == "AudioSuite" {
                            ui.separator();
                            ui.label(egui::RichText::new("Processes selected clips offline").small().color(t.text_dim));
                        }
                    });
                }
                let title = format!(
                    "{}{}  —  {}",
                    app.engine.session().name,
                    if app.engine.is_dirty() { " *" } else { "" },
                    if app.ui.window == MainWindow::Edit { "Edit" } else { "Mix" }
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(title).color(t.text_dim));
                });
            });
        });
}

fn menu_node(app: &mut SoundApp, ui: &mut egui::Ui, n: &MenuNode, extra: &[(&str, &str)]) {
    if !n.children.is_empty() {
        ui.menu_button(&n.label, |ui| {
            for c in &n.children {
                menu_node(app, ui, c, extra);
            }
        });
        return;
    }
    let id = catalog::implemented_by(&n.path, extra).or_else(|| MENU_WINDOWS.iter().find(|(p, _)| *p == n.path).map(|(_, w)| w.to_string()));
    let enabled = match id.as_deref() {
        Some(i) if i.starts_with("window.") || i.starts_with("view.mix_section") || i == "audiosuite.process" => true,
        Some(i) => soundcraft_engine::find_command(i).is_some_and(|c| (c.enabled)(&app.engine).is_ok()),
        None => false,
    };
    let checked = checked_state(app, &n.path, id.as_deref());
    let label = if checked { format!("✔ {}", n.label) } else { n.label.clone() };
    let shortcut = id.as_deref().and_then(soundcraft_engine::find_command).and_then(|c| c.shortcut).unwrap_or("");
    let btn = egui::Button::new(label).shortcut_text(shortcut);
    if ui.add_enabled(enabled, btn).clicked()
        && let Some(id) = id
    {
        invoke_menu(app, &id, &n.path);
        ui.close();
    }
}

fn checked_state(app: &SoundApp, path: &str, id: Option<&str>) -> bool {
    let e = &app.engine.session().edit;
    match id {
        Some("options.loop_playback") => e.loop_playback,
        Some("options.loop_record") => e.loop_record,
        Some("options.quickpunch") => e.quickpunch,
        Some("options.pre_post_roll") => e.pre_post_roll,
        Some("options.link_timeline_edit") => e.link_timeline_edit,
        Some("options.link_track_edit") => e.link_track_edit,
        Some("options.insertion_follows_playback") => e.insertion_follows_playback,
        Some("options.tab_to_transient") => e.tab_to_transient,
        Some("options.automation_follows_edit") => e.automation_follows_edit,
        Some("options.markers_follow_edit") => e.markers_follow_edit,
        Some("options.click") => e.click,
        Some("options.delay_compensation") => e.delay_compensation,
        Some("options.pre_fader_metering") => e.pre_fader_metering,
        Some("view.ruler") => path.rsplit(" > ").next().map(ruler_id).is_some_and(|r| e.rulers.iter().any(|x| x == r)),
        Some("view.main_counter") => path.rsplit(" > ").next().map(ruler_id).is_some_and(|r| e.main_counter.id() == r),
        Some("window.mix") => app.ui.window == MainWindow::Mix,
        Some("window.edit") => app.ui.window == MainWindow::Edit,
        Some("window.track_list") => app.ui.show_tracks_list,
        Some("window.clip_list_view") | Some("window.clip_list") => app.ui.show_clip_list,
        Some("window.narrow_mix") => app.ui.narrow_mix,
        Some("window.renderer") => app.extra.show_renderer,
        Some("window.ui_customization") => app.extra.show_ui_customization,
        Some("window.config_auto_update") => app.extra.auto_update_config,
        Some("view.mix_section") => path.rsplit(" > ").next().map(mix_section_id).is_some_and(|s| app.ui.mix_views.iter().any(|v| v == s)),
        _ => false,
    }
}
