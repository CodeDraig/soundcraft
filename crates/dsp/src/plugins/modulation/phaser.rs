//! Modulated all-pass phase shifting.

use super::channel_phase;
use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::params::Params;
use crate::params::choice;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::util::Lfo;
use crate::util::Smoother;
use crate::util::flush;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

const STAGE_CHOICES: &[&str] = &["2", "4", "6", "8", "12"];

const STAGE_COUNTS: [usize; 5] = [2, 4, 6, 8, 12];

const MAX_STAGES: usize = 12;

static PHASER_PARAMS: [ParamInfo; 6] = [
    log("rate", "Rate", 0.02, 5.0, 0.5, Unit::Hz),
    lin("depth", "Depth", 0.0, 100.0, 70.0, Unit::Percent),
    choice("stages", "Stages", STAGE_CHOICES, 2),
    lin("feedback", "Feedback", -95.0, 95.0, 40.0, Unit::Percent),
    log("center", "Center", 100.0, 4000.0, 800.0, Unit::Hz),
    lin("mix", "Mix", 0.0, 100.0, 50.0, Unit::Percent),
];

pub static PHASER_INFO: PluginInfo = PluginInfo {
    id: "phaser",
    name: "Phaser",
    short_name: "Phaser",
    category: Category::Modulation,
    params: &PHASER_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Cascade of modulated first-order allpasses with feedback.
pub struct Phaser {
    p: Params,
    sr: f32,
    ch: usize,
    z: Vec<[f32; MAX_STAGES]>,
    fbs: Vec<f32>,
    lfo: Lfo,
    depth: Smoother,
    fb: Smoother,
    center: Smoother,
    mix: Smoother,
}

impl Default for Phaser {
    fn default() -> Self {
        Self::new()
    }
}

impl Phaser {
    pub fn new() -> Self {
        let mut p = Self {
            p: Params::new(&PHASER_PARAMS),
            sr: 48_000.0,
            ch: 0,
            z: Vec::new(),
            fbs: Vec::new(),
            lfo: Lfo::new(),
            depth: Smoother::new(0.0),
            fb: Smoother::new(0.0),
            center: Smoother::new(800.0),
            mix: Smoother::new(0.0),
        };
        p.update();
        p
    }

    fn update(&mut self) {
        self.depth.set(self.p.v(1) / 100.0);
        self.fb.set(self.p.v(3) / 100.0);
        self.center.set(self.p.v(4));
        self.mix.set(self.p.v(5) / 100.0);
    }
}

impl Plugin for Phaser {
    param_plumbing!(PHASER_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.ch = sane_channels(channels);
        self.z = vec![[0.0; MAX_STAGES]; self.ch];
        self.fbs = vec![0.0; self.ch];
        for s in [&mut self.depth, &mut self.fb, &mut self.center, &mut self.mix] {
            s.set_time(30.0, self.sr);
        }
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.z.iter_mut().for_each(|z| *z = [0.0; MAX_STAGES]);
        self.fbs.iter_mut().for_each(|v| *v = 0.0);
        self.lfo.reset();
        for s in [&mut self.depth, &mut self.fb, &mut self.center, &mut self.mix] {
            s.snap();
        }
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.ch.min(io.len());
        let frames = frames_in(io, frames, ch);
        let inc = self.p.v(0) / self.sr;
        let stages = STAGE_COUNTS.get(self.p.choice(2)).copied().unwrap_or(6);
        let nyq = self.sr * 0.45;
        for n in 0..frames {
            let phase = self.lfo.tick(inc);
            let depth = self.depth.next();
            let fb = self.fb.next();
            let center = self.center.next();
            let m = self.mix.next();
            for c in 0..ch {
                let (Some(buf), Some(z), Some(fbs)) = (io.get_mut(c), self.z.get_mut(c), self.fbs.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                let dry = if x.is_finite() { *x } else { 0.0 };
                let s = Lfo::sin_at(channel_phase(phase, c, 0.25));
                let f = (center * 2f32.powf(2.0 * depth * s)).clamp(20.0, nyq);
                let t = (std::f32::consts::PI * f / self.sr).tan();
                let a = (t - 1.0) / (t + 1.0);
                let mut y = dry + fb * *fbs;
                for zi in z.iter_mut().take(stages) {
                    let out = a * y + *zi;
                    *zi = flush(y - a * out);
                    y = out;
                }
                *fbs = flush(y);
                *x = dry * (1.0 - m) + y * m;
            }
        }
    }

    fn tail_samples(&self) -> usize {
        ms_to_samples(100.0, self.sr) as usize
    }
}
