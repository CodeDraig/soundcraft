//! Stereo-linked compressor and limiter.

use super::{ballistic, compress_gr};
use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::biquad::Biquad;
use crate::biquad::FilterType;
use crate::db_to_gain;
use crate::gain_to_db;
use crate::params::Params;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::params::toggle;
use crate::util::DelayLine;
use crate::util::Smoother;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;
use crate::util::time_coef;

// ---------------------------------------------------------------------------------------------

static COMP_PARAMS: [ParamInfo; 11] = [
    lin("threshold", "Threshold", -60.0, 0.0, -20.0, Unit::Db),
    log("ratio", "Ratio", 1.0, 100.0, 4.0, Unit::Ratio),
    log("attack", "Attack", 0.01, 300.0, 10.0, Unit::Ms),
    log("release", "Release", 5.0, 3000.0, 100.0, Unit::Ms),
    lin("knee", "Knee", 0.0, 24.0, 6.0, Unit::Db),
    lin("makeup", "Makeup", 0.0, 40.0, 0.0, Unit::Db),
    toggle("sc_hpf_on", "SC HPF", false),
    log("sc_hpf_freq", "SC HPF Freq", 20.0, 500.0, 100.0, Unit::Hz),
    toggle("limiter", "Limit", false),
    lin("lookahead", "Lookahead", 0.0, 10.0, 0.0, Unit::Ms),
    lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
];

pub static COMP_INFO: PluginInfo = PluginInfo {
    id: "compressor",
    name: "Compressor/Limiter",
    short_name: "Comp",
    category: Category::Dynamics,
    params: &COMP_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Feed-forward compressor / limiter with optional lookahead and sidechain high-pass.
pub struct Compressor {
    p: Params,
    sr: f32,
    sc: Vec<Biquad>,
    delay: Vec<DelayLine>,
    gr: f32,
    att: f32,
    rel: f32,
    makeup: Smoother,
    mix: Smoother,
    lookahead: usize,
    gr_meter: f32,
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor {
    pub fn new() -> Self {
        let mut c = Self {
            p: Params::new(&COMP_PARAMS),
            sr: 48_000.0,
            sc: Vec::new(),
            delay: Vec::new(),
            gr: 0.0,
            att: 0.0,
            rel: 0.0,
            makeup: Smoother::new(1.0),
            mix: Smoother::new(1.0),
            lookahead: 0,
            gr_meter: 0.0,
        };
        c.update();
        c
    }

    fn limiting(&self) -> bool {
        self.p.on(8)
    }

    fn update(&mut self) {
        let sr = self.sr;
        self.att = time_coef(self.p.v(2), sr);
        self.rel = time_coef(self.p.v(3), sr);
        self.makeup.set(db_to_gain(self.p.v(5)));
        self.mix.set(self.p.v(10) / 100.0);
        let f = self.p.v(7);
        for b in &mut self.sc {
            b.set(FilterType::HighPass, f, std::f32::consts::FRAC_1_SQRT_2, 0.0, sr);
        }
        let max = self.delay.first().map(|d| d.max_delay() as usize).unwrap_or(0);
        self.lookahead = (ms_to_samples(self.p.v(9), sr).round() as usize).min(max);
    }

    /// Processes one frame of gain computation from a sidechain level (linear).
    #[inline]
    fn gain_for(&mut self, level: f32) -> f32 {
        let ratio = if self.limiting() { 1000.0 } else { self.p.v(1) };
        let target = compress_gr(gain_to_db(level), self.p.v(0), ratio, self.p.v(4));
        self.gr = ballistic(self.gr, target, self.att, self.rel);
        if !self.gr.is_finite() {
            self.gr = 0.0;
        }
        self.gr
    }
}

impl Plugin for Compressor {
    param_plumbing!(COMP_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        let ch = sane_channels(channels);
        self.sc = vec![Biquad::default(); ch];
        let max = ms_to_samples(10.0, self.sr).ceil() as usize + 2;
        self.delay = (0..ch)
            .map(|_| {
                let mut d = DelayLine::default();
                d.allocate(max);
                d
            })
            .collect();
        self.makeup.set_time(20.0, self.sr);
        self.mix.set_time(20.0, self.sr);
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.sc.iter_mut().for_each(Biquad::reset);
        self.delay.iter_mut().for_each(DelayLine::clear);
        self.gr = 0.0;
        self.gr_meter = 0.0;
        self.makeup.snap();
        self.mix.snap();
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.sc.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let hpf = self.p.on(6);
        let la = self.lookahead;
        let mut max_gr = 0.0f32;
        for n in 0..frames {
            let mut level = 0.0f32;
            for c in 0..ch {
                let x = io.get(c).and_then(|b| b.get(n)).copied().unwrap_or(0.0);
                let s = match self.sc.get_mut(c) {
                    Some(f) if hpf => f.process_sample(x),
                    _ => x,
                };
                if s.is_finite() {
                    level = level.max(s.abs());
                }
            }
            let gr = self.gain_for(level);
            max_gr = max_gr.max(gr);
            let g = db_to_gain(-gr) * self.makeup.next();
            let m = self.mix.next();
            for c in 0..ch {
                let (Some(buf), Some(d)) = (io.get_mut(c), self.delay.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                d.push(*x);
                let dry = if la == 0 { *x } else { d.tap(la + 1) };
                *x = dry * (1.0 - m) + dry * g * m;
            }
        }
        self.gr_meter = max_gr;
    }

    fn latency(&self) -> usize {
        self.lookahead
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr_meter
    }
}
