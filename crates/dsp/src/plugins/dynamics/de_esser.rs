//! Band-selective sibilance attenuation.

use super::ballistic;
use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::biquad::Biquad;
use crate::biquad::Coeffs;
use crate::biquad::FilterType;
use crate::gain_to_db;
use crate::params::Params;
use crate::params::lin;
use crate::params::log;
use crate::params::param_plumbing;
use crate::params::toggle;
use crate::util::frames_in;
use crate::util::sane_channels;
use crate::util::sane_sr;
use crate::util::time_coef;

// ---------------------------------------------------------------------------------------------

static DEESS_PARAMS: [ParamInfo; 4] = [
    log("freq", "Freq", 2000.0, 16000.0, 6000.0, Unit::Hz),
    lin("range", "Range", 0.0, 40.0, 10.0, Unit::Db),
    lin("threshold", "Threshold", -60.0, 0.0, -30.0, Unit::Db),
    toggle("listen", "Listen", false),
];

pub static DEESS_INFO: PluginInfo = PluginInfo {
    id: "de_esser",
    name: "De-Esser",
    short_name: "DeEss",
    category: Category::Dynamics,
    params: &DEESS_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Dynamic-EQ de-esser: a high-pass sidechain detects sibilance and a high shelf at `freq`
/// dips by up to `range` dB while it exceeds the threshold. At rest the shelf is flat, so the
/// signal passes untouched.
pub struct DeEsser {
    p: Params,
    sr: f32,
    hp: Vec<Biquad>,
    shelf: Vec<Biquad>,
    env: f32,
    att: f32,
    rel: f32,
    shelf_gr: f32,
    gr_meter: f32,
}

impl Default for DeEsser {
    fn default() -> Self {
        Self::new()
    }
}

impl DeEsser {
    pub fn new() -> Self {
        let mut d = Self {
            p: Params::new(&DEESS_PARAMS),
            sr: 48_000.0,
            hp: Vec::new(),
            shelf: Vec::new(),
            env: 0.0,
            att: 0.0,
            rel: 0.0,
            shelf_gr: 0.0,
            gr_meter: 0.0,
        };
        d.update();
        d
    }

    fn update(&mut self) {
        self.att = time_coef(0.5, self.sr);
        self.rel = time_coef(60.0, self.sr);
        let f = self.p.v(0);
        let sr = self.sr;
        for b in &mut self.hp {
            b.set(FilterType::HighPass, f, std::f32::consts::FRAC_1_SQRT_2, 0.0, sr);
        }
        self.set_shelf(self.shelf_gr);
    }

    fn set_shelf(&mut self, gr: f32) {
        self.shelf_gr = gr;
        let c = if gr < 0.01 { Coeffs::IDENTITY } else { Coeffs::design(FilterType::HighShelf, self.p.v(0) * 0.8, 0.8, -gr, self.sr) };
        for b in &mut self.shelf {
            b.coeffs = c;
        }
    }
}

impl Plugin for DeEsser {
    param_plumbing!(DEESS_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        let ch = sane_channels(channels);
        self.hp = vec![Biquad::default(); ch];
        self.shelf = vec![Biquad::default(); ch];
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.hp.iter_mut().for_each(Biquad::reset);
        self.shelf.iter_mut().for_each(Biquad::reset);
        self.env = 0.0;
        self.gr_meter = 0.0;
        self.set_shelf(0.0);
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        const CHUNK: usize = 16;
        let ch = self.hp.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let (thr, range, listen) = (self.p.v(2), self.p.v(1), self.p.on(3));
        let mut max_gr = 0.0f32;
        let mut n = 0;
        while n < frames {
            let len = CHUNK.min(frames - n);
            for i in n..n + len {
                let mut level = 0.0f32;
                for c in 0..ch {
                    let (Some(b), Some(f)) = (io.get_mut(c), self.hp.get_mut(c)) else { continue };
                    let Some(x) = b.get_mut(i) else { continue };
                    let h = f.process_sample(*x);
                    level = level.max(h.abs());
                    if listen {
                        *x = h;
                    }
                }
                self.env = ballistic(self.env, level, self.att, self.rel);
                if !self.env.is_finite() {
                    self.env = 0.0;
                }
            }
            let gr = (gain_to_db(self.env) - thr).clamp(0.0, range);
            max_gr = max_gr.max(gr);
            if (gr - self.shelf_gr).abs() > 0.05 || (gr == 0.0 && self.shelf_gr != 0.0) {
                self.set_shelf(gr);
            }
            if !listen {
                for (b, f) in io.iter_mut().zip(self.shelf.iter_mut()).take(ch) {
                    if let Some(seg) = b.get_mut(n..n + len) {
                        f.process_block(seg);
                    }
                }
            }
            n += len;
        }
        self.gr_meter = max_gr;
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr_meter
    }
}
