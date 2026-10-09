//! SVG score engraving.

use super::{Part, ScoreSetup, note_type, spell, xml_escape};
use std::fmt::Write as _;

/// Staff position of `pitch` in half-spaces above the bottom line (treble E4 / bass G2).
pub(super) fn staff_step(pitch: u8, bass: bool) -> i32 {
    let (step, _, octave) = spell(pitch);
    let idx = "CDEFGAB".find(step).map_or(0, |i| i as i32);
    let diatonic = octave * 7 + idx;
    diatonic - if bass { 2 * 7 + 4 } else { 4 * 7 + 2 }
}

/// An engraved score as a printable SVG page (A4 portrait width, as tall as needed).
pub fn svg(parts: &[Part], setup: &ScoreSetup) -> String {
    const W: f64 = 794.0;
    const MARGIN: f64 = 48.0;
    const SP: f64 = 7.0; // staff space
    const STAFF_GAP: f64 = 70.0;
    const SYSTEM_GAP: f64 = 40.0;
    let n_measures = parts.iter().map(|p| p.measures.len()).max().unwrap_or(0);
    let per = setup.bars_per_system.max(1) as usize;
    let systems = n_measures.div_ceil(per).max(1);
    let label_w = if setup.show_track_names { 70.0 } else { 0.0 };
    let staff_x0 = MARGIN + label_w + 34.0;
    let staff_x1 = W - MARGIN;
    let sys_h = parts.len().max(1) as f64 * STAFF_GAP + SYSTEM_GAP;
    let top = 110.0;
    let h = top + systems as f64 * sys_h + MARGIN;
    let mut o = String::new();
    let _ =
        writeln!(o, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W}\" height=\"{h:.0}\" viewBox=\"0 0 {W} {h:.0}\" font-family=\"serif\">");
    let _ = writeln!(o, "<rect width=\"100%\" height=\"100%\" fill=\"white\"/>");
    let _ = writeln!(o, "<text x=\"{}\" y=\"56\" font-size=\"24\" text-anchor=\"middle\">{}</text>", W / 2.0, xml_escape(&setup.title));
    if !setup.composer.is_empty() {
        let _ = writeln!(o, "<text x=\"{}\" y=\"84\" font-size=\"13\" text-anchor=\"end\">{}</text>", W - MARGIN, xml_escape(&setup.composer));
    }
    let bar_w = (staff_x1 - staff_x0) / per as f64;
    for sys in 0..systems {
        let y_sys = top + sys as f64 * sys_h;
        for (pi, p) in parts.iter().enumerate() {
            let y0 = y_sys + pi as f64 * STAFF_GAP; // top line
            let bottom = y0 + 4.0 * SP;
            if setup.show_track_names && sys == 0 {
                let _ = writeln!(o, "<text x=\"{MARGIN}\" y=\"{:.1}\" font-size=\"11\">{}</text>", y0 + 2.0 * SP + 4.0, xml_escape(&p.name));
            }
            for l in 0..5 {
                let y = y0 + f64::from(l) * SP;
                let _ = writeln!(
                    o,
                    "<line x1=\"{:.1}\" y1=\"{y:.1}\" x2=\"{staff_x1:.1}\" y2=\"{y:.1}\" stroke=\"black\" stroke-width=\"0.8\"/>",
                    staff_x0 - 30.0
                );
            }
            let clef = if p.bass { "𝄢" } else { "𝄞" };
            let _ = writeln!(
                o,
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{}\">{clef}</text>",
                staff_x0 - 29.0,
                bottom + if p.bass { -SP } else { SP * 0.6 },
                if p.bass { 26 } else { 36 }
            );
            for k in 0..per {
                let mi = sys * per + k;
                let x0 = staff_x0 + k as f64 * bar_w;
                let _ = writeln!(
                    o,
                    "<line x1=\"{:.1}\" y1=\"{y0:.1}\" x2=\"{:.1}\" y2=\"{bottom:.1}\" stroke=\"black\" stroke-width=\"1\"/>",
                    x0 + bar_w,
                    x0 + bar_w
                );
                let Some(m) = p.measures.get(mi) else { continue };
                if pi == 0 {
                    let _ = writeln!(o, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"9\">{}</text>", x0 + 2.0, y0 - 8.0, mi + 1);
                }
                let total: u32 = m.items.iter().map(|i| i.units).sum::<u32>().max(1);
                let mut acc = 0u32;
                for it in &m.items {
                    let x = x0 + 10.0 + (bar_w - 16.0) * f64::from(acc) / f64::from(total);
                    acc += it.units;
                    let (_, dot) = note_type(it.units);
                    if it.pitches.is_empty() {
                        rest(&mut o, x, y0, SP, it.units);
                        if dot {
                            let _ = writeln!(o, "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.3\"/>", x + 7.0, y0 + 1.5 * SP);
                        }
                        continue;
                    }
                    let steps: Vec<i32> = it.pitches.iter().map(|p2| staff_step(*p2, p.bass)).collect();
                    let filled = it.units < 8;
                    for (pitch, st) in it.pitches.iter().zip(&steps) {
                        let y = bottom - f64::from(*st) * SP / 2.0;
                        // Ledger lines.
                        let mut l = if *st < 0 { -2 } else { 10 };
                        while (*st < 0 && l >= *st) || (*st > 8 && l <= *st) {
                            let ly = bottom - f64::from(l) * SP / 2.0;
                            let _ = writeln!(
                                o,
                                "<line x1=\"{:.1}\" y1=\"{ly:.1}\" x2=\"{:.1}\" y2=\"{ly:.1}\" stroke=\"black\" stroke-width=\"0.8\"/>",
                                x - 7.0,
                                x + 7.0
                            );
                            l += if *st < 0 { -2 } else { 2 };
                        }
                        let fill = if filled { "black" } else { "white" };
                        let _ = writeln!(
                            o,
                            "<ellipse cx=\"{x:.1}\" cy=\"{y:.1}\" rx=\"4.6\" ry=\"3.3\" transform=\"rotate(-20 {x:.1} {y:.1})\" fill=\"{fill}\" stroke=\"black\" stroke-width=\"1.1\"/>"
                        );
                        if spell(*pitch).1 != 0 {
                            let _ = writeln!(o, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\">♯</text>", x - 14.0, y + 4.0);
                        }
                        if dot {
                            let _ = writeln!(o, "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.3\"/>", x + 8.0, y - 1.5);
                        }
                    }
                    if it.units < 16 {
                        let (lo, hi) = (steps.iter().copied().min().unwrap_or(4), steps.iter().copied().max().unwrap_or(4));
                        let up = (lo + hi) < 8;
                        let (sx, y_from, y_to) = if up {
                            (x + 4.3, bottom - f64::from(lo) * SP / 2.0, bottom - f64::from(hi) * SP / 2.0 - 3.5 * SP)
                        } else {
                            (x - 4.3, bottom - f64::from(hi) * SP / 2.0, bottom - f64::from(lo) * SP / 2.0 + 3.5 * SP)
                        };
                        let _ = writeln!(
                            o,
                            "<line x1=\"{sx:.1}\" y1=\"{y_from:.1}\" x2=\"{sx:.1}\" y2=\"{y_to:.1}\" stroke=\"black\" stroke-width=\"1\"/>"
                        );
                        let flags = match it.units {
                            1 => 2,
                            2 | 3 => 1,
                            _ => 0,
                        };
                        for f in 0..flags {
                            let fy = y_to + f64::from(f) * if up { 5.0 } else { -5.0 };
                            let dy = if up { 9.0 } else { -9.0 };
                            let _ = writeln!(
                                o,
                                "<path d=\"M{sx:.1} {fy:.1} q 7 {:.1} 5 {dy:.1}\" fill=\"none\" stroke=\"black\" stroke-width=\"1.4\"/>",
                                dy * 0.5
                            );
                        }
                    }
                    if it.tie_start {
                        let y = bottom - f64::from(steps[0]) * SP / 2.0 + 6.0;
                        let _ = writeln!(o, "<path d=\"M{:.1} {y:.1} q 8 5 16 0\" fill=\"none\" stroke=\"black\" stroke-width=\"0.9\"/>", x + 3.0);
                    }
                }
            }
        }
        // System bracket.
        let y_last = y_sys + (parts.len().max(1) - 1) as f64 * STAFF_GAP + 4.0 * SP;
        let _ = writeln!(
            o,
            "<line x1=\"{:.1}\" y1=\"{y_sys:.1}\" x2=\"{:.1}\" y2=\"{y_last:.1}\" stroke=\"black\" stroke-width=\"1.5\"/>",
            staff_x0 - 30.0,
            staff_x0 - 30.0
        );
    }
    o.push_str("</svg>\n");
    o
}

pub(super) fn rest(o: &mut String, x: f64, y0: f64, sp: f64, units: u32) {
    let mid = y0 + 2.0 * sp;
    match units {
        16 => {
            let _ = writeln!(o, "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"9\" height=\"{:.1}\"/>", x - 4.5, y0 + sp, sp / 2.0);
        }
        8 | 12 => {
            let _ = writeln!(o, "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"9\" height=\"{:.1}\"/>", x - 4.5, mid - sp / 2.0, sp / 2.0);
        }
        4 | 6 => {
            let _ = writeln!(
                o,
                "<path d=\"M{:.1} {:.1} l 4 5 l -4 4 l 4 5 q -6 -2 -2 5\" fill=\"none\" stroke=\"black\" stroke-width=\"1.8\"/>",
                x - 2.0,
                mid - 9.0
            );
        }
        _ => {
            let _ = writeln!(o, "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2\"/>", x - 2.0, mid - 3.0);
            let _ = writeln!(
                o,
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"black\" stroke-width=\"1.2\"/>",
                x + 2.5,
                mid - 4.0,
                x - 1.0,
                mid + 7.0
            );
            if units == 1 {
                let _ = writeln!(o, "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2\"/>", x - 3.5, mid + 2.0);
            }
        }
    }
}
