//! Continuous test signal generation.

use super::{OFF_DB, fader_gain};
use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::osc::Osc;
use crate::osc::Waveform;
use crate::params::Params;
use crate::params::choice;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::util::Smoother;
use crate::util::frames_in;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

const WAVE_NAMES: &[&str] = &["Sine", "Square", "Saw", "Triangle", "White Noise", "Pink Noise"];

static GEN_PARAMS: [ParamInfo; 3] = [
    choice("waveform", "Waveform", WAVE_NAMES, 0),
    log("freq", "Frequency", 20.0, 20000.0, 1000.0, Unit::Hz),
    lin("level", "Level", OFF_DB, 0.0, -20.0, Unit::Db),
];

pub static GEN_INFO: PluginInfo = PluginInfo {
    id: "signal_generator",
    name: "Signal Generator",
    short_name: "SigGen",
    category: Category::Other,
    params: &GEN_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Test-tone and noise generator; replaces its input.
pub struct SignalGenerator {
    p: Params,
    sr: f32,
    ch: usize,
    osc: Osc,
    level: Smoother,
    freq: Smoother,
}

impl Default for SignalGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalGenerator {
    pub fn new() -> Self {
        let mut g = Self {
            p: Params::new(&GEN_PARAMS),
            sr: 48_000.0,
            ch: 0,
            osc: Osc::new(0xC0FFEE),
            level: Smoother::new(0.1),
            freq: Smoother::new(1000.0),
        };
        g.update();
        g
    }

    fn update(&mut self) {
        self.level.set(fader_gain(self.p.v(2)));
        self.freq.set(self.p.v(1));
    }
}

impl Plugin for SignalGenerator {
    param_plumbing!(GEN_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.ch = sane_channels(channels);
        self.level.set_time(20.0, self.sr);
        self.freq.set_time(20.0, self.sr);
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.osc.reset();
        self.level.snap();
        self.freq.snap();
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.ch.min(io.len());
        let frames = frames_in(io, frames, ch);
        let wave = Waveform::from_index(self.p.choice(0));
        for n in 0..frames {
            let g = self.level.next();
            let f = self.freq.next();
            let y = self.osc.next(wave, f / self.sr) * g;
            for b in io.iter_mut().take(ch) {
                if let Some(x) = b.get_mut(n) {
                    *x = y;
                }
            }
        }
    }
}
