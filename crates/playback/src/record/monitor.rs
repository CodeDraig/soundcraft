//! Bounded input monitoring buffer shared by the input and output callbacks.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

/// A small interleaved FIFO from the input callback to the output callback, for input
/// monitoring. Both sides only `try_lock`; it holds at most ~0.5 s and drops the oldest frames.
#[derive(Default)]
pub struct InputRing {
    data: Mutex<std::collections::VecDeque<f32>>,
    pub channels: std::sync::atomic::AtomicUsize,
}

impl InputRing {
    pub fn new(channels: usize) -> Arc<InputRing> {
        Arc::new(InputRing {
            data: Mutex::new(std::collections::VecDeque::with_capacity(48_000 * channels.max(1))),
            channels: std::sync::atomic::AtomicUsize::new(channels.max(1)),
        })
    }

    pub fn push(&self, samples: &[f32]) {
        if let Ok(mut d) = self.data.try_lock() {
            let cap = 24_000 * self.channels.load(Ordering::Relaxed).max(1);
            d.extend(samples.iter().copied());
            while d.len() > cap {
                d.pop_front();
            }
        }
    }

    /// Fill `out` (planar) with up to `frames` frames; missing frames are silence.
    pub fn pop_into(&self, out: &mut [Vec<f32>], frames: usize) {
        let ch = self.channels.load(Ordering::Relaxed).max(1);
        for c in out.iter_mut() {
            c.iter_mut().take(frames).for_each(|x| *x = 0.0);
        }
        let Ok(mut d) = self.data.try_lock() else { return };
        let avail = (d.len() / ch).min(frames);
        for f in 0..avail {
            for c in 0..ch {
                let v = d.pop_front().unwrap_or(0.0);
                if let Some(x) = out.get_mut(c).and_then(|o| o.get_mut(f)) {
                    *x = v;
                }
            }
        }
    }
}
