//! Interpolation and replacement of clip gain envelopes.

use soundcraft_model::Clip;
use soundcraft_time::Samples;

/// Interpolated value of a `(offset, dB)` envelope (0 when empty), like the model's clip gain.
pub fn env_db(env: &[(Samples, f32)], at: Samples) -> f32 {
    let mut prev: Option<(Samples, f32)> = None;
    for &(t, v) in env {
        if t >= at {
            return match prev {
                Some((pt, pv)) if t > pt => pv + (v - pv) * ((at - pt) as f32 / (t - pt) as f32),
                _ => v,
            };
        }
        prev = Some((t, v));
    }
    prev.map_or(0.0, |(_, v)| v)
}

/// Absolute clip gain (static gain + envelope) in dB at `rel` samples from the clip start.
pub fn clip_db(c: &Clip, rel: Samples) -> f32 {
    c.gain_db + if c.gain_env.is_empty() { 0.0 } else { env_db(&c.gain_env, rel) }
}

/// Replace a clip's gain with an absolute envelope (sorted, deduplicated, clamped).
pub fn set_clip_env(c: &mut Clip, mut pts: Vec<(Samples, f32)>) {
    pts.retain(|(o, v)| *o >= 0 && *o <= c.length && v.is_finite());
    pts.sort_by_key(|p| p.0);
    pts.dedup_by_key(|p| p.0);
    for p in &mut pts {
        p.1 = p.1.clamp(-144.0, 36.0);
    }
    c.gain_db = 0.0;
    let flat = pts.windows(2).all(|w| w.first().map(|a| a.1) == w.get(1).map(|b| b.1));
    if flat {
        c.gain_db = pts.first().map_or(0.0, |p| p.1);
        c.gain_env.clear();
    } else {
        c.gain_env = pts;
    }
}
