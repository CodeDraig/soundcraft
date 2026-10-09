//! Bit and sample-rate reduction with controlled noise.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::params::Params;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::util::Rng;
use crate::util::Smoother;
use crate::util::frames_in;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

static LOFI_PARAMS: [ParamInfo; 4] = [
    lin("bits", "Bit Depth", 1.0, 24.0, 8.0, Unit::None),
    log("sample_rate", "Sample Rate", 500.0, 48000.0, 11025.0, Unit::Hz),
    lin("noise", "Noise", 0.0, 100.0, 0.0, Unit::Percent),
    lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
];

pub static LOFI_INFO: PluginInfo = PluginInfo {
    id: "lofi",
    name: "Lo-Fi",
    short_name: "LoFi",
    category: Category::Harmonic,
    params: &LOFI_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Bit crusher and sample-and-hold decimator with optional hiss.
pub struct LoFi {
    p: Params,
    sr: f32,
    held: Vec<f32>,
    phase: f32,
    rng: Rng,
    mix: Smoother,
    noise: Smoother,
}

impl Default for LoFi {
    fn default() -> Self {
        Self::new()
    }
}

impl LoFi {
    pub fn new() -> Self {
        let mut l = Self {
            p: Params::new(&LOFI_PARAMS),
            sr: 48_000.0,
            held: Vec::new(),
            phase: 1.0,
            rng: Rng::new(77),
            mix: Smoother::new(1.0),
            noise: Smoother::new(0.0),
        };
        l.update();
        l
    }

    fn update(&mut self) {
        self.mix.set(self.p.v(3) / 100.0);
        let n = self.p.v(2) / 100.0;
        self.noise.set(n * n * 0.1);
    }
}

impl Plugin for LoFi {
    param_plumbing!(LOFI_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.held = vec![0.0; sane_channels(channels)];
        self.mix.set_time(20.0, self.sr);
        self.noise.set_time(20.0, self.sr);
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.held.iter_mut().for_each(|v| *v = 0.0);
        self.phase = 1.0;
        self.mix.snap();
        self.noise.snap();
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.held.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let step = 2f32.powf(1.0 - self.p.v(0));
        let inc = (self.p.v(1) / self.sr).min(1.0);
        for n in 0..frames {
            let m = self.mix.next();
            let nz = self.noise.next();
            self.phase += inc;
            let grab = self.phase >= 1.0;
            if grab {
                self.phase -= 1.0;
            }
            for c in 0..ch {
                let (Some(buf), Some(h)) = (io.get_mut(c), self.held.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                let dry = if x.is_finite() { *x } else { 0.0 };
                if grab {
                    let noisy = dry + if nz > 0.0 { self.rng.bipolar() * nz } else { 0.0 };
                    *h = (noisy / step).round() * step;
                }
                *x = dry * (1.0 - m) + *h * m;
            }
        }
    }
}
