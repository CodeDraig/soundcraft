//! Noise-shaped quantization dither.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::params::Params;
use crate::params::choice;
use crate::params::param_plumbing;
use crate::params::toggle;
use crate::util::Rng;
use crate::util::frames_in;
use crate::util::sane_channels;

// ---------------------------------------------------------------------------------------------

static DITHER_PARAMS: [ParamInfo; 2] = [choice("bits", "Bit Depth", &["16", "20", "24"], 0), toggle("noise_shaping", "Noise Shaping", false)];

pub static DITHER_INFO: PluginInfo = PluginInfo {
    id: "dither",
    name: "Dither",
    short_name: "Dither",
    category: Category::Dither,
    params: &DITHER_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// TPDF dither and requantization with optional first-order error-feedback noise shaping.
/// Digital silence passes through as silence.
pub struct Dither {
    p: Params,
    rng: Rng,
    err: Vec<f32>,
}

impl Default for Dither {
    fn default() -> Self {
        Self::new()
    }
}

impl Dither {
    pub fn new() -> Self {
        Self { p: Params::new(&DITHER_PARAMS), rng: Rng::new(0xD17E), err: Vec::new() }
    }

    fn update(&mut self) {}
}

impl Plugin for Dither {
    param_plumbing!(DITHER_INFO);

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, channels: usize) {
        self.err = vec![0.0; sane_channels(channels)];
    }

    fn reset(&mut self) {
        self.err.iter_mut().for_each(|e| *e = 0.0);
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.err.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let bits = [16.0f32, 20.0, 24.0].get(self.p.choice(0)).copied().unwrap_or(16.0);
        let lsb = 2f32.powf(1.0 - bits);
        let shape = self.p.on(1);
        for (b, e) in io.iter_mut().zip(self.err.iter_mut()) {
            for x in b.iter_mut().take(frames) {
                if *x == 0.0 || !x.is_finite() {
                    *x = 0.0;
                    *e = 0.0;
                    continue;
                }
                let v = if shape { *x - *e } else { *x };
                let d = (self.rng.bipolar() + self.rng.bipolar()) * 0.5 * lsb;
                let q = ((v + d) / lsb).round() * lsb;
                *e = q - v;
                *x = q.clamp(-1.0, 1.0 - lsb);
            }
        }
    }
}
