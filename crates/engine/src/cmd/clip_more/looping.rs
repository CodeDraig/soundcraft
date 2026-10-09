//! Regrouping and identifying repeated clip material.

use super::super::*;
use serde_json::json;
use soundcraft_model::Clip;
use soundcraft_model::ClipContent;
use soundcraft_model::ClipId;
use soundcraft_model::Session;
use soundcraft_model::TrackId;

pub(super) fn regroup(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    if ids.len() < 2 {
        return Err(bad("clip.regroup", "select two or more clips"));
    }
    let s = e.session_mut();
    let mut counts: std::collections::BTreeMap<u64, usize> = Default::default();
    for id in &ids {
        if let Some(g) = s.find_clip(*id).and_then(|(_, c)| c.group) {
            *counts.entry(g).or_default() += 1;
        }
    }
    let g = counts.iter().max_by_key(|(_, n)| **n).map(|(g, _)| *g).unwrap_or_else(|| s.alloc());
    let mut n = 0;
    for id in &ids {
        if let Some(c) = s.find_clip_mut(*id) {
            c.group = Some(g);
            n += 1;
        }
    }
    Ok(json!({"group": g, "clips": n}))
}

fn same_material(a: &Clip, b: &Clip) -> bool {
    a.length == b.length
        && match (&a.content, &b.content) {
            (ClipContent::Audio { source: s1, offset: o1 }, ClipContent::Audio { source: s2, offset: o2 }) => s1 == s2 && o1 == o2,
            (ClipContent::Midi { sequence: q1 }, ClipContent::Midi { sequence: q2 }) => q1 == q2,
            (ClipContent::Video { source: s1, offset: o1 }, ClipContent::Video { source: s2, offset: o2 }) => s1 == s2 && o1 == o2,
            _ => false,
        }
}

/// The loop a clip belongs to: back-to-back copies of the same material. Origin first.
fn loop_chain(s: &Session, t: TrackId, id: ClipId) -> Vec<ClipId> {
    let Some(tr) = s.track(t) else { return Vec::new() };
    let clips = tr.clips();
    let Some(mut cur) = clips.iter().find(|c| c.id == id) else { return Vec::new() };
    let mut guard = 0;
    while let Some(prev) = clips.iter().find(|x| x.id != cur.id && x.end() == cur.start && same_material(x, cur)) {
        cur = prev;
        guard += 1;
        if guard > clips.len() {
            break;
        }
    }
    let mut chain = vec![cur.id];
    while let Some(next) = clips.iter().find(|x| !chain.contains(&x.id) && x.start == cur.end() && same_material(x, cur)) {
        chain.push(next.id);
        cur = next;
    }
    chain
}

pub(super) fn unloop(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "clip.unloop";
    let remove = match str_param(p, "mode") {
        None | Some("remove") => true,
        Some("flatten") => false,
        Some(m) => return Err(bad(id, format!("unknown mode `{m}`"))),
    };
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut done: Vec<ClipId> = Vec::new();
    let mut gone: Vec<ClipId> = Vec::new();
    let mut affected = 0;
    for cid in ids {
        if done.contains(&cid) {
            continue;
        }
        let Some(t) = s.find_clip(cid).map(|(t, _)| t) else { continue };
        let chain = loop_chain(s, t, cid);
        done.extend(chain.iter().copied());
        if chain.len() < 2 {
            continue;
        }
        let copies: Vec<ClipId> = chain.iter().skip(1).copied().collect();
        if let Some(pl) = s.track_mut(t).and_then(|tr| tr.playlist_mut()) {
            if remove {
                pl.clips.retain(|c| !copies.contains(&c.id));
                gone.extend(copies.iter().copied());
            } else {
                for c in pl.clips.iter_mut().filter(|c| chain.contains(&c.id)) {
                    c.group = None;
                }
            }
        }
        affected += copies.len();
    }
    if affected == 0 {
        return Err(bad(id, "the selection is not a looped clip"));
    }
    s.edit.selected_clips.retain(|c| !gone.contains(c));
    Ok(json!({"copies": affected, "removed": remove}))
}
