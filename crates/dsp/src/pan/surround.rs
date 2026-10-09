//! Speaker-layout panning, puck coordinates and elevation.

/// Most loudspeakers the surround panner handles (9.1.6 = 16).
pub const MAX_SPEAKERS: usize = 16;

/// A loudspeaker direction for the surround panner: azimuth in degrees (0 = front, positive =
/// right), elevation in degrees (> 0 = height layer), and whether it is the LFE (never panned to).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpeakerPos {
    pub az: f32,
    pub el: f32,
    pub lfe: bool,
}

/// Surround panner controls (see `soundcraft_model::SurroundPan`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurroundParams {
    /// -1 (left) ..= 1 (right).
    pub x: f32,
    /// -1 (back) ..= 1 (front).
    pub y: f32,
    /// 0 (ear level) ..= 1 (overhead).
    pub z: f32,
    /// 0 = point source ..= 1 = every speaker.
    pub divergence: f32,
    /// 0..=1 share of centre-speaker energy kept on C (the rest goes phantom to L/R).
    pub center: f32,
}

pub(super) fn fin(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

fn wrap180(a: f32) -> f32 {
    let mut a = fin(a, 0.0) % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

/// Azimuth (degrees) of a square puck position. The square's front corners land on the front
/// L/R speakers (±30°), its side midpoints at ±90° and its rear corners on the layout's rear-most
/// surround (±110° for 5.x, ±150° for 7.x), so a puck in a corner hits exactly one speaker.
pub fn puck_azimuth(layout: &[SpeakerPos], x: f32, y: f32) -> f32 {
    let (x, y) = (fin(x, 0.0).clamp(-1.0, 1.0), fin(y, 1.0).clamp(-1.0, 1.0));
    if x.abs() < 1e-9 && y.abs() < 1e-9 {
        return 0.0;
    }
    let rear =
        layout.iter().filter(|s| !s.lfe && s.el.abs() < 1.0 && s.az.abs() > 90.5 && s.az.abs() < 179.5).map(|s| s.az.abs()).fold(0.0f32, f32::max);
    let rear = if rear > 0.0 { rear } else { 135.0 };
    let front = if layout.iter().any(|s| !s.lfe && s.el.abs() < 1.0 && (s.az.abs() - 30.0).abs() < 0.5) { 30.0 } else { 45.0 };
    let phi = x.atan2(y).to_degrees();
    let a = phi.abs();
    let warped = if a <= 45.0 {
        a / 45.0 * front
    } else if a <= 90.0 {
        front + (a - 45.0) / 45.0 * (90.0 - front)
    } else if a <= 135.0 {
        90.0 + (a - 90.0) / 45.0 * (rear - 90.0)
    } else {
        rear + (a - 135.0) / 45.0 * (180.0 - rear)
    };
    warped.copysign(phi)
}

/// Inverse of [`puck_azimuth`] on the square's edge (for drawing speakers on a square panner).
pub fn azimuth_to_puck(layout: &[SpeakerPos], az: f32) -> (f32, f32) {
    // Bisect the monotonic warp on the square angle.
    let target = wrap180(az);
    let (mut lo, mut hi) = (0.0f32, 180.0f32);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        let r = mid.to_radians();
        let w = puck_azimuth(layout, r.sin(), r.cos());
        if w < target.abs() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let phi = (0.5 * (lo + hi)).to_radians().copysign(if target < 0.0 { -1.0 } else { 1.0 });
    let (sx, sy) = (phi.sin(), phi.cos());
    let m = sx.abs().max(sy.abs()).max(1e-6);
    (sx / m, sy / m)
}

/// Constant-power pairwise pan of a direction onto one layer (bed or height) of `layout`,
/// accumulating `weight × gain²` into `pow`. Returns false when the layer has no speakers.
fn pair_power(layout: &[SpeakerPos], height: bool, az: f32, weight: f32, pow: &mut [f32; MAX_SPEAKERS]) -> bool {
    let mut idx = [0usize; MAX_SPEAKERS];
    let mut n = 0;
    for (i, s) in layout.iter().enumerate().take(MAX_SPEAKERS) {
        if !s.lfe
            && (s.el > 1.0) == height
            && let Some(slot) = idx.get_mut(n)
        {
            *slot = i;
            n += 1;
        }
    }
    let ids = idx.get(..n).unwrap_or(&[]);
    let az_of = |i: usize| layout.get(i).map_or(0.0, |s| wrap180(s.az));
    match ids {
        [] => false,
        [only] => {
            if let Some(p) = pow.get_mut(*only) {
                *p += weight;
            }
            true
        }
        [first, ..] => {
            let a = wrap180(az);
            // The nearest speaker clockwise (to the right, increasing azimuth) and anticlockwise.
            let (mut cw, mut cw_d) = (*first, f32::MAX);
            let (mut acw, mut acw_d) = (*first, f32::MAX);
            for &i in ids {
                let d = (az_of(i) - a).rem_euclid(360.0);
                if d < cw_d {
                    cw_d = d;
                    cw = i;
                }
                let d2 = (a - az_of(i)).rem_euclid(360.0);
                if d2 < acw_d {
                    acw_d = d2;
                    acw = i;
                }
            }
            if cw == acw || cw_d < 1e-4 {
                if let Some(p) = pow.get_mut(cw) {
                    *p += weight;
                }
                return true;
            }
            let span = (cw_d + acw_d).max(1e-6);
            let t = (acw_d / span).clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
            if let Some(p) = pow.get_mut(acw) {
                *p += weight * t.cos() * t.cos();
            }
            if let Some(p) = pow.get_mut(cw) {
                *p += weight * t.sin() * t.sin();
            }
            true
        }
    }
}

/// Point-source gains for a direction (azimuth, elevation in degrees) on `layout`: pairwise
/// constant power on the speaker layer nearest in elevation; a height direction on a layout
/// without heights folds to the bed at -3 dB. Writes `out[..layout.len()]`.
pub fn direction_gains(layout: &[SpeakerPos], az: f32, el: f32, out: &mut [f32]) {
    let mut pow = [0.0f32; MAX_SPEAKERS];
    let height = fin(el, 0.0) > 1.0;
    if height && !pair_power(layout, true, az, 1.0, &mut pow) {
        pair_power(layout, false, az, 0.5, &mut pow);
    } else if !height {
        pair_power(layout, false, az, 1.0, &mut pow);
    }
    for (o, p) in out.iter_mut().zip(pow.iter()) {
        *o = p.max(0.0).sqrt();
    }
}

/// Surround panner: gains for every speaker of `layout` (LFE gets 0; the caller adds an LFE send).
/// Energy-preserving: the squared gains of the non-LFE speakers always sum to 1.
///
/// The puck's azimuth comes from [`puck_azimuth`]. Divergence (and moving the puck towards the
/// middle of the room) spreads the image: the gains are the power average of point sources
/// sampled across `±spread × 180°`. `center` < 1 moves that share of the centre speaker's power
/// to a phantom centre on L/R. Elevation crossfades bed and height layers (constant power).
pub fn surround_gains(layout: &[SpeakerPos], p: &SurroundParams, out: &mut [f32]) {
    out.iter_mut().for_each(|g| *g = 0.0);
    let layout = layout.get(..layout.len().min(MAX_SPEAKERS)).unwrap_or(&[]);
    if layout.is_empty() {
        return;
    }
    let az = puck_azimuth(layout, p.x, p.y);
    let dist = fin(p.x, 0.0).abs().max(fin(p.y, 1.0).abs()).clamp(0.0, 1.0);
    let div = fin(p.divergence, 0.0).clamp(0.0, 1.0);
    let spread = 1.0 - dist * (1.0 - div);
    let k = 1 + (spread * 24.0).round() as usize;
    let width = spread * 180.0;
    let has_height = layout.iter().any(|s| !s.lfe && s.el > 1.0);
    let z = if has_height { fin(p.z, 0.0).clamp(0.0, 1.0) } else { 0.0 };
    let (bed_w, top_w) = {
        let a = z * std::f32::consts::FRAC_PI_2;
        (a.cos() * a.cos(), a.sin() * a.sin())
    };
    let mut pow = [0.0f32; MAX_SPEAKERS];
    for i in 0..k {
        let a = if k == 1 { az } else { az - width + 2.0 * width * (i as f32 + 0.5) / k as f32 };
        let w = 1.0 / k as f32;
        if bed_w > 1e-9 && !pair_power(layout, false, a, w * bed_w, &mut pow) {
            pair_power(layout, true, a, w * bed_w, &mut pow);
        }
        if top_w > 1e-9 {
            pair_power(layout, true, a, w * top_w, &mut pow);
        }
    }
    // Centre %: move energy from C to a phantom centre on L/R.
    let find = |az: f32| layout.iter().position(|s| !s.lfe && s.el.abs() < 1.0 && (s.az - az).abs() < 0.5);
    let c_share = fin(p.center, 1.0).clamp(0.0, 1.0);
    if let (Some(c), Some(l), Some(r)) = (find(0.0), find(-30.0), find(30.0))
        && c_share < 1.0
    {
        let pc = pow.get(c).copied().unwrap_or(0.0);
        let moved = pc * (1.0 - c_share);
        if let Some(x) = pow.get_mut(c) {
            *x = pc - moved;
        }
        for i in [l, r] {
            if let Some(x) = pow.get_mut(i) {
                *x += 0.5 * moved;
            }
        }
    }
    for (o, p) in out.iter_mut().zip(pow.iter()) {
        *o = p.max(0.0).sqrt();
    }
}
