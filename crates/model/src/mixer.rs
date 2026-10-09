//! Per-track mixer state: inserts, sends, routing, fader, pan, mute/solo.

use crate::{AutomationMode, BusId};
use std::collections::BTreeMap;

mod channel_format;
pub use channel_format::{ChannelFormat, Speaker};
mod surround;
pub use surround::SurroundPan;
mod fader;
pub use fader::{fader_db_to_pos, fader_pos_to_db};

pub const INSERT_SLOTS: usize = 10;
pub const SEND_SLOTS: usize = 10;

/// An internal mix bus (Pro Tools "bus" paths).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bus {
    pub id: BusId,
    pub name: String,
    pub format: ChannelFormat,
}

/// Physical output paths (from the I/O setup).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutputPath {
    pub name: String,
    /// First hardware channel (0-based).
    pub first_channel: u16,
    pub format: ChannelFormat,
}

/// Where a signal goes or comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Route {
    #[default]
    None,
    /// The main output (hardware outputs 1-2, or the bounce source).
    Main,
    Bus(BusId),
    /// Hardware input/output by path name.
    Hardware(String),
}

/// A plugin instance on an insert slot.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Insert {
    /// Registry id from `soundcraft-dsp` (e.g. `eq7`, `comp`).
    pub plugin: String,
    pub params: BTreeMap<String, f32>,
    pub bypass: bool,
    pub active: bool,
    /// Preset name shown in the plugin window header.
    pub preset: String,
    /// Opaque plugin state (third-party plugins: CLAP state / VST3 component + controller
    /// state), standard base64. Restored into a new instance before it first processes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl Insert {
    pub fn new(plugin: impl Into<String>) -> Self {
        Insert { plugin: plugin.into(), params: BTreeMap::new(), bypass: false, active: true, preset: String::from("<factory default>"), state: None }
    }

    /// The decoded plugin state (`None` when absent or malformed).
    pub fn state_bytes(&self) -> Option<Vec<u8>> {
        self.state.as_deref().and_then(crate::b64::decode)
    }

    /// Stores a plugin state blob (base64-encoded); an empty blob clears it.
    pub fn set_state_bytes(&mut self, data: &[u8]) {
        self.state = (!data.is_empty()).then(|| crate::b64::encode(data));
    }
}

/// A send to a bus.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SendSlot {
    pub target: Route,
    pub level_db: f32,
    pub pan: f32,
    pub mute: bool,
    pub pre_fader: bool,
    pub follow_main_pan: bool,
    /// Surround position into a multichannel bus (None = follow the track / stereo pan).
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub surround: Option<SurroundPan>,
}

impl SendSlot {
    pub fn new(target: Route) -> Self {
        SendSlot { target, level_db: -144.0, pan: 0.0, mute: false, pre_fader: false, follow_main_pan: true, surround: None }
    }
}

/// The channel strip.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mixer {
    pub volume_db: f32,
    /// One panner per source channel for mono/stereo (stereo tracks: two panners, default hard L/R).
    pub pan: Vec<f32>,
    pub mute: bool,
    pub solo: bool,
    pub solo_safe: bool,
    pub record_arm: bool,
    pub input_monitor: bool,
    pub phase_invert: bool,
    pub input: Route,
    pub output: Route,
    pub inserts: Vec<Option<Insert>>,
    pub sends: Vec<Option<SendSlot>>,
    pub automation_mode: AutomationMode,
    /// Input gain / trim, dB.
    pub trim_db: f32,
    /// VCA master that controls this track, if any (track id of the VCA).
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub vca: Option<u64>,
    /// Surround panner, used when the output is multichannel (None = the stereo pan to L/R).
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub surround: Option<SurroundPan>,
}

impl Mixer {
    pub fn new(format: ChannelFormat) -> Self {
        let pan = match format {
            ChannelFormat::Mono => vec![0.0],
            ChannelFormat::Stereo => vec![-1.0, 1.0],
            _ => Vec::new(),
        };
        Mixer {
            volume_db: 0.0,
            pan,
            mute: false,
            solo: false,
            solo_safe: false,
            record_arm: false,
            input_monitor: false,
            phase_invert: false,
            input: Route::None,
            output: Route::Main,
            inserts: vec![None; INSERT_SLOTS],
            sends: vec![None; SEND_SLOTS],
            automation_mode: AutomationMode::Read,
            trim_db: 0.0,
            vca: None,
            surround: None,
        }
    }

    /// Clamp everything into range (after deserialising untrusted files).
    pub fn sanitize(&mut self) {
        self.volume_db = finite_or(self.volume_db, 0.0).clamp(-144.0, 12.0);
        for p in &mut self.pan {
            *p = finite_or(*p, 0.0).clamp(-1.0, 1.0);
        }
        self.inserts.resize(INSERT_SLOTS, None);
        self.sends.resize(SEND_SLOTS, None);
        for s in self.sends.iter_mut().flatten() {
            s.level_db = finite_or(s.level_db, -144.0).clamp(-144.0, 12.0);
            s.pan = finite_or(s.pan, 0.0).clamp(-1.0, 1.0);
            if let Some(sp) = &mut s.surround {
                sp.sanitize();
            }
        }
        if let Some(sp) = &mut self.surround {
            sp.sanitize();
        }
        self.trim_db = finite_or(self.trim_db, 0.0).clamp(-144.0, 24.0);
    }
}

impl crate::Session {
    /// The main mix format: the format of the first (main) output path, at least stereo and at
    /// most 16 channels (wider formats fall back to stereo).
    pub fn main_format(&self) -> ChannelFormat {
        match self.outputs.first().map(|o| o.format) {
            Some(f) if (2..=16).contains(&f.channels()) => f,
            _ => ChannelFormat::Stereo,
        }
    }
}

pub(crate) fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_state_round_trips_through_json() {
        let mut i = Insert::new("vst3:00");
        assert_eq!(i.state_bytes(), None);
        let blob: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        i.set_state_bytes(&blob);
        let json = serde_json::to_string(&i).unwrap();
        let back: Insert = serde_json::from_str(&json).unwrap();
        assert_eq!(back.state_bytes().unwrap(), blob);
        i.set_state_bytes(&[]);
        assert_eq!(i.state, None);
        // The current writer omits state for stateless inserts.
        let stateless = Insert::new("eq_7band");
        let json = serde_json::to_string(&stateless).unwrap();
        assert!(!json.contains("state"));
        assert_eq!(serde_json::from_str::<Insert>(&json).unwrap(), stateless);
        // A hostile state string decodes to nothing rather than failing the load.
        let mut v = serde_json::to_value(&stateless).unwrap();
        v["state"] = serde_json::json!("!!!");
        let bad: Insert = serde_json::from_value(v).unwrap();
        assert_eq!(bad.state_bytes(), None);
    }

    #[test]
    fn fader_taper_round_trips() {
        for db in [-100.0f32, -60.0, -24.0, -10.0, 0.0, 6.0, 12.0] {
            let p = fader_db_to_pos(db);
            assert!((fader_pos_to_db(p) - db).abs() < 0.05, "{db} → {p}");
        }
        assert_eq!(fader_pos_to_db(0.0), -144.0);
        assert_eq!(fader_db_to_pos(f32::NAN), 0.0);
    }

    #[test]
    fn mixer_defaults_and_sanitize() {
        let mut m = Mixer::new(ChannelFormat::Stereo);
        assert_eq!(m.pan, vec![-1.0, 1.0]);
        m.volume_db = f32::INFINITY;
        m.inserts.truncate(2);
        m.sanitize();
        assert_eq!(m.volume_db, 0.0);
        assert_eq!(m.inserts.len(), INSERT_SLOTS);
    }

    #[test]
    fn speaker_layouts_match_channel_counts_and_masks() {
        for f in ChannelFormat::ALL {
            if !f.is_ambisonic() {
                assert_eq!(f.speakers().len(), f.channels(), "{f:?}");
                // Mask bits are unique and in ascending channel order.
                let bits: Vec<u32> = f.speakers().iter().map(|s| s.wave_bit()).filter(|b| *b != 0).collect();
                assert!(bits.windows(2).all(|w| w[0] < w[1]), "{f:?} not in WAV order");
            }
        }
        assert_eq!(ChannelFormat::Surround51.channel_mask(), 0x3F);
        assert_eq!(ChannelFormat::Surround71.channel_mask(), 0x63F);
        assert_eq!(ChannelFormat::Atmos714.channel_mask(), 0x2_D63F);
        assert_eq!(ChannelFormat::Ambisonics(1).channel_mask(), 0);
    }

    #[test]
    fn surround_pan_current_format_and_sanitize() {
        let m = Mixer::new(ChannelFormat::Mono);
        assert_eq!(m.surround, None);
        // The current writer emits the surround field even when its value is null.
        let mut v = serde_json::to_value(&m).unwrap();
        v.as_object_mut().unwrap().remove("surround");
        assert!(serde_json::from_value::<Mixer>(v).is_err());
        let v = serde_json::to_value(SurroundPan { x: 5.0, ..SurroundPan::default() }).unwrap();
        let mut sp: SurroundPan = serde_json::from_value(v).unwrap();
        sp.sanitize();
        assert_eq!(sp.x, 1.0);
        assert_eq!(sp.center, 100.0);
    }

    #[test]
    fn formats() {
        assert_eq!(ChannelFormat::for_channels(6), ChannelFormat::Surround51);
        assert_eq!(ChannelFormat::from_id("stereo"), Some(ChannelFormat::Stereo));
    }
}
