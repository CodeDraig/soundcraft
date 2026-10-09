//! Rectification and DC removal.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::db_to_gain;
use crate::params::Params;
use crate::params::choice;
use crate::params::lin;
use crate::params::param_plumbing;
use crate::util::DcBlocker;
use crate::util::Smoother;
use crate::util::frames_in;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

static RECT_PARAMS: [ParamInfo; 3] = [
    choice("mode", "Mode", &["Half Wave", "Full Wave"], 1),
    lin("mix", "Mix", 0.0, 100.0, 100.0, Unit::Percent),
    lin("output", "Output", -24.0, 12.0, 0.0, Unit::Db),
];

pub static RECT_INFO: PluginInfo = PluginInfo {
    id: "rectifier",
    name: "Rectifier",
    short_name: "Rectify",
    category: Category::Harmonic,
    params: &RECT_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Half/full-wave rectifier (octave-up harmonics) with DC removal.
pub struct Rectifier {
    p: Params,
    sr: f32,
    dc: Vec<DcBlocker>,
    mix: Smoother,
    out: Smoother,
}

impl Default for Rectifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Rectifier {
    pub fn new() -> Self {
        let mut r = Self { p: Params::new(&RECT_PARAMS), sr: 48_000.0, dc: Vec::new(), mix: Smoother::new(1.0), out: Smoother::new(1.0) };
        r.update();
        r
    }

    fn update(&mut self) {
        self.mix.set(self.p.v(1) / 100.0);
        self.out.set(db_to_gain(self.p.v(2)));
    }
}

impl Plugin for Rectifier {
    param_plumbing!(RECT_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.dc = vec![DcBlocker::new(10.0, self.sr); sane_channels(channels)];
        self.mix.set_time(20.0, self.sr);
        self.out.set_time(20.0, self.sr);
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.dc.iter_mut().for_each(DcBlocker::reset);
        self.mix.snap();
        self.out.snap();
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.dc.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let full = self.p.choice(0) == 1;
        for n in 0..frames {
            let m = self.mix.next();
            let o = self.out.next();
            for c in 0..ch {
                let (Some(buf), Some(dc)) = (io.get_mut(c), self.dc.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                let dry = if x.is_finite() { *x } else { 0.0 };
                let r = if full { dry.abs() } else { dry.max(0.0) };
                let wet = dc.process(r);
                *x = (dry * (1.0 - m) + wet * m) * o;
            }
        }
    }
}
