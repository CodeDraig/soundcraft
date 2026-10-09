//! Harmonic effects: Saturator (tanh), Lo-Fi (bit/sample-rate reduction) and Rectifier.

mod saturator;
pub use saturator::{SAT_INFO, Saturator};
mod lofi;
pub use lofi::{LOFI_INFO, LoFi};
mod rectifier;
pub use rectifier::{RECT_INFO, Rectifier};
