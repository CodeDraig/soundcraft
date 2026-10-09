//! Time compression and expansion to the edit selection.

use super::super::more_util::render_stretched;
use super::super::more_util::replace_clip;
use super::super::*;
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_time::Range;
use soundcraft_time::Samples;

pub(super) fn tce(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "edit.tce_to_timeline";
    let target = {
        let st = position_param(e, id, p, "start")?;
        let en = position_param(e, id, p, "end")?;
        match (st, en) {
            (Some(a), Some(b)) => Range::new(a, b),
            _ => e.session().edit.timeline_selection,
        }
    };
    if target.is_empty() || target.start < 0 {
        return Err(bad(id, "make a timeline selection first"));
    }
    let ids = clip_ids_param(e, p);
    let Some((t, c)) = ids.iter().find_map(|cid| e.session().find_clip(*cid).filter(|(_, c)| c.is_audio()).map(|(t, c)| (t, c.clone()))) else {
        return Err(bad(id, "select an audio clip"));
    };
    if c.time_locked || c.edit_locked {
        return Err(bad(id, "the clip is locked"));
    }
    let s = e.session_mut();
    let Some(src) = render_stretched(s, id, &c, target.len())? else { return Err(bad(id, "the clip's audio is not loaded")) };
    let ratio = target.len() as f64 / c.length.max(1) as f64;
    let scale = |x: Samples| soundcraft_time::to_samples(x as f64 * ratio);
    let mut nc = c.clone();
    nc.content = ClipContent::Audio { source: src, offset: 0 };
    nc.start = target.start;
    nc.length = target.len();
    nc.stretch = 1.0;
    nc.gain_env = c.gain_env.iter().map(|&(o, v)| (scale(o), v)).collect();
    nc.fade_in.len = scale(c.fade_in.len).clamp(0, nc.length);
    nc.fade_out.len = scale(c.fade_out.len).clamp(0, nc.length);
    nc.sync_point = scale(c.sync_point).clamp(0, nc.length);
    replace_clip(s, t, c.id, nc);
    s.edit.selected_clips = vec![c.id];
    Ok(json!({"clip": c.id, "source": src, "ratio": ratio}))
}
