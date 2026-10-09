//! QuickTime and iTunes user metadata parsing.

use crate::isobmff::bytes::Cur;
use crate::isobmff::bytes::boxes;
use crate::isobmff::bytes::find;

/// Text metadata from `udta` (QuickTime `©xxx` atoms and iTunes-style `meta/ilst`, incl. `keys`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Metadata {
    pub entries: Vec<(String, String)>,
}

impl Metadata {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    /// Title from `©nam` or the `com.apple.quicktime.title` key.
    pub fn title(&self) -> Option<&str> {
        self.get("©nam").or_else(|| self.get("com.apple.quicktime.title")).or_else(|| self.get("title"))
    }
}

pub(super) fn parse_udta(p: &[u8], md: &mut Metadata) {
    for b in boxes(p).map_while(|b| b.ok()) {
        if b.kind == b"meta" {
            parse_meta(b.payload, md);
        } else if b.kind.0[0] == 0xA9 && b.payload.len() >= 4 {
            let n = u16::from_be_bytes([b.payload[0], b.payload[1]]) as usize;
            if let Some(s) = b.payload.get(4..4 + n) {
                md.entries.push((b.kind.to_string_lossy(), String::from_utf8_lossy(s).into_owned()));
            }
        }
    }
}

pub(super) fn parse_meta(p: &[u8], md: &mut Metadata) {
    // QuickTime `meta` is a plain box; ISO `meta` is a full box.
    let body = if p.len() >= 8 && &p[4..8] == b"hdlr" { p } else { p.get(4..).unwrap_or(&[]) };
    let mut keys: Vec<String> = Vec::new();
    if let Some(k) = find(body, b"keys") {
        let mut c = Cur::new(k, "keys");
        if c.full_header().is_ok()
            && let Ok(n) = c.count(8)
        {
            for _ in 0..n {
                let Ok(sz) = c.u32() else { break };
                let Ok(_ns) = c.u32() else { break };
                let Ok(name) = c.bytes((sz as usize).saturating_sub(8)) else { break };
                keys.push(String::from_utf8_lossy(name).into_owned());
            }
        }
    }
    let Some(ilst) = find(body, b"ilst") else { return };
    for item in boxes(ilst).map_while(|b| b.ok()) {
        let idx = item.kind.as_u32() as usize;
        let key = if !keys.is_empty() && idx >= 1 && idx <= keys.len() { keys[idx - 1].clone() } else { item.kind.to_string_lossy() };
        if let Some(d) = find(item.payload, b"data")
            && d.len() >= 8
        {
            let ty = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) & 0xFF_FFFF;
            if ty == 1 || ty == 0 {
                md.entries.push((key, String::from_utf8_lossy(&d[8..]).into_owned()));
            }
        }
    }
}
