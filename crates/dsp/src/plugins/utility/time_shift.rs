//! Crossfaded delay compensation.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::params::Params;
use crate::params::lin;
use crate::params::param_plumbing;
use crate::util::DelayLine;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

const MAX_SHIFT_MS: f32 = 1000.0;

const MAX_SHIFT_SAMPLES: f32 = 10_000.0;

const XFADE: usize = 256;

static SHIFT_PARAMS: [ParamInfo; 2] =
    [lin("delay_ms", "Delay", 0.0, MAX_SHIFT_MS, 0.0, Unit::Ms), lin("delay_samples", "Samples", 0.0, MAX_SHIFT_SAMPLES, 0.0, Unit::None)];

pub static SHIFT_INFO: PluginInfo = PluginInfo {
    id: "time_shift",
    name: "Time Shift",
    short_name: "TShift",
    category: Category::Other,
    params: &SHIFT_PARAMS,
    is_instrument: false,
    audiosuite: false,
};

/// Delays the signal by `delay_ms + delay_samples`; changes crossfade between taps (no clicks).
pub struct TimeShift {
    p: Params,
    sr: f32,
    lines: Vec<DelayLine>,
    cur: usize,
    target: usize,
    fade: usize,
}

impl Default for TimeShift {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeShift {
    pub fn new() -> Self {
        Self { p: Params::new(&SHIFT_PARAMS), sr: 48_000.0, lines: Vec::new(), cur: 0, target: 0, fade: 0 }
    }

    fn delay_samples(&self) -> usize {
        let max = self.lines.first().map(|l| l.max_delay() as usize).unwrap_or(1).saturating_sub(2);
        ((ms_to_samples(self.p.v(0), self.sr) + self.p.v(1).max(0.0)).round() as usize).min(max)
    }

    fn update(&mut self) {
        let d = self.delay_samples();
        if d != self.target {
            if self.fade == 0 {
                self.fade = XFADE;
            }
            self.target = d;
        }
    }
}

impl Plugin for TimeShift {
    param_plumbing!(SHIFT_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        let max = (ms_to_samples(MAX_SHIFT_MS, self.sr) + MAX_SHIFT_SAMPLES) as usize + 8;
        self.lines = (0..sane_channels(channels))
            .map(|_| {
                let mut d = DelayLine::default();
                d.allocate(max);
                d
            })
            .collect();
        self.reset();
    }

    fn reset(&mut self) {
        self.lines.iter_mut().for_each(DelayLine::clear);
        self.target = self.delay_samples();
        self.cur = self.target;
        self.fade = 0;
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.lines.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        for n in 0..frames {
            let t = if self.fade > 0 { 1.0 - self.fade as f32 / XFADE as f32 } else { 1.0 };
            for c in 0..ch {
                let (Some(buf), Some(line)) = (io.get_mut(c), self.lines.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                line.push(if x.is_finite() { *x } else { 0.0 });
                let new = line.tap(self.target + 1);
                *x = if self.fade > 0 { line.tap(self.cur + 1) * (1.0 - t) + new * t } else { new };
            }
            if self.fade > 0 {
                self.fade -= 1;
                if self.fade == 0 {
                    self.cur = self.target;
                }
            }
        }
    }

    fn tail_samples(&self) -> usize {
        self.target
    }
}
