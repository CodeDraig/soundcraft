//! Event-menu operation windows: Beat Detective, Tempo Operations, Time Operations, MIDI
//! Operations and MIDI Real-Time Properties. Each is a small form over engine commands.

use crate::SoundApp;
use crate::theme::Tokens;
use serde_json::Value;

mod beat;
use beat::beat_detective;
mod tempo;
use tempo::tempo_ops;
mod time;
use time::time_ops;
mod midi;
use midi::{midi_ops, rtp};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpsState {
    pub beat_sensitivity: f32,
    pub beat_separate: bool,
    pub beat_set_tempo: bool,
    pub beat_result: String,
    pub tempo_curve: String,
    pub tempo_start: f64,
    pub tempo_end: f64,
    pub time_op: String,
    pub time_seconds: f64,
    pub meter_num: u32,
    pub meter_den: u32,
    pub midi_op: String,
    pub quant_grid: String,
    pub quant_strength: f64,
    pub quant_swing: f64,
    pub transpose: i64,
    pub velocity_add: i64,
    pub rtp_velocity: i64,
    pub rtp_transpose: i64,
    pub rtp_duration: f64,
    pub rtp_delay: i64,
    pub identify_bars: u32,
}

impl Default for OpsState {
    fn default() -> Self {
        OpsState {
            beat_sensitivity: 0.5,
            beat_separate: true,
            beat_set_tempo: false,
            beat_result: String::new(),
            tempo_curve: "linear".into(),
            tempo_start: 100.0,
            tempo_end: 120.0,
            time_op: "insert".into(),
            time_seconds: 4.0,
            meter_num: 4,
            meter_den: 4,
            midi_op: "quantize".into(),
            quant_grid: "1/16".into(),
            quant_strength: 100.0,
            quant_swing: 0.0,
            transpose: 12,
            velocity_add: 10,
            rtp_velocity: 0,
            rtp_transpose: 0,
            rtp_duration: 100.0,
            rtp_delay: 0,
            identify_bars: 1,
        }
    }
}

fn result_line(ui: &mut egui::Ui, r: &Result<Value, String>) {
    match r {
        Ok(v) => {
            let s = v.to_string();
            ui.label(egui::RichText::new(if s.len() > 160 { format!("{}…", &s[..160]) } else { s }).small().color(Tokens::DARK.counter_text));
        }
        Err(e) => {
            ui.label(egui::RichText::new(e).small().color(Tokens::DARK.rec));
        }
    }
}

pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    beat_detective(app, ctx);
    tempo_ops(app, ctx);
    time_ops(app, ctx);
    midi_ops(app, ctx);
    rtp(app, ctx);
}
