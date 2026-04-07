pub use tracing::{debug, error, info, trace, warn};
pub type Sample = i32;
pub type USample = u32;

/// Audio clock (number of samples since arbitrary epoch). May wrap.
pub type Clock = usize;

/// Signed version of the clock. For clock deltas.
pub type ClockDiff = isize;

/// Non-wrapping clock
pub type LongClock = u64;

/// Signed version of non-wrapping clock. For clock deltas.
/// In a correctly working network, can be casted to ClockDiff without checks.
pub type LongClockDiff = i64;

/// Subtract clocks and return the result as a signed number.
/// Hint: wrapped `a > b` is equivalent to `wrapped_diff(a, b) > 0`
/// This function is intentionally not defined for LongClock, because diffs should never exceed i32 anyway.
pub fn wrapped_diff(a: Clock, b: Clock) -> ClockDiff {
  (a as ClockDiff).wrapping_sub(b as ClockDiff)
}

pub trait LogAndForget {
  fn log_and_forget(&self);
}

impl<T, E: std::fmt::Debug> LogAndForget for Result<T, E> {
  fn log_and_forget(&self) {
    if let Err(e) = self {
      warn!("Encountered error {e:?} at {:?}", std::backtrace::Backtrace::capture());
    }
  }
}
