//! Utilities: Time Shift, Gain, Trim, Invert, DC Offset Removal, Signal Generator, Dither.

mod time_shift;
pub use time_shift::{SHIFT_INFO, TimeShift};
mod gain;
pub use gain::{GAIN_INFO, Gain, TRIM_INFO, Trim};
use gain::{OFF_DB, fader_gain};
mod invert;
pub use invert::{INVERT_INFO, Invert};
mod dc_removal;
pub use dc_removal::{DC_INFO, DcRemoval};
mod signal_generator;
pub use signal_generator::{GEN_INFO, SignalGenerator};
mod dither;
pub use dither::{DITHER_INFO, Dither};
