//! Lookahead peak limiting.

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
use crate::util::DelayLine;
use crate::util::Smoother;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;
use crate::util::time_coef;

// ---------------------------------------------------------------------------------------------

static MAX_PARAMS: [ParamInfo; 3] = [
    lin("threshold", "Threshold", -30.0, 0.0, 0.0, Unit::Db),
    lin("ceiling", "Ceiling", -30.0, 0.0, -0.3, Unit::Db),
    log("release", "Release", 1.0, 1000.0, 50.0, Unit::Ms),
];

pub static MAX_INFO: PluginInfo = PluginInfo {
    id: "maximizer",
    name: "Maximizer",
    short_name: "Max",
    category: Category::Dynamics,
    params: &MAX_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

const MAX_LOOKAHEAD_MS: f32 = 1.5;

/// Lookahead brickwall limiter: the input is driven by `-threshold` dB and never exceeds the
/// ceiling. Gain is a min-hold over the lookahead window followed by an equal-length moving
/// average, which ramps the gain down before each peak reaches the output.
pub struct Maximizer {
    p: Params,
    sr: f32,
    la: usize,
    delay: Vec<DelayLine>,
    req: Vec<f32>,
    smooth: Vec<f32>,
    idx: usize,
    sum: f64,
    held: f32,
    rel: f32,
    drive: Smoother,
    ceiling: Smoother,
    gr_meter: f32,
}

impl Default for Maximizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Maximizer {
    pub fn new() -> Self {
        let mut m = Self {
            p: Params::new(&MAX_PARAMS),
            sr: 48_000.0,
            la: 1,
            delay: Vec::new(),
            req: vec![1.0],
            smooth: vec![1.0],
            idx: 0,
            sum: 1.0,
            held: 1.0,
            rel: 0.0,
            drive: Smoother::new(1.0),
            ceiling: Smoother::new(1.0),
            gr_meter: 0.0,
        };
        m.update();
        m
    }

    fn update(&mut self) {
        self.drive.set(db_to_gain(-self.p.v(0)));
        // The ceiling is a hard guarantee, so it changes immediately.
        self.ceiling.set(db_to_gain(self.p.v(1)));
        self.ceiling.snap();
        self.rel = time_coef(self.p.v(2), self.sr);
    }
}

impl Plugin for Maximizer {
    param_plumbing!(MAX_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.la = (ms_to_samples(MAX_LOOKAHEAD_MS, self.sr).round() as usize).max(1);
        self.delay = (0..sane_channels(channels))
            .map(|_| {
                let mut d = DelayLine::default();
                d.allocate(self.la + 2);
                d
            })
            .collect();
        self.req = vec![1.0; self.la];
        self.smooth = vec![1.0; self.la];
        self.drive.set_time(20.0, self.sr);
        self.ceiling.set_time(20.0, self.sr);
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.delay.iter_mut().for_each(DelayLine::clear);
        self.req.iter_mut().for_each(|v| *v = 1.0);
        self.smooth.iter_mut().for_each(|v| *v = 1.0);
        self.sum = self.la as f64;
        self.held = 1.0;
        self.idx = 0;
        self.drive.snap();
        self.ceiling.snap();
        self.gr_meter = 0.0;
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.delay.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let la = self.la.max(1);
        let mut min_gain = 1.0f32;
        for n in 0..frames {
            let drive = self.drive.next();
            let ceil = self.ceiling.next();
            let mut peak = 0.0f32;
            for c in 0..ch {
                let (Some(buf), Some(d)) = (io.get(c), self.delay.get_mut(c)) else { continue };
                let x = buf.get(n).copied().filter(|v| v.is_finite()).unwrap_or(0.0) * drive;
                d.push(x);
                peak = peak.max(x.abs());
            }
            let req = if peak > ceil { ceil / peak } else { 1.0 };
            if let Some(r) = self.req.get_mut(self.idx) {
                *r = req;
            }
            let hold = self.req.iter().copied().fold(1.0f32, f32::min);
            self.held = if hold < self.held { hold } else { hold + (self.held - hold) * self.rel };
            if let Some(s) = self.smooth.get_mut(self.idx) {
                self.sum += f64::from(self.held) - f64::from(*s);
                *s = self.held;
            }
            self.idx = (self.idx + 1) % la;
            if self.idx == 0 {
                // Re-sum once per window to cancel floating-point drift.
                self.sum = self.smooth.iter().map(|&v| f64::from(v)).sum();
            }
            let g = ((self.sum / la as f64) as f32).clamp(0.0, 1.0);
            min_gain = min_gain.min(g);
            for c in 0..ch {
                let (Some(buf), Some(d)) = (io.get_mut(c), self.delay.get(c)) else { continue };
                if let Some(x) = buf.get_mut(n) {
                    *x = (d.tap(la) * g).clamp(-ceil, ceil);
                }
            }
        }
        self.gr_meter = -gain_to_db(min_gain);
    }

    fn latency(&self) -> usize {
        self.la - 1
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr_meter
    }
}
