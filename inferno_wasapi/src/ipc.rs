//! Named pipe IPC between inferno service and tray GUI.
//! Protocol: newline-delimited JSON messages.

use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\inferno";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StatusMessage {
    /// Service -> Tray: periodic status update
    Status {
        tx_active: bool,
        rx_active: bool,
        tx_channels: u32,
        rx_channels: u32,
        sample_rate: u32,
        clock_mode: String,    // "SafeClock" or "PTP"
        tx_peak_db: Vec<f32>,  // per-channel peaks in dB
        uptime_secs: u64,
    },
    /// Service -> Tray: error notification
    Error { message: String },
    /// Tray -> Service: request status
    GetStatus,
    /// Tray -> Service: reload config
    ReloadConfig,
    /// Tray -> Service: graceful shutdown
    Shutdown,
}

/// Start the IPC server (runs in service, accepts tray connections)
pub async fn start_ipc_server(
    start_time: std::time::Instant,
    _tx_channels: u32,
    rx_channels: u32,
    sample_rate: u32,
) {
    tracing::info!("IPC server initialized for {}", PIPE_NAME);
    tracing::debug!("Listening for tray connections (rx_channels={}, sample_rate={}Hz)", rx_channels, sample_rate);
    
    // Placeholder: keep task alive and periodically log uptime
    loop {
        let _uptime = start_time.elapsed().as_secs();
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}
