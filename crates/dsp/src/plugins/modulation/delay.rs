//! Modulated feedback delay.

use super::channel_phase;
use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::params::Params;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::util::DelayLine;
use crate::util::Lfo;
use crate::util::Smoother;
use crate::util::flush;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

const MAX_DELAY_MS: f32 = 2000.0;

const MOD_DELAY_DEPTH_MS: f32 = 5.0;

static DELAY_PARAMS: [ParamInfo; 6] = [
    log("time", "Time", 1.0, MAX_DELAY_MS, 250.0, Unit::Ms),
    lin("feedback", "Feedback", -99.0, 99.0, 30.0, Unit::Percent),
    lin("mix", "Mix", 0.0, 100.0, 30.0, Unit::Percent),
    log("mod_rate", "Mod Rate", 0.01, 10.0, 0.5, Unit::Hz),
    lin("mod_depth", "Mod Depth", 0.0, 100.0, 0.0, Unit::Percent),
    log("lpf", "LPF", 500.0, 20000.0, 12000.0, Unit::Hz),
];

pub static DELAY_INFO: PluginInfo = PluginInfo {
    id: "mod_delay",
    name: "Mod Delay",
    short_name: "ModDly",
    category: Category::Delay,
    params: &DELAY_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Feedback delay with modulation and a low-pass in the feedback path.
pub struct ModDelay {
    p: Params,
    sr: f32,
    lines: Vec<DelayLine>,
    lp: Vec<f32>,
    lfo: Lfo,
    time: Smoother,
    fb: Smoother,
    mix: Smoother,
    depth: Smoother,
    lp_coef: f32,
}

impl Default for ModDelay {
    fn default() -> Self {
        Self::new()
    }
}

impl ModDelay {
    pub fn new() -> Self {
        let mut d = Self {
            p: Params::new(&DELAY_PARAMS),
            sr: 48_000.0,
            lines: Vec::new(),
            lp: Vec::new(),
            lfo: Lfo::new(),
            time: Smoother::new(250.0),
            fb: Smoother::new(0.0),
            mix: Smoother::new(0.0),
            depth: Smoother::new(0.0),
            lp_coef: 0.0,
        };
        d.update();
        d
    }

    fn update(&mut self) {
        self.time.set(self.p.v(0));
        self.fb.set(self.p.v(1) / 100.0);
        self.mix.set(self.p.v(2) / 100.0);
        self.depth.set(self.p.v(4) / 100.0 * MOD_DELAY_DEPTH_MS);
        self.lp_coef = (-std::f32::consts::TAU * self.p.v(5) / self.sr).exp();
    }
}

impl Plugin for ModDelay {
    param_plumbing!(DELAY_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        let max = ms_to_samples(MAX_DELAY_MS + 2.0 * MOD_DELAY_DEPTH_MS, self.sr) as usize + 8;
        self.lines = (0..sane_channels(channels))
            .map(|_| {
                let mut d = DelayLine::default();
                d.allocate(max);
                d
            })
            .collect();
        self.lp = vec![0.0; self.lines.len()];
        self.time.set_time(120.0, self.sr);
        for s in [&mut self.fb, &mut self.mix, &mut self.depth] {
            s.set_time(20.0, self.sr);
        }
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.lines.iter_mut().for_each(DelayLine::clear);
        self.lp.iter_mut().for_each(|v| *v = 0.0);
        self.lfo.reset();
        for s in [&mut self.time, &mut self.fb, &mut self.mix, &mut self.depth] {
            s.snap();
        }
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.lines.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let inc = self.p.v(3) / self.sr;
        let a = self.lp_coef;
        for n in 0..frames {
            let phase = self.lfo.tick(inc);
            let t = self.time.next();
            let fb = self.fb.next();
            let m = self.mix.next();
            let depth = self.depth.next();
            for c in 0..ch {
                let (Some(buf), Some(line), Some(lp)) = (io.get_mut(c), self.lines.get_mut(c), self.lp.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                let dry = if x.is_finite() { *x } else { 0.0 };
                let modv = depth * 0.5 * (1.0 + Lfo::sin_at(channel_phase(phase, c, 0.25)));
                let d = ms_to_samples(t + modv, self.sr);
                let y = line.read_cubic(d);
                *lp = flush(y + (*lp - y) * a);
                line.push(flush(dry + fb * *lp));
                *x = dry * (1.0 - m) + *lp * m;
            }
        }
    }

    fn tail_samples(&self) -> usize {
        // Time for feedback to fall by 60 dB.
        let fb = (self.p.v(1).abs() / 100.0).clamp(0.0, 0.99);
        let repeats = if fb < 1e-3 { 1.0 } else { (-60.0 / (20.0 * fb.log10())).min(500.0) + 1.0 };
        ms_to_samples(self.p.v(0) * repeats, self.sr) as usize
    }
}
