//! Modulated delay-line chorus and flanger.

use super::channel_phase;
use crate::Category;
use crate::ParamInfo;
use crate::Plugin;
use crate::PluginInfo;
use crate::Unit;
use crate::params::Params;
use crate::params::lin;
use crate::params::log;
use crate::util::DelayLine;
use crate::util::Lfo;
use crate::util::Smoother;
use crate::util::flush;
use crate::util::frames_in;
use crate::util::ms_to_samples;
use crate::util::sane_channels;
use crate::util::sane_sr;

// ---------------------------------------------------------------------------------------------

static CHORUS_PARAMS: [ParamInfo; 6] = [
    log("rate", "Rate", 0.05, 5.0, 0.8, Unit::Hz),
    lin("depth", "Depth", 0.0, 100.0, 50.0, Unit::Percent),
    lin("delay", "Delay", 5.0, 30.0, 12.0, Unit::Ms),
    lin("feedback", "Feedback", 0.0, 90.0, 0.0, Unit::Percent),
    lin("mix", "Mix", 0.0, 100.0, 50.0, Unit::Percent),
    lin("spread", "Spread", 0.0, 100.0, 100.0, Unit::Percent),
];

pub static CHORUS_INFO: PluginInfo = PluginInfo {
    id: "chorus",
    name: "Chorus",
    short_name: "Chorus",
    category: Category::Modulation,
    params: &CHORUS_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

static FLANGER_PARAMS: [ParamInfo; 5] = [
    log("rate", "Rate", 0.02, 5.0, 0.25, Unit::Hz),
    lin("depth", "Depth", 0.0, 100.0, 70.0, Unit::Percent),
    lin("delay", "Delay", 0.1, 10.0, 2.0, Unit::Ms),
    lin("feedback", "Feedback", -95.0, 95.0, 50.0, Unit::Percent),
    lin("mix", "Mix", 0.0, 100.0, 50.0, Unit::Percent),
];

pub static FLANGER_INFO: PluginInfo = PluginInfo {
    id: "flanger",
    name: "Flanger",
    short_name: "Flanger",
    category: Category::Modulation,
    params: &FLANGER_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

#[derive(Clone, Copy, PartialEq)]
enum ModKind {
    Chorus,
    Flanger,
}

/// Modulated short delay used by both Chorus and Flanger.
pub struct ModFx {
    kind: ModKind,
    p: Params,
    sr: f32,
    lines: Vec<DelayLine>,
    fbs: Vec<f32>,
    lfo: Lfo,
    delay: Smoother,
    depth: Smoother,
    fb: Smoother,
    mix: Smoother,
}

impl ModFx {
    pub fn chorus() -> Self {
        Self::build(ModKind::Chorus)
    }

    pub fn flanger() -> Self {
        Self::build(ModKind::Flanger)
    }

    fn build(kind: ModKind) -> Self {
        let info = if kind == ModKind::Chorus { &CHORUS_INFO } else { &FLANGER_INFO };
        let mut m = Self {
            kind,
            p: Params::new(info.params),
            sr: 48_000.0,
            lines: Vec::new(),
            fbs: Vec::new(),
            lfo: Lfo::new(),
            delay: Smoother::new(0.0),
            depth: Smoother::new(0.0),
            fb: Smoother::new(0.0),
            mix: Smoother::new(0.0),
        };
        m.update();
        m
    }

    fn update(&mut self) {
        self.delay.set(self.p.v(2));
        self.depth.set(self.p.v(1) / 100.0);
        self.fb.set(self.p.v(3) / 100.0);
        self.mix.set(self.p.v(4) / 100.0);
    }
}

impl Plugin for ModFx {
    fn info(&self) -> &'static PluginInfo {
        if self.kind == ModKind::Chorus { &CHORUS_INFO } else { &FLANGER_INFO }
    }

    fn set_param(&mut self, id: &str, value: f32) -> bool {
        if self.p.set(id, value).is_some() {
            self.update();
            true
        } else {
            false
        }
    }

    fn param(&self, id: &str) -> Option<f32> {
        self.p.get(id)
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        let max = ms_to_samples(45.0, self.sr) as usize + 8;
        self.lines = (0..sane_channels(channels))
            .map(|_| {
                let mut d = DelayLine::default();
                d.allocate(max);
                d
            })
            .collect();
        self.fbs = vec![0.0; self.lines.len()];
        for s in [&mut self.delay, &mut self.depth, &mut self.fb, &mut self.mix] {
            s.set_time(30.0, self.sr);
        }
        self.update();
        self.reset();
    }

    fn reset(&mut self) {
        self.lines.iter_mut().for_each(DelayLine::clear);
        self.fbs.iter_mut().for_each(|v| *v = 0.0);
        self.lfo.reset();
        for s in [&mut self.delay, &mut self.depth, &mut self.fb, &mut self.mix] {
            s.snap();
        }
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.lines.len().min(io.len());
        let frames = frames_in(io, frames, ch);
        let inc = self.p.v(0) / self.sr;
        let spread = if self.kind == ModKind::Chorus { 0.25 * self.p.v(5) / 100.0 } else { 0.25 };
        for n in 0..frames {
            let phase = self.lfo.tick(inc);
            let base = self.delay.next();
            let depth = self.depth.next();
            let fb = self.fb.next();
            let m = self.mix.next();
            for c in 0..ch {
                let (Some(buf), Some(line), Some(fbs)) = (io.get_mut(c), self.lines.get_mut(c), self.fbs.get_mut(c)) else { continue };
                let Some(x) = buf.get_mut(n) else { continue };
                let dry = if x.is_finite() { *x } else { 0.0 };
                let s = Lfo::sin_at(channel_phase(phase, c, spread));
                let ms = match self.kind {
                    // Up to ±6 ms of sweep around the base delay.
                    ModKind::Chorus => base + depth * 6.0 * 0.5 * (1.0 + s),
                    // Sweep from (1 - depth)·delay up to delay.
                    ModKind::Flanger => (base * (1.0 - depth * 0.5 * (1.0 - s))).max(0.05),
                };
                line.push(flush(dry + fb * *fbs));
                let y = line.read_cubic(ms_to_samples(ms, self.sr).max(2.0));
                *fbs = flush(y);
                *x = dry * (1.0 - m) + y * m;
            }
        }
    }

    fn tail_samples(&self) -> usize {
        ms_to_samples(200.0, self.sr) as usize
    }
}
