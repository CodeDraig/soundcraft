//! Smoothed gain and polarity trim.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::db_to_gain;
use crate::params::Params;
use crate::params::lin;
use crate::params::param_plumbing;
use crate::params::toggle;
use crate::util::Smoother;
use crate::util::frames_in;
use crate::util::sane_channels;
use crate::util::sane_sr;

/// Gain parameter floor that means "-inf".
pub(super) const OFF_DB: f32 = -96.0;

pub(super) fn fader_gain(db: f32) -> f32 {
    if db <= OFF_DB { 0.0 } else { db_to_gain(db) }
}

/// Applies a per-sample smoothed gain to all channels.
fn apply_gain(io: &mut [Vec<f32>], frames: usize, ch: usize, g: &mut Smoother) {
    let ch = ch.min(io.len());
    let frames = frames_in(io, frames, ch);
    if !g.settling() {
        let v = g.value();
        if v == 1.0 {
            return;
        }
        io.iter_mut().take(ch).for_each(|b| b.iter_mut().take(frames).for_each(|x| *x *= v));
        return;
    }
    for n in 0..frames {
        let v = g.next();
        for b in io.iter_mut().take(ch) {
            if let Some(x) = b.get_mut(n) {
                *x *= v;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------

static GAIN_PARAMS: [ParamInfo; 1] = [lin("gain", "Gain", OFF_DB, 24.0, 0.0, Unit::Db)];

pub static GAIN_INFO: PluginInfo = PluginInfo {
    id: "gain",
    name: "Gain",
    short_name: "Gain",
    category: Category::Other,
    params: &GAIN_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Smoothed gain change (-inf .. +24 dB).
pub struct Gain {
    p: Params,
    ch: usize,
    g: Smoother,
}

impl Default for Gain {
    fn default() -> Self {
        Self::new()
    }
}

impl Gain {
    pub fn new() -> Self {
        Self { p: Params::new(&GAIN_PARAMS), ch: 0, g: Smoother::new(1.0) }
    }

    fn update(&mut self) {
        self.g.set(fader_gain(self.p.v(0)));
    }
}

impl Plugin for Gain {
    param_plumbing!(GAIN_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.ch = sane_channels(channels);
        self.g.set_time(20.0, sane_sr(sample_rate));
        self.update();
        self.g.snap();
    }

    fn reset(&mut self) {
        self.g.snap();
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        apply_gain(io, frames, self.ch, &mut self.g);
    }
}

// ---------------------------------------------------------------------------------------------

static TRIM_PARAMS: [ParamInfo; 2] = [lin("gain", "Trim", OFF_DB, 12.0, 0.0, Unit::Db), toggle("invert", "Phase Invert", false)];

pub static TRIM_INFO: PluginInfo = PluginInfo {
    id: "trim",
    name: "Trim",
    short_name: "Trim",
    category: Category::Other,
    params: &TRIM_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Trim (-inf .. +12 dB) with a click-free polarity switch.
pub struct Trim {
    p: Params,
    ch: usize,
    g: Smoother,
}

impl Default for Trim {
    fn default() -> Self {
        Self::new()
    }
}

impl Trim {
    pub fn new() -> Self {
        Self { p: Params::new(&TRIM_PARAMS), ch: 0, g: Smoother::new(1.0) }
    }

    fn update(&mut self) {
        let sign = if self.p.on(1) { -1.0 } else { 1.0 };
        self.g.set(sign * fader_gain(self.p.v(0)));
    }
}

impl Plugin for Trim {
    param_plumbing!(TRIM_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.ch = sane_channels(channels);
        self.g.set_time(10.0, sane_sr(sample_rate));
        self.update();
        self.g.snap();
    }

    fn reset(&mut self) {
        self.g.snap();
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        apply_gain(io, frames, self.ch, &mut self.g);
    }
}
