//! Named pipe IPC between inferno service and tray GUI.
//! Protocol: newline-delimited JSON messages.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;

pub const PIPE_NAME: &str = r"\\.\pipe\inferno";

/// Runtime state of the audio service, shared between the audio engine and IPC handlers.
#[derive(Default, Clone)]
pub struct ServiceState {
    pub rx_active: bool,
    pub tx_active: bool,
    pub rx_channels: u32,
    pub tx_channels: u32,
    pub sample_rate: u32,
    pub clock_mode: String,
    pub tx_peak_db: Vec<f32>,
    pub dante_peers: Vec<String>,
    pub reload_requested: bool,
}

pub type SharedState = Arc<tokio::sync::RwLock<ServiceState>>;

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

async fn handle_client(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    start_time: std::time::Instant,
    state: SharedState,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
) {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let msg: StatusMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("IPC: invalid message from client: {e}");
                continue;
            }
        };

        match msg {
            StatusMessage::GetStatus => {
                let s = state.read().await;
                let response = StatusMessage::Status {
                    tx_active: s.tx_active,
                    rx_active: s.rx_active,
                    tx_channels: s.tx_channels,
                    rx_channels: s.rx_channels,
                    sample_rate: s.sample_rate,
                    clock_mode: if s.clock_mode.is_empty() {
                        "SafeClock".to_string()
                    } else {
                        s.clock_mode.clone()
                    },
                    tx_peak_db: s.tx_peak_db.clone(),
                    uptime_secs: start_time.elapsed().as_secs(),
                };
                drop(s);
                match serde_json::to_string(&response) {
                    Ok(json) => {
                        if let Err(e) = writer.write_all(format!("{json}\n").as_bytes()).await {
                            tracing::warn!("IPC: write error: {e}");
                            break;
                        }
                    }
                    Err(e) => tracing::error!("IPC: serialization error: {e}"),
                }
            }
            StatusMessage::Shutdown => {
                tracing::info!("IPC: shutdown requested by client");
                shutdown_tx.send(true).ok();
            }
            StatusMessage::ReloadConfig => {
                tracing::info!("IPC: reload config requested");
                state.write().await.reload_requested = true;
            }
            other => {
                tracing::debug!("IPC: unexpected message variant: {:?}", other);
            }
        }
    }
}

/// Start the IPC server (runs in service, accepts tray connections).
/// Loops: create a server instance, wait for a client to connect, spawn a handler task,
/// then create a fresh server instance for the next client.
pub async fn start_ipc_server(
    start_time: std::time::Instant,
    state: SharedState,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
) {
    tracing::info!("IPC server listening on {}", PIPE_NAME);

    // The first instance must be created before entering the accept loop so the
    // pipe name is registered in the system before any client tries to connect.
    let mut server = match ServerOptions::new()
        .first_pipe_instance(true)
        .create(PIPE_NAME)
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("IPC: failed to create named pipe: {e}");
            return;
        }
    };

    loop {
        // Wait for a client to connect to the current server instance.
        if let Err(e) = server.connect().await {
            tracing::warn!("IPC: connect error: {e}");
            // Create a replacement instance and try again.
            server = match ServerOptions::new().create(PIPE_NAME) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("IPC: failed to recreate named pipe: {e}");
                    return;
                }
            };
            continue;
        }

        // Create the next server instance *before* handing the connected one to a
        // task, so new clients can connect while the current one is being served.
        let next_server = match ServerOptions::new().create(PIPE_NAME) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("IPC: failed to create next pipe instance: {e}");
                return;
            }
        };

        let connected = std::mem::replace(&mut server, next_server);
        tokio::spawn(handle_client(
            connected,
            start_time,
            state.clone(),
            shutdown_tx.clone(),
        ));
    }
}
