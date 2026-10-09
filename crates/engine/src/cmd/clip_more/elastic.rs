//! Elastic clip tempo conforming and pitch restoration.

use super::super::more_util::render_stretched;
use super::super::more_util::replace_clip;
use super::super::*;
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_time::Samples;

pub(super) fn conform(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "clip.conform_to_tempo";
    let ids = clip_ids_param(e, p);
    let given = p.get("source_bpm").and_then(Value::as_f64).filter(|b| b.is_finite());
    if let Some(b) = given
        && !(5.0..=1000.0).contains(&b)
    {
        return Err(bad(id, "source_bpm must be 5..1000"));
    }
    let s = e.session_mut();
    let mut out = Vec::new();
    for cid in ids {
        let Some((_, c)) = s.find_clip(cid).map(|(t, c)| (t, c.clone())) else { continue };
        if !c.is_audio() || c.edit_locked {
            continue;
        }
        let bpm_key = format!("clip.bpm.{}", cid.0);
        let len_key = format!("clip.len0.{}", cid.0);
        let stretch = if c.stretch.is_finite() && c.stretch > 0.0 { c.stretch } else { 1.0 };
        let now = s.tempo.tempo_at_tick(s.tempo.samples_to_ticks(c.start, s.sample_rate));
        let src_bpm = given.unwrap_or_else(|| s.edit.value(&bpm_key, now));
        let len0 = s.edit.value(&len_key, c.length as f64 / stretch).max(1.0);
        s.edit.values.insert(bpm_key, src_bpm);
        s.edit.values.insert(len_key, len0);
        let ratio = (src_bpm / now).clamp(0.05, 20.0);
        let new_len = ((len0 * ratio).round() as Samples).max(1);
        if let Some(cl) = s.find_clip_mut(cid) {
            cl.stretch = ratio;
            cl.length = new_len;
            cl.fade_in.len = cl.fade_in.len.min(new_len);
            cl.fade_out.len = cl.fade_out.len.min(new_len);
        }
        out.push(json!({"clip": cid, "source_bpm": src_bpm, "bpm": now, "stretch": ratio}));
    }
    Ok(json!({"clips": out}))
}

pub(super) fn remove_pitch_shift(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "clip.remove_pitch_shift";
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for cid in ids {
        let Some((t, c)) = s.find_clip(cid).map(|(t, c)| (t, c.clone())) else { continue };
        s.edit.values.remove(&format!("clip.pitch.{}", cid.0));
        if !c.is_audio() || (c.stretch - 1.0).abs() < 1e-9 || !c.stretch.is_finite() {
            continue;
        }
        let Some(src) = render_stretched(s, id, &c, c.length)? else { continue };
        let mut nc = c.clone();
        nc.content = ClipContent::Audio { source: src, offset: 0 };
        nc.stretch = 1.0;
        replace_clip(s, t, cid, nc);
        n += 1;
    }
    Ok(json!({"clips": n}))
}
