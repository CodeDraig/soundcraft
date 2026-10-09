//! Screenshot request, viewport capture, timeout and PNG delivery lifecycle.

use super::{ControlRequest, err, ok};
use crate::SoundApp;
use serde_json::Value;
use serde_json::json;
use std::sync::mpsc::Sender;

pub struct PendingShot {
    token: u64,
    reply: Sender<Value>,
    path: Option<String>,
    frames: u32,
    sent: bool,
    deadline_frames: u32,
}

/// Request and collect viewport screenshots for pending `ui.screenshot` calls.
pub fn collect_screenshots(app: &mut SoundApp, ctx: &egui::Context) {
    if app.pending_shots.is_empty() {
        return;
    }
    let images: Vec<(u64, std::sync::Arc<egui::ColorImage>)> = ctx.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Screenshot { user_data, image, .. } => {
                    user_data.data.as_ref().and_then(|d| d.downcast_ref::<u64>()).map(|t| (*t, image.clone()))
                }
                _ => None,
            })
            .collect()
    });
    let mut keep = Vec::new();
    for mut s in std::mem::take(&mut app.pending_shots) {
        if let Some((_, img)) = images.iter().find(|(t, _)| *t == s.token) {
            let _ = s.reply.send(encode(img, s.path.as_deref()));
            continue;
        }
        s.frames += 1;
        if !s.sent && s.frames >= 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(s.token)));
            s.sent = true;
        }
        if s.frames > s.deadline_frames {
            let _ = s.reply.send(err("no frame was presented (is the window visible?)"));
            continue;
        }
        keep.push(s);
    }
    app.pending_shots = keep;
    ctx.request_repaint();
}

pub fn png_bytes(img: &egui::ColorImage) -> Result<Vec<u8>, String> {
    let [w, h] = img.size;
    let mut raw = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        raw.extend_from_slice(&c.to_array());
    }
    let buf = image::RgbaImage::from_raw(u32::try_from(w).map_err(|e| e.to_string())?, u32::try_from(h).map_err(|e| e.to_string())?, raw)
        .ok_or("bad image size")?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

fn encode(img: &egui::ColorImage, path: Option<&str>) -> Value {
    let bytes = match png_bytes(img) {
        Ok(b) => b,
        Err(e) => return err(e),
    };
    match path {
        Some(p) => match std::fs::write(p, &bytes) {
            Ok(()) => ok(json!({"path": p, "width": img.size[0], "height": img.size[1]})),
            Err(e) => err(format!("{p}: {e}")),
        },
        None => ok(json!({"png_base64": soundcraft_model::b64::encode(&bytes), "width": img.size[0], "height": img.size[1]})),
    }
}

pub(super) fn request_screenshot(app: &mut SoundApp, ctx: &egui::Context, req: &ControlRequest) {
    let p = &req.params;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let token = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    app.pending_shots.push(PendingShot {
        token,
        reply: req.reply.clone(),
        path: p.get("path").and_then(Value::as_str).map(str::to_string),
        frames: 0,
        sent: false,
        deadline_frames: 600,
    });
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_encodes_a_decodable_png() {
        let img = egui::ColorImage::filled([2, 3], egui::Color32::RED);
        let reply = encode(&img, None);
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["result"]["width"], 2);
        assert_eq!(reply["result"]["height"], 3);
        let bytes = soundcraft_model::b64::decode(reply["result"]["png_base64"].as_str().unwrap()).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 3));
        assert!(decoded.pixels().all(|pixel| pixel.0 == [255, 0, 0, 255]));
    }
}
