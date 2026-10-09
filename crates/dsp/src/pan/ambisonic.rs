//! First-order ambisonic encoding and decoding.

use super::fin;

/// First-order Ambisonics (ACN channel order, SN3D) encoding gains for a direction, with the
/// directional components scaled by `1 - spread`. Higher-order channels get 0.
pub fn ambisonic_encode(az: f32, el: f32, spread: f32, out: &mut [f32]) {
    out.iter_mut().for_each(|g| *g = 0.0);
    let (a, e) = (fin(az, 0.0).to_radians(), fin(el, 0.0).to_radians());
    let d = 1.0 - fin(spread, 0.0).clamp(0.0, 1.0);
    // Ambisonic azimuth is anticlockwise (left positive); ours is clockwise.
    let vals = [1.0, -a.sin() * e.cos() * d, e.sin() * d, a.cos() * e.cos() * d];
    for (o, v) in out.iter_mut().zip(vals) {
        *o = v;
    }
}

/// Basic first-order Ambisonics decode gains (W, Y, Z, X) for one speaker of an `n`-speaker layout.
pub fn ambisonic_decode(az: f32, el: f32, n: usize) -> [f32; 4] {
    let (a, e) = (fin(az, 0.0).to_radians(), fin(el, 0.0).to_radians());
    let k = 1.0 / n.max(1) as f32;
    [k, 3.0 * k * -a.sin() * e.cos(), 3.0 * k * e.sin(), 3.0 * k * a.cos() * e.cos()]
}
