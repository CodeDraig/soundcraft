//! MIDI note duration and legato transformations.

use super::round_i64;
use crate::Note;

/// Duration change for [`change_duration`]. Lengths are always kept >= 1 tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DurationOp {
    Set(i64),
    Add(i64),
    /// Multiply (non-finite factors leave lengths unchanged).
    Scale(f32),
    /// Each note extends to `gap` ticks before the next later note start (any pitch). The last
    /// note(s) keep their length.
    Legato {
        gap: i64,
    },
    /// Shortens notes so they end no later than the next note of the same pitch and channel.
    RemoveOverlap,
}

/// Changes note lengths. See [`DurationOp`].
pub fn change_duration(notes: &mut [Note], op: DurationOp) {
    match op {
        DurationOp::Set(l) => notes.iter_mut().for_each(|n| n.length = l.max(1)),
        DurationOp::Add(d) => notes.iter_mut().for_each(|n| n.length = n.length.saturating_add(d).max(1)),
        DurationOp::Scale(f) => {
            if f.is_finite() {
                notes.iter_mut().for_each(|n| n.length = round_i64(n.length as f64 * f64::from(f)).max(1));
            }
        }
        DurationOp::Legato { gap } => {
            let mut starts: Vec<i64> = notes.iter().map(|n| n.start).collect();
            starts.sort_unstable();
            starts.dedup();
            for n in notes.iter_mut() {
                let idx = starts.partition_point(|&s| s <= n.start);
                if let Some(&next) = starts.get(idx) {
                    n.length = next.saturating_sub(gap).saturating_sub(n.start).max(1);
                }
            }
        }
        DurationOp::RemoveOverlap => {
            let mut order: Vec<usize> = (0..notes.len()).collect();
            order.sort_by_key(|&i| notes.get(i).map(|n| (n.channel, n.pitch, n.start)));
            for w in order.windows(2) {
                let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else { continue };
                let Some(&nb) = notes.get(b) else { continue };
                if let Some(na) = notes.get_mut(a)
                    && na.channel == nb.channel
                    && na.pitch == nb.pitch
                    && na.end() > nb.start
                {
                    na.length = nb.start.saturating_sub(na.start).max(1);
                }
            }
        }
    }
}
