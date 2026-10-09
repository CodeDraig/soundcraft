//! Level meters: DAW-style peak meter, RMS meter, and an ITU-R BS.1770 loudness meter
//! (momentary, short-term and gated integrated LUFS plus 4× oversampled true peak).
//!
//! None of the `process` methods allocate.

mod peak;
pub use peak::PeakMeter;
mod rms;
pub use rms::RmsMeter;
mod loudness;
pub use loudness::LoudnessMeter;
