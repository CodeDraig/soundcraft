//! Signal polarity inversion.

use crate::Category;
use crate::Plugin;
use crate::PluginInfo;
use crate::util::frames_in;
use crate::util::sane_channels;

// ---------------------------------------------------------------------------------------------

pub static INVERT_INFO: PluginInfo =
    PluginInfo { id: "invert", name: "Invert", short_name: "Invert", category: Category::Other, params: &[], is_instrument: false, audiosuite: true };

/// Polarity inversion.
#[derive(Default)]
pub struct Invert {
    ch: usize,
}

impl Plugin for Invert {
    fn info(&self) -> &'static PluginInfo {
        &INVERT_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, channels: usize) {
        self.ch = sane_channels(channels);
    }

    fn reset(&mut self) {}

    fn set_param(&mut self, _id: &str, _value: f32) -> bool {
        false
    }

    fn param(&self, _id: &str) -> Option<f32> {
        None
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.ch.min(io.len());
        let frames = frames_in(io, frames, ch);
        io.iter_mut().take(ch).for_each(|b| b.iter_mut().take(frames).for_each(|x| *x = -*x));
    }
}
