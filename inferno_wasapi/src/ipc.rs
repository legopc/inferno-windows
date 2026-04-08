//! Named pipe IPC between inferno service and tray GUI.
//! Protocol: newline-delimited JSON messages.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;

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

async fn handle_client(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    start_time: std::time::Instant,
    tx_channels: u32,
    rx_channels: u32,
    sample_rate: u32,
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
                let response = StatusMessage::Status {
                    tx_active: true,
                    rx_active: true,
                    tx_channels,
                    rx_channels,
                    sample_rate,
                    clock_mode: "SafeClock".to_string(),
                    tx_peak_db: vec![-60.0; tx_channels as usize],
                    uptime_secs: start_time.elapsed().as_secs(),
                };
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
            }
            StatusMessage::ReloadConfig => {
                tracing::info!("IPC: reload config requested");
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
    tx_channels: u32,
    rx_channels: u32,
    sample_rate: u32,
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
            tx_channels,
            rx_channels,
            sample_rate,
        ));
    }
}
