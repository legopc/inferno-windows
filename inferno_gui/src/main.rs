#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use serde::{Deserialize, Serialize};

// ── IPC types (mirrors inferno_wasapi::ipc) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum IpcMessage {
    GetStatus,
    ReloadConfig,
    Shutdown,
    Status {
        tx_active: bool,
        rx_active: bool,
        tx_channels: u32,
        rx_channels: u32,
        sample_rate: u32,
        clock_mode: String,
        tx_peak_db: Vec<f32>,
        uptime_secs: u64,
    },
    Error {
        message: String,
    },
}

// ── Shared state ─────────────────────────────────────────────────────────────

#[derive(Clone)]
struct StatusData {
    tx_active: bool,
    rx_active: bool,
    rx_channels: u32,
    sample_rate: u32,
    uptime_secs: u64,
}

type SharedStatus = Arc<Mutex<Option<StatusData>>>;

/// Pending one-shot commands the UI wants to send on the next pipe connect.
#[derive(Clone, Default)]
struct PendingCommand {
    reload: bool,
    shutdown: bool,
}
type SharedCmd = Arc<Mutex<PendingCommand>>;

// ── Background IPC thread ────────────────────────────────────────────────────

fn spawn_ipc_thread(status: SharedStatus, cmd: SharedCmd) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async move {
            loop {
                try_poll_once(&status, &cmd).await;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    });
}

async fn try_poll_once(status: &SharedStatus, cmd: &SharedCmd) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;

    let pipe = match ClientOptions::new().open(r"\\.\pipe\inferno") {
        Ok(p) => p,
        Err(_) => {
            // Service not running — clear status so UI shows the error banner.
            *status.lock().unwrap() = None;
            return;
        }
    };

    // Drain any pending command first, then always poll status.
    let (do_reload, do_shutdown) = {
        let mut c = cmd.lock().unwrap();
        let r = (c.reload, c.shutdown);
        *c = PendingCommand::default();
        r
    };

    let (reader, mut writer) = tokio::io::split(pipe);
    let mut reader = BufReader::new(reader);

    if do_reload {
        let msg = serde_json::to_vec(&IpcMessage::ReloadConfig).unwrap_or_default();
        let _ = writer.write_all(&msg).await;
        let _ = writer.write_all(b"\n").await;
    }
    if do_shutdown {
        let msg = serde_json::to_vec(&IpcMessage::Shutdown).unwrap_or_default();
        let _ = writer.write_all(&msg).await;
        let _ = writer.write_all(b"\n").await;
        return; // Don't wait for a status reply after shutdown.
    }

    // Request status.
    let msg = serde_json::to_vec(&IpcMessage::GetStatus).unwrap_or_default();
    if writer.write_all(&msg).await.is_err() {
        return;
    }
    if writer.write_all(b"\n").await.is_err() {
        return;
    }

    let mut line = String::new();
    match tokio::time::timeout(Duration::from_secs(3), reader.read_line(&mut line)).await {
        Ok(Ok(_)) => {}
        _ => return,
    }

    if let Ok(IpcMessage::Status {
        tx_active,
        rx_active,
        rx_channels,
        sample_rate,
        uptime_secs,
        ..
    }) = serde_json::from_str(&line)
    {
        *status.lock().unwrap() = Some(StatusData {
            tx_active,
            rx_active,
            rx_channels,
            sample_rate,
            uptime_secs,
        });
    }
}

// ── egui App ─────────────────────────────────────────────────────────────────

struct InfernoApp {
    status: SharedStatus,
    cmd: SharedCmd,
}

impl InfernoApp {
    fn new(status: SharedStatus, cmd: SharedCmd) -> Self {
        Self { status, cmd }
    }
}

impl eframe::App for InfernoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint every 2 s so the display stays fresh.
        ctx.request_repaint_after(Duration::from_secs(2));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Inferno AoIP");
            ui.separator();

            let maybe_status = self.status.lock().unwrap().clone();

            match maybe_status {
                None => {
                    ui.colored_label(egui::Color32::RED, "⚠ Service not running");
                }
                Some(s) => {
                    // Status grid
                    egui::Grid::new("status_grid")
                        .num_columns(2)
                        .spacing([20.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("RX:");
                            if s.rx_active {
                                ui.colored_label(egui::Color32::GREEN, "Active");
                            } else {
                                ui.label("Idle");
                            }
                            ui.end_row();

                            ui.label("TX:");
                            if s.tx_active {
                                ui.colored_label(egui::Color32::GREEN, "Active");
                            } else {
                                ui.label("Idle");
                            }
                            ui.end_row();

                            ui.label("Sample Rate:");
                            ui.label(format!("{} Hz", s.sample_rate));
                            ui.end_row();

                            ui.label("Channels:");
                            ui.label(format!("{}", s.rx_channels));
                            ui.end_row();

                            ui.label("Uptime:");
                            let mins = s.uptime_secs / 60;
                            let secs = s.uptime_secs % 60;
                            ui.label(format!("{}m {}s", mins, secs));
                            ui.end_row();
                        });

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Reload Config").clicked() {
                            self.cmd.lock().unwrap().reload = true;
                        }
                        if ui.button("Shutdown Service").clicked() {
                            self.cmd.lock().unwrap().shutdown = true;
                        }
                    });
                }
            }
        });
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    let status: SharedStatus = Arc::new(Mutex::new(None));
    let cmd: SharedCmd = Arc::new(Mutex::new(PendingCommand::default()));

    spawn_ipc_thread(Arc::clone(&status), Arc::clone(&cmd));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Inferno AoIP")
            .with_inner_size([400.0, 300.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "Inferno AoIP",
        options,
        Box::new(move |_cc| -> Box<dyn eframe::App> { Box::new(InfernoApp::new(status, cmd)) }),
    )
}
