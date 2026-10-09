//! Track group membership and enablement.

use super::super::*;
use serde_json::json;
use soundcraft_model::Group;
use soundcraft_model::GroupId;

pub(super) fn group(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "track.group", p)?;
    let s = e.session_mut();
    let id = GroupId(s.alloc());
    let letter = (b'a'..=b'z').map(char::from).find(|c| !s.groups.iter().any(|g| g.letter == *c)).unwrap_or('z');
    let name = str_param(p, "name").map_or_else(|| format!("Group {}", s.groups.len() + 1), str::to_string);
    let color = s.next_track_color();
    s.groups.push(Group {
        id,
        name,
        letter,
        members: tracks,
        edit: bool_or(p, "edit", true),
        mix: bool_or(p, "mix", true),
        active: true,
        color,
        attributes: vec!["volume".into(), "mute".into(), "solo".into(), "record".into()],
    });
    Ok(json!({"group": id}))
}

fn find_group(e: &Engine, p: &Value, cmd: &str) -> Result<GroupId> {
    let s = e.session();
    let g = match (p.get("group").and_then(Value::as_u64), str_param(p, "group")) {
        (Some(n), _) => s.groups.iter().find(|g| g.id.0 == n),
        (_, Some(name)) => s.groups.iter().find(|g| g.name == name || g.letter.to_string() == name),
        _ => None,
    };
    g.map(|g| g.id).ok_or_else(|| bad(cmd, "unknown `group`"))
}

pub(super) fn delete_group(e: &mut Engine, p: &Value) -> Result<Value> {
    let g = find_group(e, p, "track.ungroup")?;
    e.session_mut().groups.retain(|x| x.id != g);
    Ok(json!({}))
}

pub(super) fn toggle_group(e: &mut Engine, p: &Value) -> Result<Value> {
    let g = find_group(e, p, "track.group_toggle")?;
    let s = e.session_mut();
    let mut active = false;
    if let Some(gr) = s.groups.iter_mut().find(|x| x.id == g) {
        gr.active = !gr.active;
        active = gr.active;
    }
    Ok(json!({"active": active}))
}
