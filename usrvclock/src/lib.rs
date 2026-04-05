//! Windows stub for usrvclock protocol.
//!
//! On Windows, PTP clock integration via Unix domain sockets is not available.
//! This stub provides the same API as the Unix version but:
//! - `ClockOverlay::now_ns()` uses `std::time::SystemTime` instead of a PTP-adjusted clock
//! - `AsyncClient` never delivers clock overlays (no PTP daemon on Windows yet)
//!
//! See NOTES.md in the workspace root for details on enabling PTP on Windows.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use custom_error::custom_error;

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

    /// Returns current underlying clock timestamp in nanoseconds (using SystemTime on Windows).
    pub fn now_underlying_ns(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
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

/// Placeholder for SafeClock (not implemented on Windows).
pub struct SafeClock;

/// Placeholder timestamp for SafeClock.
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
        Self
    }

    pub fn now(&mut self, overlay: &ClockOverlay) -> SafeTimestamp {
        SafeTimestamp { nanos: overlay.now_ns(), estimated: false }
    }
}

/// Default path for the clock overlay socket.
/// On Windows, this is unused (no Unix domain socket support).
pub const DEFAULT_SERVER_SOCKET_PATH: &str = r"\\.\pipe\ptp-usrvclock";

/// Async client for the usrvclock protocol.
///
/// On Windows, this is a stub that never delivers clock overlays.
/// A future implementation could use a named pipe or UDP socket to receive
/// clock data from a Windows PTP daemon.
#[cfg(feature = "tokio")]
pub struct AsyncClient {
    sender: tokio::sync::watch::Sender<Option<ClockOverlay>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
}

#[cfg(feature = "tokio")]
impl AsyncClient {
    /// Starts the async client. On Windows, this spawns a no-op task that
    /// waits for shutdown. No clock overlays will be delivered.
    pub fn start(_path: PathBuf, _error_handler: Box<dyn FnMut(OverlayReceiveError) + Send>) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (tx, _rx) = tokio::sync::watch::channel(None);
        let join_handle = tokio::spawn(async move {
            // Windows stub: wait for shutdown signal, never deliver clock overlays.
            let _ = shutdown_rx.await;
        });
        Self { sender: tx, shutdown: Some(shutdown_tx), join_handle: Some(join_handle) }
    }

    /// Subscribes to clock overlay updates.
    /// On Windows, this receiver will never receive a value (always None).
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
