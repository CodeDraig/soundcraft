//! MusicXML serialization of laid-out score parts.

use super::{Part, ScoreSetup, spell, xml_escape};
use std::fmt::Write as _;

pub(super) fn note_type(units: u32) -> (&'static str, bool) {
    match units {
        16 => ("whole", false),
        12 => ("half", true),
        8 => ("half", false),
        6 => ("quarter", true),
        4 => ("quarter", false),
        3 => ("eighth", true),
        2 => ("eighth", false),
        _ => ("16th", false),
    }
}

/// MusicXML 4.0 (partwise) for `parts`.
pub fn musicxml(parts: &[Part], setup: &ScoreSetup) -> String {
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    x.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    x.push_str("<score-partwise version=\"4.0\">\n");
    let _ = writeln!(x, "  <work><work-title>{}</work-title></work>", xml_escape(&setup.title));
    x.push_str("  <identification>\n");
    if !setup.composer.is_empty() {
        let _ = writeln!(x, "    <creator type=\"composer\">{}</creator>", xml_escape(&setup.composer));
    }
    x.push_str("    <encoding><software>SoundCraft</software></encoding>\n  </identification>\n  <part-list>\n");
    for (i, p) in parts.iter().enumerate() {
        let _ = writeln!(x, "    <score-part id=\"P{}\"><part-name>{}</part-name></score-part>", i + 1, xml_escape(&p.name));
    }
    x.push_str("  </part-list>\n");
    for (i, p) in parts.iter().enumerate() {
        let _ = writeln!(x, "  <part id=\"P{}\">", i + 1);
        let mut meter = (0, 0);
        for (mi, m) in p.measures.iter().enumerate() {
            let _ = writeln!(x, "    <measure number=\"{}\">", mi + 1);
            if mi == 0 || meter != (m.numerator, m.denominator) {
                x.push_str("      <attributes>");
                if mi == 0 {
                    x.push_str("<divisions>4</divisions><key><fifths>0</fifths></key>");
                }
                let _ = write!(x, "<time><beats>{}</beats><beat-type>{}</beat-type></time>", m.numerator, m.denominator);
                if mi == 0 {
                    x.push_str(if p.bass { "<clef><sign>F</sign><line>4</line></clef>" } else { "<clef><sign>G</sign><line>2</line></clef>" });
                }
                x.push_str("</attributes>\n");
                meter = (m.numerator, m.denominator);
            }
            for it in &m.items {
                let (ty, dot) = note_type(it.units);
                if it.pitches.is_empty() {
                    let _ = writeln!(
                        x,
                        "      <note><rest/><duration>{}</duration><voice>1</voice><type>{ty}</type>{}</note>",
                        it.units,
                        if dot { "<dot/>" } else { "" }
                    );
                    continue;
                }
                for (k, pitch) in it.pitches.iter().enumerate() {
                    let (step, alter, octave) = spell(*pitch);
                    x.push_str("      <note>");
                    if k > 0 {
                        x.push_str("<chord/>");
                    }
                    let _ = write!(x, "<pitch><step>{step}</step>");
                    if alter != 0 {
                        let _ = write!(x, "<alter>{alter}</alter>");
                    }
                    let _ = write!(x, "<octave>{octave}</octave></pitch><duration>{}</duration>", it.units);
                    if it.tie_stop {
                        x.push_str("<tie type=\"stop\"/>");
                    }
                    if it.tie_start {
                        x.push_str("<tie type=\"start\"/>");
                    }
                    let _ = write!(x, "<voice>1</voice><type>{ty}</type>");
                    if dot {
                        x.push_str("<dot/>");
                    }
                    if alter != 0 {
                        x.push_str("<accidental>sharp</accidental>");
                    }
                    if it.tie_stop || it.tie_start {
                        x.push_str("<notations>");
                        if it.tie_stop {
                            x.push_str("<tied type=\"stop\"/>");
                        }
                        if it.tie_start {
                            x.push_str("<tied type=\"start\"/>");
                        }
                        x.push_str("</notations>");
                    }
                    x.push_str("</note>\n");
                }
            }
            x.push_str("    </measure>\n");
        }
        x.push_str("  </part>\n");
    }
    x.push_str("</score-partwise>\n");
    x
}
