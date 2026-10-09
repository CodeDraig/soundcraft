//! DC offset filtering.

use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::params::Params;
use crate::params::log;
use crate::params::param_plumbing;
use crate::util::DcBlocker;
use crate::util::frames_in;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

static DC_PARAMS: [ParamInfo; 1] = [log("cutoff", "Cutoff", 1.0, 40.0, 5.0, Unit::Hz)];

pub static DC_INFO: PluginInfo = PluginInfo {
    id: "dc_offset_removal",
    name: "DC Offset Removal",
    short_name: "DC Rmv",
    category: Category::Other,
    params: &DC_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// One-pole DC-blocking high-pass.
pub struct DcRemoval {
    p: Params,
    sr: f32,
    f: Vec<DcBlocker>,
}

impl Default for DcRemoval {
    fn default() -> Self {
        Self::new()
    }
}

impl DcRemoval {
    pub fn new() -> Self {
        Self { p: Params::new(&DC_PARAMS), sr: 48_000.0, f: Vec::new() }
    }

    fn update(&mut self) {
        let (fc, sr) = (self.p.v(0), self.sr);
        self.f.iter_mut().for_each(|f| f.set_cutoff(fc, sr));
    }
}

impl Plugin for DcRemoval {
    param_plumbing!(DC_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.f = vec![DcBlocker::new(self.p.v(0), self.sr); sane_channels(channels)];
    }

    fn reset(&mut self) {
        self.f.iter_mut().for_each(DcBlocker::reset);
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.f.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        for (b, f) in io.iter_mut().zip(self.f.iter_mut()) {
            for x in b.iter_mut().take(frames) {
                *x = f.process(if x.is_finite() { *x } else { 0.0 });
            }
        }
    }
}
