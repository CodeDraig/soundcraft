//! Bounded multichannel delay for compensation.

/// A multichannel delay line (samples), resized only when the delay changes.
#[derive(Default)]
pub(super) struct DelayLine {
    bufs: Vec<Vec<f32>>,
    idx: usize,
    pub(super) len: usize,
}

impl DelayLine {
    pub(super) fn set(&mut self, len: usize, ch: usize) {
        let len = len.min(1 << 20);
        if len != self.len || self.bufs.len() != ch {
            self.len = len;
            self.idx = 0;
            self.bufs = vec![vec![0.0; len]; ch];
        }
    }
    pub(super) fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        if self.len == 0 {
            return;
        }
        let start = self.idx;
        for (c, ch) in io.iter_mut().enumerate() {
            let Some(ring) = self.bufs.get_mut(c) else { continue };
            let mut i = start;
            for x in ch.iter_mut().take(frames) {
                if let Some(r) = ring.get_mut(i) {
                    std::mem::swap(r, x);
                }
                i += 1;
                if i >= self.len {
                    i = 0;
                }
            }
        }
        self.idx = (start + frames) % self.len.max(1);
    }
}
