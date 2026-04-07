//! Windows stub for usrvclock protocol.
//!
//! On Windows, PTP clock integration via Unix domain sockets is not available.
//! This stub provides the same API as the Unix version but:
//! - When a PTP daemon is available: `ClockOverlay::now_ns()` uses PTP-adjusted timestamps
//! - When no PTP grandmaster: `SafeClock` uses QPC (QueryPerformanceCounter) for high-resolution free-running clock
//!
//! See NOTES.md in the workspace root for details on enabling PTP on Windows.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use std::sync::Mutex;

use custom_error::custom_error;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;

const OVERLAY_SIZE_BYTES: usize = 40;
const PROTOCOL_MAJOR_VERSION: u16 = 1;
#[allow(dead_code)]
const PROTOCOL_MINOR_VERSION: u16 = 0;

/// Clock overlay received from a PTP clock daemon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockOverlay {
    pub clock_id: i64,
    pub last_sync: i64,
    pub shift: i64,
    pub freq_scale: f64,
}

custom_error! { pub OverlayReceiveError
    PacketTooShort = "packet too short",
    UnexpectedData = "unexpected data in packet",
    UnsupportedMajorVersion = "unsupported major version",
    InvalidFlags = "invalid flags (client too old?)",
}

impl ClockOverlay {
    pub(crate) fn decode(buff: &[u8]) -> Result<Self, OverlayReceiveError> {
        if buff.len() < OVERLAY_SIZE_BYTES {
            return Err(OverlayReceiveError::PacketTooShort);
        }
        if buff[0] != b'V' || buff[1] != b'C' {
            return Err(OverlayReceiveError::UnexpectedData);
        }
        let major_version = u16::from_ne_bytes(buff[2..4].try_into().unwrap());
        if major_version != PROTOCOL_MAJOR_VERSION {
            return Err(OverlayReceiveError::UnsupportedMajorVersion);
        }
        let flags = u16::from_ne_bytes(buff[6..8].try_into().unwrap());
        if (flags & 1) == 0 {
            return Err(OverlayReceiveError::InvalidFlags);
        }
        Ok(Self {
            clock_id: i64::from_ne_bytes(buff[8..16].try_into().unwrap()),
            last_sync: i64::from_ne_bytes(buff[16..24].try_into().unwrap()),
            shift: i64::from_ne_bytes(buff[24..32].try_into().unwrap()),
            freq_scale: f64::from_ne_bytes(buff[32..40].try_into().unwrap()),
        })
    }

    /// Calculates timestamp in overlay clock's timescale given underlying clock's timestamp.
    /// All timestamps are in nanoseconds and may wrap.
    pub fn underlying_to_overlay_ns(&self, timestamp: i64) -> i64 {
        let elapsed = timestamp.wrapping_sub(self.last_sync);
        let correction = ((elapsed as f64) * self.freq_scale).round() as i64;
        timestamp.wrapping_add(self.shift).wrapping_add(correction)
    }

    /// Returns current underlying clock timestamp in nanoseconds (using GetSystemTimePreciseAsFileTime on Windows).
    pub fn now_underlying_ns(&self) -> i64 {
        get_precise_system_time_ns()
    }

    /// Returns current timestamp adjusted through the clock overlay.
    /// On Windows, uses SystemTime; on Linux this would use a PTP hardware/software clock.
    pub fn now_ns(&self) -> i64 {
        self.underlying_to_overlay_ns(self.now_underlying_ns())
    }

    /// Returns the combined frequency scale (overlay + hardware adjustment).
    /// On Windows, hardware clock adjustment is not available, so this returns freq_scale.
    pub fn freq_scale_including_hw(&self) -> f64 {
        self.freq_scale
    }
}

/// SafeClock using QPC for high-resolution free-running clock on Windows.
/// Uses QueryPerformanceCounter (monotonic) anchored to GetSystemTimePreciseAsFileTime (~100ns resolution).
pub struct SafeClock {
    inner: Arc<Mutex<SafeClockInner>>,
}

struct SafeClockInner {
    qpc_anchor: i64,
    filetime_anchor: u64,  // 100ns intervals since 1601-01-01
    qpc_freq: i64,
}

/// Timestamp from SafeClock.
pub struct SafeTimestamp {
    pub nanos: i64,
    pub estimated: bool,
}

impl SafeTimestamp {
    pub fn precise_ns(&self) -> Option<i64> {
        if !self.estimated { Some(self.nanos) } else { None }
    }
}

impl SafeClock {
    pub fn new(_tolerance: f64, _timeout_ns: i64) -> Self {
        let mut freq = 0i64;
        let mut counter = 0i64;
        
        unsafe {
            let _ = QueryPerformanceFrequency(&mut freq);
            let _ = QueryPerformanceCounter(&mut counter);
        }
        
        // Get initial wall-clock anchor using GetSystemTimePreciseAsFileTime
        let ft = unsafe { GetSystemTimePreciseAsFileTime() };
        
        let filetime = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
        
        tracing::info!(
            "SafeClock initialized: QPC freq={} Hz, anchor filetime={}, QPC counter={}",
            freq, filetime, counter
        );
        
        Self {
            inner: Arc::new(Mutex::new(SafeClockInner {
                qpc_anchor: counter,
                filetime_anchor: filetime,
                qpc_freq: freq,
            })),
        }
    }

    pub fn now(&mut self, overlay: &ClockOverlay) -> SafeTimestamp {
        let inner = self.inner.lock().unwrap();
        
        let mut counter = 0i64;
        unsafe {
            let _ = QueryPerformanceCounter(&mut counter);
        }
        
        let elapsed_ticks = counter.saturating_sub(inner.qpc_anchor);
        
        // Convert QPC ticks to 100ns intervals
        // elapsed_100ns = (elapsed_ticks * 10_000_000) / qpc_freq
        let elapsed_100ns = if inner.qpc_freq > 0 {
            ((elapsed_ticks as u128).saturating_mul(10_000_000u128)) / (inner.qpc_freq as u128)
        } else {
            0
        };
        
        // Current filetime in 100ns intervals since 1601-01-01
        let current_filetime_100ns = inner.filetime_anchor.saturating_add(elapsed_100ns as u64);
        
        // Convert to Unix epoch (1970-01-01)
        // Offset between 1601 and 1970 is 116_444_736_000_000_000 (100ns intervals)
        let unix_100ns = if current_filetime_100ns >= 116_444_736_000_000_000u64 {
            current_filetime_100ns - 116_444_736_000_000_000u64
        } else {
            0
        };
        
        // Convert 100ns intervals to nanoseconds
        let nanos = (unix_100ns as i64).saturating_mul(100);
        
        // Apply overlay correction to the SafeClock timestamp
        let overlay_nanos = overlay.underlying_to_overlay_ns(nanos);
        
        SafeTimestamp { nanos: overlay_nanos, estimated: false }
    }
}

/// Default path for the clock overlay socket.
/// On Windows, this is unused (no Unix domain socket support).
pub const DEFAULT_SERVER_SOCKET_PATH: &str = r"\\.\pipe\ptp-usrvclock";

/// Get precise system time in nanoseconds using GetSystemTimePreciseAsFileTime.
/// Provides ~100ns resolution on modern Windows systems.
fn get_precise_system_time_ns() -> i64 {
    let ft = unsafe { GetSystemTimePreciseAsFileTime() };
    let filetime = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
    // Convert FILETIME (100ns intervals since 1601-01-01) to ns since Unix epoch
    let unix_100ns = if filetime >= 116_444_736_000_000_000u64 {
        filetime - 116_444_736_000_000_000u64
    } else {
        0
    };
    (unix_100ns as i64).saturating_mul(100)
}

fn system_time_overlay() -> ClockOverlay {
    let now_ns = get_precise_system_time_ns();
    // shift=0, freq_scale=0.0: pass system time through unchanged (no PTP correction)
    ClockOverlay { clock_id: 0, last_sync: now_ns, shift: 0, freq_scale: 0.0 }
}

/// Async client for the usrvclock protocol.
///
/// On Windows, no PTP daemon is available. This stub delivers a system-time-based
/// `ClockOverlay` immediately on start, and refreshes it every second so that
/// `MediaClock::wrapping_now_in_timebase()` always returns a valid timestamp.
/// Audio will play without PTP-corrected timing (some drift expected).
#[cfg(feature = "tokio")]
pub struct AsyncClient {
    sender: tokio::sync::watch::Sender<Option<ClockOverlay>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
}

#[cfg(feature = "tokio")]
impl AsyncClient {
    /// Starts the async client. Immediately sends a system-time overlay and
    /// refreshes it every second so `MediaClock` always has a valid timestamp.
    pub fn start(_path: PathBuf, _error_handler: Box<dyn FnMut(OverlayReceiveError) + Send>) -> Self {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let (tx, _rx) = tokio::sync::watch::channel(Some(system_time_overlay()));
        let tx2 = tx.clone();
        let join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                        let _ = tx2.send(Some(system_time_overlay()));
                    }
                }
            }
        });
        Self { sender: tx, shutdown: Some(shutdown_tx), join_handle: Some(join_handle) }
    }

    /// Subscribes to clock overlay updates.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Option<ClockOverlay>> {
        self.sender.subscribe()
    }

    /// Stops the client.
    pub async fn stop(mut self) -> Result<(), tokio::task::JoinError> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join_handle.take() {
            handle.await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_overlay_decode_valid() {
        let mut buf = vec![0u8; 40];
        buf[0] = b'V';
        buf[1] = b'C';
        buf[2..4].copy_from_slice(&PROTOCOL_MAJOR_VERSION.to_ne_bytes());
        buf[6..8].copy_from_slice(&1u16.to_ne_bytes()); // flags = 1 (valid)
        buf[8..16].copy_from_slice(&42i64.to_ne_bytes());
        buf[16..24].copy_from_slice(&1000i64.to_ne_bytes());
        buf[24..32].copy_from_slice(&500i64.to_ne_bytes());
        buf[32..40].copy_from_slice(&1.5f64.to_ne_bytes());

        let overlay = ClockOverlay::decode(&buf).unwrap();
        assert_eq!(overlay.clock_id, 42);
        assert_eq!(overlay.last_sync, 1000);
        assert_eq!(overlay.shift, 500);
        assert_eq!(overlay.freq_scale, 1.5);
    }

    #[test]
    fn test_clock_overlay_decode_packet_too_short() {
        let buf = vec![0u8; 30];
        assert!(ClockOverlay::decode(&buf).is_err());
    }

    #[test]
    fn test_clock_overlay_decode_invalid_magic() {
        let mut buf = vec![0u8; 40];
        buf[0] = b'X';
        buf[1] = b'Y';
        assert!(matches!(
            ClockOverlay::decode(&buf),
            Err(OverlayReceiveError::UnexpectedData)
        ));
    }

    #[test]
    fn test_clock_overlay_decode_unsupported_version() {
        let mut buf = vec![0u8; 40];
        buf[0] = b'V';
        buf[1] = b'C';
        buf[2..4].copy_from_slice(&99u16.to_ne_bytes());
        assert!(matches!(
            ClockOverlay::decode(&buf),
            Err(OverlayReceiveError::UnsupportedMajorVersion)
        ));
    }

    #[test]
    fn test_clock_overlay_decode_invalid_flags() {
        let mut buf = vec![0u8; 40];
        buf[0] = b'V';
        buf[1] = b'C';
        buf[2..4].copy_from_slice(&PROTOCOL_MAJOR_VERSION.to_ne_bytes());
        buf[6..8].copy_from_slice(&0u16.to_ne_bytes()); // flags = 0 (invalid)
        assert!(matches!(
            ClockOverlay::decode(&buf),
            Err(OverlayReceiveError::InvalidFlags)
        ));
    }

    #[test]
    fn test_safeclock_is_monotonic() {
        let mut clock = SafeClock::new(0.0, 0);
        let overlay = system_time_overlay();
        
        let t1 = clock.now(&overlay);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = clock.now(&overlay);
        
        assert!(t2.nanos > t1.nanos, "clock should be monotonic: t1={} t2={}", t1.nanos, t2.nanos);
    }

    #[test]
    fn test_safeclock_resolution_reasonable() {
        let mut clock = SafeClock::new(0.0, 0);
        let overlay = system_time_overlay();
        
        let t1 = clock.now(&overlay);
        let t2 = clock.now(&overlay);
        
        // Two consecutive reads should differ by less than 1 second (reasonable)
        let delta = (t2.nanos - t1.nanos).abs();
        assert!(delta < 1_000_000_000, "clock resolution unreasonable: delta={} ns", delta);
    }

    #[test]
    fn test_clock_overlay_underlying_to_overlay_no_correction() {
        let overlay = ClockOverlay {
            clock_id: 1,
            last_sync: 1000,
            shift: 0,
            freq_scale: 0.0,
        };
        let underlying = 2000i64;
        let result = overlay.underlying_to_overlay_ns(underlying);
        // With shift=0 and freq_scale=0.0, result should be underlying (no correction)
        assert_eq!(result, underlying);
    }

    #[test]
    fn test_clock_overlay_underlying_to_overlay_with_shift() {
        let overlay = ClockOverlay {
            clock_id: 1,
            last_sync: 1000,
            shift: 500,
            freq_scale: 0.0,
        };
        let underlying = 2000i64;
        let result = overlay.underlying_to_overlay_ns(underlying);
        // Result should be underlying + shift
        assert_eq!(result, underlying + 500);
    }

    #[test]
    fn test_system_time_overlay() {
        let overlay = system_time_overlay();
        assert_eq!(overlay.clock_id, 0);
        assert_eq!(overlay.shift, 0);
        assert_eq!(overlay.freq_scale, 0.0);
        assert!(overlay.last_sync > 0);
    }
}
