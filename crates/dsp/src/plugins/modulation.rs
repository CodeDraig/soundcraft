//! Delay and modulation effects: Mod Delay, Chorus, Flanger, Phaser.

mod delay;
pub use delay::{DELAY_INFO, ModDelay};
mod chorus_flanger;
pub use chorus_flanger::{CHORUS_INFO, FLANGER_INFO, ModFx};
mod phaser;
pub use phaser::{PHASER_INFO, Phaser};

/// Per-channel LFO phase offset for stereo movement (channel 1 is offset by `spread` cycles).
#[inline]
fn channel_phase(phase: f32, c: usize, spread: f32) -> f32 {
    let p = phase + if c % 2 == 1 { spread } else { 0.0 };
    if p >= 1.0 { p - 1.0 } else { p }
}
