//! Writing fader automation during playback gestures.

use super::{Gesture, SoundApp};
use soundcraft_model::TrackId;

impl SoundApp {
    /// While playing, record volume automation from fader positions for tracks in Write mode, and
    /// for Touch/Latch tracks whose fader is (or, for Latch, was) being moved. One undo step per pass.
    pub(super) fn write_automation(&mut self, ctx: &egui::Context) {
        use soundcraft_model::AutomationMode as M;
        if !self.is_playing() {
            if !self.touched.is_empty() {
                self.touched.clear();
                self.engine.end_merge();
            }
            self.last_write_at = i64::MIN;
            return;
        }
        let pos = self.position();
        let step = self.engine.session().sample_rate.samples(0.02);
        if (pos - self.last_write_at).abs() < step {
            return;
        }
        self.last_write_at = pos;
        let dragging = ctx.input(|i| i.pointer.any_down());
        let fader_track = match &self.gesture {
            Some(Gesture::Fader { track, .. }) => Some(*track),
            _ => None,
        };
        let writes: Vec<(TrackId, f32)> = self
            .engine
            .session()
            .tracks
            .iter()
            .filter_map(|t| {
                let m = t.mixer.automation_mode;
                let touched_now = dragging && fader_track == Some(t.id);
                let write = match m {
                    M::Write => true,
                    M::Touch => touched_now,
                    M::Latch | M::TouchLatch => touched_now || self.touched.contains(&t.id),
                    _ => false,
                };
                write.then_some((t.id, t.mixer.volume_db))
            })
            .collect();
        for (t, v) in writes {
            self.touched.insert(t);
            let _ = self.engine.execute_merged(
                "automation.set_point",
                &serde_json::json!({"track": t.0, "param": "volume", "at": pos, "value": v}),
                "automation_pass",
            );
        }
    }
}
