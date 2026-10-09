//! Fractional delay-line storage and interpolation.

/// Power-of-two circular delay line with fractional reads.
#[derive(Debug, Clone, Default)]
pub(crate) struct DelayLine {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
}

impl DelayLine {
    /// Allocates room for at least `max_delay` samples of delay.
    pub(crate) fn allocate(&mut self, max_delay: usize) {
        let len = max_delay.saturating_add(4).clamp(4, 1 << 26).next_power_of_two();
        self.buf = vec![0.0; len];
        self.mask = len - 1;
        self.pos = 0;
    }

    pub(crate) fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
    }

    /// Largest delay (in samples) that `read` supports.
    pub(crate) fn max_delay(&self) -> f32 {
        self.buf.len().saturating_sub(3) as f32
    }

    #[inline]
    pub(crate) fn push(&mut self, x: f32) {
        if let Some(s) = self.buf.get_mut(self.pos) {
            *s = x;
        }
        self.pos = (self.pos + 1) & self.mask;
    }

    /// Reads the sample written `d` pushes ago (`d = 1` is the last written sample).
    #[inline]
    pub(crate) fn tap(&self, d: usize) -> f32 {
        let idx = self.pos.wrapping_sub(d) & self.mask;
        self.buf.get(idx).copied().unwrap_or(0.0)
    }

    /// Linear-interpolated read `d` samples back (clamped to 1..=max_delay).
    #[inline]
    pub(crate) fn read(&self, d: f32) -> f32 {
        let d = if d.is_finite() { d.clamp(1.0, self.max_delay().max(1.0)) } else { 1.0 };
        let i = d as usize;
        let f = d - i as f32;
        let a = self.tap(i);
        let b = self.tap(i + 1);
        a + (b - a) * f
    }

    /// Cubic (Hermite) interpolated read, for modulated delays.
    #[inline]
    pub(crate) fn read_cubic(&self, d: f32) -> f32 {
        let d = if d.is_finite() { d.clamp(2.0, (self.max_delay() - 1.0).max(2.0)) } else { 2.0 };
        let i = d as usize;
        let f = d - i as f32;
        let xm1 = self.tap(i - 1);
        let x0 = self.tap(i);
        let x1 = self.tap(i + 1);
        let x2 = self.tap(i + 2);
        let c1 = 0.5 * (x1 - xm1);
        let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
        let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
        ((c3 * f + c2) * f + c1) * f + x0
    }
}
