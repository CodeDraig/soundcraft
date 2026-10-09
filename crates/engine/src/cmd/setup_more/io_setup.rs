//! Bus and output path setup.

use super::super::*;
use serde_json::json;
use soundcraft_model::BusId;
use soundcraft_model::ChannelFormat;
use soundcraft_model::OutputPath;
use soundcraft_model::Route;
use soundcraft_model::Session;

fn io_state(s: &Session) -> Value {
    json!({"busses": s.busses, "outputs": s.outputs})
}

pub(super) fn io_setup(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "setup.io";
    let action = str_param(p, "action").unwrap_or("list");
    if action == "list" {
        return Ok(io_state(e.session()));
    }
    let name = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(id, "`name` required"))?.to_string();
    let new_name = str_param(p, "new_name").map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    let format = match str_param(p, "format") {
        Some(f) => ChannelFormat::from_id(f).ok_or_else(|| bad(id, format!("unknown format `{f}`")))?,
        None => ChannelFormat::Stereo,
    };
    let s = e.session_mut();
    match action {
        "create_bus" => {
            if s.bus_by_name(&name).is_some() {
                return Err(bad(id, format!("a bus named `{name}` exists")));
            }
            s.add_bus(&name, format);
        }
        "rename_bus" => {
            let to = new_name.ok_or_else(|| bad(id, "`new_name` required"))?;
            if s.bus_by_name(&to).is_some() {
                return Err(bad(id, format!("a bus named `{to}` exists")));
            }
            let b = s.busses.iter_mut().find(|b| b.name.eq_ignore_ascii_case(&name)).ok_or_else(|| bad(id, format!("no bus `{name}`")))?;
            b.name = to;
        }
        "delete_bus" => {
            let bid: BusId = s.bus_by_name(&name).map(|b| b.id).ok_or_else(|| bad(id, format!("no bus `{name}`")))?;
            s.busses.retain(|b| b.id != bid);
            for t in &mut s.tracks {
                if t.mixer.input == Route::Bus(bid) {
                    t.mixer.input = Route::None;
                }
                if t.mixer.output == Route::Bus(bid) {
                    t.mixer.output = Route::None;
                }
                for slot in &mut t.mixer.sends {
                    if slot.as_ref().is_some_and(|x| x.target == Route::Bus(bid)) {
                        *slot = None;
                    }
                }
            }
        }
        "create_output" => {
            if s.outputs.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
                return Err(bad(id, format!("an output named `{name}` exists")));
            }
            let first = i64_or(p, "first_channel", 0).clamp(0, 1023) as u16;
            s.outputs.push(OutputPath { name, first_channel: first, format });
        }
        "rename_output" => {
            let to = new_name.ok_or_else(|| bad(id, "`new_name` required"))?;
            let o = s.outputs.iter_mut().find(|o| o.name.eq_ignore_ascii_case(&name)).ok_or_else(|| bad(id, format!("no output `{name}`")))?;
            let old = std::mem::replace(&mut o.name, to.clone());
            for t in &mut s.tracks {
                if t.mixer.output == Route::Hardware(old.clone()) {
                    t.mixer.output = Route::Hardware(to.clone());
                }
            }
        }
        "delete_output" => {
            let before = s.outputs.len();
            s.outputs.retain(|o| !o.name.eq_ignore_ascii_case(&name));
            if s.outputs.len() == before {
                return Err(bad(id, format!("no output `{name}`")));
            }
        }
        other => return Err(bad(id, format!("unknown action `{other}`"))),
    }
    Ok(io_state(s))
}
