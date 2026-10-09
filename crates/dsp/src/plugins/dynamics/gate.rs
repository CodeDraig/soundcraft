//! Stereo-linked expander and gate.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::db_to_gain;
use crate::gain_to_db;
use crate::params::Params;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;
use crate::util::time_coef;

// ---------------------------------------------------------------------------------------------

static GATE_PARAMS: [ParamInfo; 6] = [
    lin("threshold", "Threshold", -80.0, 0.0, -40.0, Unit::Db),
    log("ratio", "Ratio", 1.0, 100.0, 100.0, Unit::Ratio),
    lin("range", "Range", 0.0, 80.0, 80.0, Unit::Db),
    log("attack", "Attack", 0.01, 100.0, 0.5, Unit::Ms),
    lin("hold", "Hold", 0.0, 2000.0, 50.0, Unit::Ms),
    log("release", "Release", 1.0, 4000.0, 100.0, Unit::Ms),
];

pub static GATE_INFO: PluginInfo = PluginInfo {
    id: "expander_gate",
    name: "Expander/Gate",
    short_name: "Gate",
    category: Category::Dynamics,
    params: &GATE_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Downward expander / gate with hold.
pub struct ExpanderGate {
    p: Params,
    sr: f32,
    ch: usize,
    env: f32,
    env_rel: f32,
    gr: f32,
    att: f32,
    rel: f32,
    hold: usize,
    hold_left: usize,
    gr_meter: f32,
}

impl Default for ExpanderGate {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpanderGate {
    pub fn new() -> Self {
        let mut g = Self {
            p: Params::new(&GATE_PARAMS),
            sr: 48_000.0,
            ch: 0,
            env: 0.0,
            env_rel: 0.0,
            gr: 0.0,
            att: 0.0,
            rel: 0.0,
            hold: 0,
            hold_left: 0,
            gr_meter: 0.0,
        };
        g.update();
        g
    }

    fn update(&mut self) {
        self.att = time_coef(self.p.v(3), self.sr);
        self.rel = time_coef(self.p.v(5), self.sr);
        self.hold = ms_to_samples(self.p.v(4), self.sr) as usize;
        self.env_rel = time_coef(5.0, self.sr);
    }

    /// Returns the gain reduction (dB ≥ 0) for one frame given the linked peak level.
    #[inline]
    pub(crate) fn tick(&mut self, level: f32) -> f32 {
        let level = if level.is_finite() { level } else { 0.0 };
        self.env = level.max(self.env * self.env_rel);
        let ldb = gain_to_db(self.env);
        let thr = self.p.v(0);
        let mut target = if ldb < thr { ((thr - ldb) * (self.p.v(1) - 1.0)).min(self.p.v(2)) } else { 0.0 };
        if target <= 0.0 {
            self.hold_left = self.hold;
        } else if target > self.gr && self.hold_left > 0 {
            self.hold_left -= 1;
            target = self.gr;
        }
        // Opening (less reduction) uses attack, closing uses release.
        let c = if target < self.gr { self.att } else { self.rel };
        self.gr = target + (self.gr - target) * c;
        if !self.gr.is_finite() {
            self.gr = 0.0;
        }
        self.gr
    }
}

impl Plugin for ExpanderGate {
    param_plumbing!(GATE_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.ch = sane_channels(channels);
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.env = 0.0;
        self.gr = 0.0;
        self.hold_left = 0;
        self.gr_meter = 0.0;
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.ch.min(io.len());
        let frames = frames_in(io, frames, ch);
        let mut max_gr = 0.0f32;
        for n in 0..frames {
            let level = io.iter().take(ch).filter_map(|b| b.get(n)).fold(0.0f32, |m, v| m.max(v.abs()));
            let gr = self.tick(level);
            max_gr = max_gr.max(gr);
            let g = db_to_gain(-gr);
            for b in io.iter_mut().take(ch) {
                if let Some(x) = b.get_mut(n) {
                    *x *= g;
                }
            }
        }
        self.gr_meter = max_gr;
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr_meter
    }
}
