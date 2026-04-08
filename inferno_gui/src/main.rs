#![windows_subsystem = "windows"]

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use native_windows_gui as nwg;
use serde::{Deserialize, Serialize};

// ── IPC types ────────────────────────────────────────────────────────────────

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
    clock_mode: String,
    uptime_secs: u64,
}

type SharedStatus = Arc<Mutex<Option<StatusData>>>;

#[derive(Clone, Default)]
struct PendingCommand {
    reload: bool,
    shutdown: bool,
}
type SharedCmd = Arc<Mutex<PendingCommand>>;

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

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
            *status.lock().unwrap() = None;
            return;
        }
    };

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
        return;
    }

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
        clock_mode,
        uptime_secs,
        ..
    }) = serde_json::from_str(&line)
    {
        *status.lock().unwrap() = Some(StatusData {
            tx_active,
            rx_active,
            rx_channels,
            sample_rate,
            clock_mode,
            uptime_secs,
        });
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    nwg::init().expect("Failed to init NWG");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set font");

    // ── Build window ──────────────────────────────────────────────────────────
    let mut window: nwg::Window = Default::default();
    nwg::Window::builder()
        .size((420, 260))
        .position((300, 300))
        .title("Inferno AoIP")
        .build(&mut window)
        .unwrap();

    // ── Labels ────────────────────────────────────────────────────────────────
    let mut lbl_status: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Status: Connecting...")
        .position((10, 15))
        .size((400, 20))
        .parent(&window)
        .build(&mut lbl_status)
        .unwrap();

    let mut lbl_rate: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Sample Rate: —")
        .position((10, 45))
        .size((400, 20))
        .parent(&window)
        .build(&mut lbl_rate)
        .unwrap();

    let mut lbl_channels: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Channels: —")
        .position((10, 75))
        .size((400, 20))
        .parent(&window)
        .build(&mut lbl_channels)
        .unwrap();

    let mut lbl_clock: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Clock: —")
        .position((10, 105))
        .size((400, 20))
        .parent(&window)
        .build(&mut lbl_clock)
        .unwrap();

    let mut lbl_uptime: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Uptime: —")
        .position((10, 135))
        .size((400, 20))
        .parent(&window)
        .build(&mut lbl_uptime)
        .unwrap();

    // ── Buttons ───────────────────────────────────────────────────────────────
    let mut btn_reload: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Reload Config")
        .position((10, 200))
        .size((130, 30))
        .parent(&window)
        .build(&mut btn_reload)
        .unwrap();

    let mut btn_shutdown: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Shutdown Service")
        .position((155, 200))
        .size((140, 30))
        .parent(&window)
        .build(&mut btn_shutdown)
        .unwrap();

    // ── Timer ─────────────────────────────────────────────────────────────────
    let mut timer: nwg::AnimationTimer = Default::default();
    nwg::AnimationTimer::builder()
        .parent(&window)
        .interval(std::time::Duration::from_millis(2000))
        .build(&mut timer)
        .unwrap();
    timer.start();

    // ── Shared state & IPC thread ─────────────────────────────────────────────
    let shared_status: SharedStatus = Arc::new(Mutex::new(None));
    let shared_cmd: SharedCmd = Arc::new(Mutex::new(PendingCommand::default()));

    spawn_ipc_thread(Arc::clone(&shared_status), Arc::clone(&shared_cmd));

    // ── Wrap controls in Rc so the closure can own them ───────────────────────
    let window = Rc::new(window);
    let lbl_status = Rc::new(lbl_status);
    let lbl_rate = Rc::new(lbl_rate);
    let lbl_channels = Rc::new(lbl_channels);
    let lbl_clock = Rc::new(lbl_clock);
    let lbl_uptime = Rc::new(lbl_uptime);
    let btn_reload = Rc::new(btn_reload);
    let btn_shutdown = Rc::new(btn_shutdown);
    let _timer = Rc::new(timer);

    let window_handle = window.handle;
    let btn_reload_handle = btn_reload.handle;
    let btn_shutdown_handle = btn_shutdown.handle;

    let handler = nwg::full_bind_event_handler(
        &window_handle,
        move |evt, _data, handle| {
            use nwg::Event as E;
            match evt {
                E::OnWindowClose => {
                    if handle == window_handle {
                        nwg::stop_thread_dispatch();
                    }
                }
                E::OnTimerTick => {
                    let snapshot = shared_status.lock().unwrap().clone();
                    match snapshot {
                        None => {
                            lbl_status.set_text("Status: Service not running");
                            lbl_rate.set_text("Sample Rate: —");
                            lbl_channels.set_text("Channels: —");
                            lbl_clock.set_text("Clock: —");
                            lbl_uptime.set_text("Uptime: —");
                        }
                        Some(s) => {
                            let rx = if s.rx_active { "RX Active" } else { "RX Idle" };
                            let tx = if s.tx_active { "TX Active" } else { "TX Idle" };
                            lbl_status.set_text(&format!("Status: {} / {}", rx, tx));
                            lbl_rate.set_text(&format!("Sample Rate: {} Hz", s.sample_rate));
                            lbl_channels
                                .set_text(&format!("Channels: {}", s.rx_channels));
                            lbl_clock.set_text(&format!("Clock: {}", s.clock_mode));
                            lbl_uptime
                                .set_text(&format!("Uptime: {}", format_uptime(s.uptime_secs)));
                        }
                    }
                }
                E::OnButtonClick => {
                    if handle == btn_reload_handle {
                        shared_cmd.lock().unwrap().reload = true;
                    } else if handle == btn_shutdown_handle {
                        shared_cmd.lock().unwrap().shutdown = true;
                    }
                }
                _ => {}
            }
        },
    );

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}
