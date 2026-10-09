//! Transport requests, playback and recording lifecycle.

use super::{SoundApp, feed_meter};
use soundcraft_engine::TransportRequest;
use soundcraft_time::Range;
use soundcraft_time::Samples;

impl SoundApp {
    pub fn is_playing(&self) -> bool {
        self.engine.transport.playing
    }

    /// Current transport position (playhead while playing, else the edit insertion).
    pub fn position(&self) -> Samples {
        if self.engine.transport.playing { self.engine.transport.position } else { self.engine.session().edit.selection.start }
    }

    pub(super) fn start_play(&mut self, from: Samples, end: Option<Samples>, looped: Option<Range>) {
        if let Some(p) = &self.player {
            p.update_session(self.engine.session_arc());
            p.play(from, end, looped);
        } else {
            self.sim = Some((from, end, looped));
        }
        self.engine.transport.playing = true;
        self.engine.transport.position = from;
    }

    pub(super) fn stop_play(&mut self) {
        if let Some(p) = &self.player {
            p.stop();
        }
        if self.engine.transport.recording {
            self.finish_recording();
        }
        self.sim = None;
        self.engine.transport.playing = false;
        self.engine.transport.recording = false;
        let s = self.engine.session();
        if s.edit.insertion_follows_playback {
            let pos = self.engine.transport.position;
            self.engine.session_mut().edit.selection = Range::point(pos);
        }
    }

    pub(super) fn start_recording(&mut self) {
        let armed = self.engine.session().tracks.iter().any(|t| t.mixer.record_arm);
        if !armed {
            self.ui.status = "Record-enable a track first (the red button in its header).".into();
            return;
        }
        self.ensure_input();
        if self.recorder.is_none() {
            self.ui.status = "Cannot record: no audio input device".into();
            return;
        }
        if let Some(p) = &self.player {
            p.set_recording(true);
        }
        let already_playing = self.is_playing();
        if let Some(r) = &self.recorder {
            r.arm();
        }
        self.engine.transport.recording = true;
        if already_playing {
            // Punch in on the fly at the current playhead.
            self.record_start = self.position();
            self.punch = None;
            return;
        }
        let e = self.engine.session().edit.clone();
        let sel = e.selection;
        if e.loop_record && !sel.is_empty() {
            self.record_start = sel.start;
            self.punch = None;
            self.start_play(sel.start, None, Some(sel));
        } else {
            // Record into the selection when there is one (punch), with pre/post-roll if enabled;
            // the take is trimmed back to the punch range afterwards.
            let (pre, post) = if e.pre_post_roll { (e.pre_roll, e.post_roll) } else { (0, 0) };
            let from = (sel.start - pre).max(0);
            self.record_start = from;
            self.punch = (!sel.is_empty()).then_some(sel);
            let end = (!sel.is_empty()).then_some(sel.end + post);
            self.start_play(from, end, None);
        }
    }

    /// Open the input device once (for recording and input monitoring) and feed the player.
    pub(super) fn ensure_input(&mut self) {
        if self.recorder.is_some() || self.recorder_failed || self.player.is_none() {
            return;
        }
        match soundcraft_playback::record::Recorder::open() {
            Ok(r) => {
                if let Some(p) = &self.player {
                    p.set_input(std::sync::Arc::clone(&r.monitor));
                }
                self.recorder = Some(r);
            }
            Err(e) => {
                self.recorder_failed = true;
                self.ui.status = format!("No audio input: {e}");
            }
        }
    }

    pub(super) fn finish_recording(&mut self) {
        self.engine.transport.recording = false;
        if let Some(p) = &self.player {
            p.set_recording(false);
        }
        let Some(r) = &self.recorder else { return };
        let take = r.take();
        let rate = take.sample_rate;
        let sel = self.engine.session().edit.selection;
        if self.engine.session().edit.loop_record && !sel.is_empty() {
            // Split the capture into one take per loop pass; each pass gets its own playlist.
            let pass = usize::try_from(soundcraft_time::to_samples(sel.len() as f64 * f64::from(rate) / self.engine.session().sample_rate.as_f64()))
                .unwrap_or(0)
                .max(1);
            let total = take.channels.first().map_or(0, Vec::len);
            let passes = total.div_ceil(pass).max(1);
            let mut made = 0;
            for k in 0..passes {
                let chunk: Vec<Vec<f32>> =
                    take.channels.iter().map(|c| c.get(k * pass..((k + 1) * pass).min(c.len())).map(<[f32]>::to_vec).unwrap_or_default()).collect();
                if chunk.first().is_none_or(|c| c.len() < pass / 8) {
                    continue;
                }
                if made > 0 {
                    let armed: Vec<u64> = self.engine.session().tracks.iter().filter(|t| t.mixer.record_arm).map(|t| t.id.0).collect();
                    let _ = self.engine.execute("track.playlist_new", &serde_json::json!({"tracks": armed}));
                }
                if soundcraft_engine::io::add_recording(&mut self.engine, sel.start, chunk, rate).is_ok() {
                    made += 1;
                }
            }
            self.ui.status = format!("Loop-recorded {made} take(s)");
            return;
        }
        match soundcraft_engine::io::add_recording(&mut self.engine, self.record_start, take.channels, rate) {
            Ok(ids) => {
                if let Some(p) = self.punch.take() {
                    // Keep only the punch range of each new take (merged into the Record undo step).
                    let clip_ids: Vec<u64> = ids.iter().map(|c| c.0).collect();
                    let _ = self.engine.execute_merged(
                        "edit.trim_to_fill_selection",
                        &serde_json::json!({"clips": clip_ids, "start": p.start, "end": p.end}),
                        "record",
                    );
                }
                self.ui.status = format!("Recorded {} clip(s)", ids.len());
            }
            Err(e) => self.ui.status = e.to_string(),
        }
    }

    /// Play from the selection, honouring loop playback and selection end.
    pub fn play_from_selection(&mut self) {
        let s = self.engine.session();
        let sel = s.edit.selection;
        let preroll = if s.edit.pre_post_roll { s.edit.pre_roll } else { 0 };
        let postroll = if s.edit.pre_post_roll { s.edit.post_roll } else { 0 };
        if s.edit.loop_playback && !sel.is_empty() {
            self.start_play(sel.start, None, Some(sel));
        } else if !sel.is_empty() {
            self.start_play((sel.start - preroll).max(0), Some(sel.end + postroll), None);
        } else {
            self.start_play((sel.start - preroll).max(0), None, None);
        }
    }

    pub(super) fn handle_transport(&mut self, dt: f32) {
        let reqs: Vec<TransportRequest> = std::mem::take(&mut self.engine.transport_requests);
        for r in reqs {
            match r {
                TransportRequest::Play => {
                    if !self.is_playing() {
                        self.play_from_selection();
                    }
                }
                TransportRequest::Stop | TransportRequest::Pause => self.stop_play(),
                TransportRequest::TogglePlay => {
                    if self.is_playing() {
                        self.stop_play();
                    } else {
                        self.play_from_selection();
                    }
                }
                TransportRequest::Record => {
                    if self.engine.transport.recording {
                        self.stop_play();
                    } else {
                        self.start_recording();
                    }
                }
                TransportRequest::Locate(at) => {
                    if self.is_playing() {
                        if let Some(p) = &self.player {
                            p.locate(at);
                        }
                        if let Some(sim) = &mut self.sim {
                            sim.0 = at;
                        }
                    }
                    self.engine.transport.position = at;
                }
                TransportRequest::PlaySelection => {
                    let sel = self.engine.session().edit.selection;
                    self.start_play(sel.start, Some(sel.end.max(sel.start + 1)), None);
                }
                TransportRequest::HalfSpeed => {
                    if let Some(p) = &self.player {
                        p.set_speed(0.5);
                    }
                    self.play_from_selection();
                }
                TransportRequest::Scrub(at) => self.engine.transport.position = at,
                TransportRequest::AllNotesOff => {}
            }
        }
        // Read back the position.
        if let Some(p) = &self.player {
            let playing = p.is_playing();
            let ended = self.engine.transport.playing && !playing;
            if ended {
                p.set_speed(1.0);
            }
            if self.engine.transport.playing {
                self.engine.transport.position = p.position();
            }
            let snap = p.meters();
            for (id, m) in &snap.tracks {
                let d = self.meters.entry(*id).or_default();
                feed_meter(d, m.peak, m.gain_reduction_db, dt);
            }
            let main = snap.main.peak;
            feed_meter(&mut self.main_meter, main, 0.0, dt);
            if ended {
                self.stop_play();
            }
        } else if let Some((pos, end, looped)) = &mut self.sim {
            let sr = self.engine.session().sample_rate.as_f64();
            *pos += (f64::from(dt) * sr) as i64;
            if let Some(l) = looped
                && *pos >= l.end
            {
                *pos = l.start;
            }
            let stop = end.is_some_and(|e| *pos >= e);
            self.engine.transport.position = *pos;
            if stop {
                self.stop_play();
            }
        }
        if !self.engine.transport.playing {
            for d in self.meters.values_mut() {
                feed_meter(d, [0.0, 0.0], 0.0, dt);
            }
            feed_meter(&mut self.main_meter, [0.0, 0.0], 0.0, dt);
        }
    }
}
