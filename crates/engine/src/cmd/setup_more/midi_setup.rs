//! MIDI input filtering and device enablement.

use super::super::*;
use serde_json::json;

const MIDI_FILTER_KINDS: [&str; 8] = ["notes", "pitch_bend", "mono_aftertouch", "poly_aftertouch", "program", "cc", "sysex", "realtime"];

pub(super) fn kinds(p: &Value, key: &str) -> Vec<&'static str> {
    p.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).filter_map(|k| MIDI_FILTER_KINDS.iter().copied().find(|x| *x == k)).collect())
        .unwrap_or_default()
}

pub(super) fn midi_filter(e: &mut Engine, p: &Value) -> Result<Value> {
    let s = e.session_mut();
    for k in kinds(p, "filter") {
        s.edit.set_flag(&format!("midi.filter.{k}"), true);
    }
    for k in kinds(p, "allow") {
        s.edit.set_flag(&format!("midi.filter.{k}"), false);
    }
    let filtered: Vec<&str> = MIDI_FILTER_KINDS.iter().copied().filter(|k| s.edit.flag(&format!("midi.filter.{k}"))).collect();
    Ok(json!({"filtered": filtered}))
}

fn device_key(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control() && *c != '=').take(64).collect();
    format!("midi.input.{}", n.trim())
}

pub(super) fn midi_inputs(e: &mut Engine, p: &Value) -> Result<Value> {
    let s = e.session_mut();
    for (key, on) in [("enable", true), ("disable", false)] {
        for n in p.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).take(64).collect::<Vec<_>>()).unwrap_or_default() {
            if !n.trim().is_empty() {
                s.edit.set_flag(&device_key(n), on);
            }
        }
    }
    let enabled: Vec<&str> = s.edit.flags.iter().filter_map(|f| f.strip_prefix("midi.input.")).collect();
    Ok(json!({"enabled": enabled}))
}
