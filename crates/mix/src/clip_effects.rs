//! Per-clip equalizer and compressor lifecycle.

use soundcraft_dsp::Plugin;
use soundcraft_dsp::db_to_gain;
use soundcraft_model::Session;

/// Clip Effects: an EQ and a compressor per clip, configured from `edit.values`
/// (`clip_fx.<clip>.eq.<param>`, `clip_fx.<clip>.comp.<param>`, `clip_fx.<clip>.gain`).
pub(super) struct ClipFx {
    eq: Option<Box<dyn Plugin>>,
    comp: Option<Box<dyn Plugin>>,
    gain_db: f32,
    sr: f32,
    ch: usize,
}

/// Clip-effect parameters of a clip, or None when it has none or they are bypassed.
pub(super) fn clip_fx_params(s: &Session, clip: u64) -> Option<Vec<(String, f32)>> {
    if s.edit.values.is_empty() {
        return None;
    }
    let pre = format!("clip_fx.{clip}.");
    let v: Vec<(String, f32)> = s
        .edit
        .values
        .range(pre.clone()..)
        .take_while(|(k, _)| k.starts_with(&pre))
        .map(|(k, v)| (k.get(pre.len()..).unwrap_or("").to_string(), *v as f32))
        .collect();
    if v.is_empty() || s.edit.flag(&format!("clip_fx.bypass.{clip}")) { None } else { Some(v) }
}

impl ClipFx {
    pub(super) fn new(sr: f32, ch: usize) -> ClipFx {
        ClipFx { eq: None, comp: None, gain_db: 0.0, sr, ch }
    }
    fn make(&self, id: &str) -> Option<Box<dyn Plugin>> {
        soundcraft_dsp::create(id).map(|mut p| {
            p.prepare(self.sr, 16_384, self.ch.max(1));
            p
        })
    }
    /// Modules exist only when one of their parameters is set.
    pub(super) fn configure(&mut self, params: &[(String, f32)]) {
        self.gain_db = 0.0;
        let has_eq = params.iter().any(|(k, _)| k.starts_with("eq."));
        let has_comp = params.iter().any(|(k, _)| k.starts_with("comp."));
        if has_eq && self.eq.is_none() {
            self.eq = self.make("eq_7band");
        } else if !has_eq {
            self.eq = None;
        }
        if has_comp && self.comp.is_none() {
            self.comp = self.make("compressor");
        } else if !has_comp {
            self.comp = None;
        }
        for (k, v) in params {
            if let Some(p) = k.strip_prefix("eq.") {
                if let Some(eq) = &mut self.eq
                    && eq.param(p) != Some(*v)
                {
                    eq.set_param(p, *v);
                }
            } else if let Some(p) = k.strip_prefix("comp.") {
                if let Some(c) = &mut self.comp
                    && c.param(p) != Some(*v)
                {
                    c.set_param(p, *v);
                }
            } else if k == "gain" {
                self.gain_db = *v;
            }
        }
    }
    pub(super) fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        if let Some(eq) = &mut self.eq {
            eq.process(io, frames);
        }
        if let Some(c) = &mut self.comp {
            c.process(io, frames);
        }
        if self.gain_db.abs() > 1e-6 {
            let g = db_to_gain(self.gain_db);
            for ch in io.iter_mut() {
                ch.iter_mut().take(frames).for_each(|x| *x *= g);
            }
        }
    }
}
