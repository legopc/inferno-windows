#![windows_subsystem = "windows"]

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use native_windows_gui as nwg;
use serde::{Deserialize, Serialize};

mod settings_window;

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
        .size((420, 300))
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

    // ── Peers label ───────────────────────────────────────────────────────────
    let mut lbl_peers: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Peers: \u{2014}")
        .position((10, 165))
        .size((400, 20))
        .parent(&window)
        .build(&mut lbl_peers)
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

    let mut btn_view_logs: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("View Logs")
        .position((310, 200))
        .size((100, 30))
        .parent(&window)
        .build(&mut btn_view_logs)
        .unwrap();

    let mut btn_settings: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Settings")
        .position((10, 255))
        .size((100, 30))
        .parent(&window)
        .build(&mut btn_settings)
        .unwrap();

    // ── Autostart checkbox ────────────────────────────────────────────────────
    let mut chk_autostart: nwg::CheckBox = Default::default();
    nwg::CheckBox::builder()
        .text("Launch on Windows start")
        .position((10, 240))
        .size((200, 20))
        .parent(&window)
        .build(&mut chk_autostart)
        .unwrap();

    // Set initial checkbox state based on whether the .bat file already exists.
    let startup_bat_path = format!(
        "{}\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\InfernoAoIP.bat",
        std::env::var("APPDATA").unwrap_or_default()
    );
    if std::path::Path::new(&startup_bat_path).exists() {
        chk_autostart.set_check_state(nwg::CheckBoxState::Checked);
    }

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
    let lbl_peers = Rc::new(lbl_peers);
    let btn_reload = Rc::new(btn_reload);
    let btn_shutdown = Rc::new(btn_shutdown);
    let btn_view_logs = Rc::new(btn_view_logs);
    let btn_settings = Rc::new(btn_settings);
    let chk_autostart = Rc::new(chk_autostart);
    let _timer = Rc::new(timer);

    let window_handle = window.handle;
    let btn_reload_handle = btn_reload.handle;
    let btn_shutdown_handle = btn_shutdown.handle;
    let btn_view_logs_handle = btn_view_logs.handle;
    let btn_settings_handle = btn_settings.handle;
    let chk_autostart_handle = chk_autostart.handle;

    let handler = nwg::full_bind_event_handler(
        &window_handle,
        move |evt, _data, handle| {
            use nwg::Event as E;
            match evt {
                E::OnWindowClose => {
                    if handle == window_handle {
                        let choice = nwg::modal_message(
                            &window_handle,
                            &nwg::MessageParams {
                                title: "Inferno AoIP",
                                content: "Minimize to background instead of quitting?",
                                buttons: nwg::MessageButtons::YesNo,
                                icons: nwg::MessageIcons::Question,
                            },
                        );
                        if choice == nwg::MessageChoice::Yes {
                            window.set_visible(false);
                        } else {
                            nwg::stop_thread_dispatch();
                        }
                    }
                }
                E::OnTimerTick => {
                    let snapshot = shared_status.lock().unwrap().clone();
                    match snapshot {
                        None => {
                            lbl_status.set_text("Status: Service not running");
                            lbl_rate.set_text("Sample Rate: \u{2014}");
                            lbl_channels.set_text("Channels: \u{2014}");
                            lbl_clock.set_text("Clock: \u{2014}");
                            lbl_uptime.set_text("Uptime: \u{2014}");
                            lbl_peers.set_text("Peers: \u{2014}");
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
                            // Placeholder — real peers arrive via IPC once dante_peers field is added
                            lbl_peers.set_text("Peers: (discovering...)");
                        }
                    }
                }
                E::OnButtonClick => {
                    if handle == btn_reload_handle {
                        shared_cmd.lock().unwrap().reload = true;
                    } else if handle == btn_shutdown_handle {
                        shared_cmd.lock().unwrap().shutdown = true;
                    } else if handle == btn_view_logs_handle {
                        let log_path = format!(
                            "{}\\inferno_aoip\\logs\\inferno.log",
                            std::env::var("LOCALAPPDATA").unwrap_or_default()
                        );
                        let content = std::fs::read_to_string(&log_path)
                            .unwrap_or_else(|_| "Log file not found".to_string());
                        let lines: Vec<&str> = content.lines().collect();
                        let tail: String = lines
                            .iter()
                            .rev()
                            .take(50)
                            .rev()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n");
                        nwg::modal_info_message(&window_handle, "Inferno Logs", &tail);
                    } else if handle == btn_settings_handle {
                        // Open settings window (non-modal for now; could be made modal with modal_dialog)
                        let result = show_settings_window(&window_handle, &shared_cmd);
                        if let Err(e) = result {
                            nwg::modal_error_message(&window_handle, "Settings Error", &format!("Failed to open settings: {}", e));
                        }
                    } else if handle == chk_autostart_handle {
                        let startup_dir = format!(
                            "{}\\Microsoft\\Windows\\Start Menu\\Programs\\Startup",
                            std::env::var("APPDATA").unwrap_or_default()
                        );
                        let bat_path = format!("{}\\InfernoAoIP.bat", startup_dir);
                        if chk_autostart.check_state() == nwg::CheckBoxState::Checked {
                            let exe = std::env::current_exe().unwrap_or_default();
                            let _ = std::fs::write(
                                &bat_path,
                                format!("@echo off\nstart \"\" \"{}\"", exe.display()),
                            );
                        } else {
                            let _ = std::fs::remove_file(&bat_path);
                        }
                    }
                }
                _ => {}
            }
        },
    );

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}

/// Show the settings window and handle user interactions
fn show_settings_window(parent_handle: &nwg::ControlHandle, shared_cmd: &SharedCmd) -> Result<(), String> {
    use settings_window::SettingsWindow;

    // Create settings window
    let mut window: nwg::Window = Default::default();
    nwg::Window::builder()
        .size((520, 580))
        .position((400, 200))
        .title("Inferno Settings")
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .build(&mut window)
        .map_err(|e| format!("Failed to create window: {:?}", e))?;

    let mut settings = SettingsWindow::default();

    // Build all the controls
    nwg::Label::builder()
        .text("Device Name:")
        .position((10, 15))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_device_name)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    nwg::TextInput::builder()
        .text("")
        .position((200, 15))
        .size((290, 25))
        .parent(&window)
        .build(&mut settings.txt_device_name)
        .map_err(|e| format!("Failed to create input: {:?}", e))?;

    nwg::Label::builder()
        .text("Sample Rate (Hz):")
        .position((10, 50))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_sample_rate)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    nwg::ComboBox::builder()
        .collection(vec!["44100".to_string(), "48000".to_string(), "96000".to_string()])
        .parent(&window)
        .size((290, 25))
        .position((200, 50))
        .build(&mut settings.combo_sample_rate)
        .map_err(|e| format!("Failed to create combobox: {:?}", e))?;

    nwg::Label::builder()
        .text("Channels (1-64):")
        .position((10, 85))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_channels)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    nwg::TextInput::builder()
        .text("2")
        .position((200, 85))
        .size((290, 25))
        .parent(&window)
        .build(&mut settings.txt_channels)
        .map_err(|e| format!("Failed to create input: {:?}", e))?;

    nwg::Label::builder()
        .text("Latency (ms):")
        .position((10, 120))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_latency)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    nwg::TextInput::builder()
        .text("10")
        .position((200, 120))
        .size((290, 25))
        .parent(&window)
        .build(&mut settings.txt_latency)
        .map_err(|e| format!("Failed to create input: {:?}", e))?;

    nwg::Label::builder()
        .text("WASAPI Device:")
        .position((10, 155))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_wasapi_device)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    nwg::ComboBox::builder()
        .collection(vec!["Default Device".to_string(), "Speakers".to_string(), "Line In".to_string()])
        .parent(&window)
        .size((290, 25))
        .position((200, 155))
        .build(&mut settings.combo_wasapi_device)
        .map_err(|e| format!("Failed to create combobox: {:?}", e))?;

    nwg::Label::builder()
        .text("Network Interface:")
        .position((10, 190))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_network_interface)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    let nics = settings_window::list_network_interfaces();
    nwg::ComboBox::builder()
        .collection(nics)
        .parent(&window)
        .size((290, 25))
        .position((200, 190))
        .build(&mut settings.combo_network_interface)
        .map_err(|e| format!("Failed to create combobox: {:?}", e))?;

    nwg::Label::builder()
        .text("FPP Mode:")
        .position((10, 225))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_fpp)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;

    nwg::ComboBox::builder()
        .collection(vec![
            "Auto (negotiate)".to_string(),
            "Min Latency (1 packet)".to_string(),
            "Max Efficiency (64 packets)".to_string(),
            "Custom...".to_string(),
        ])
        .parent(&window)
        .size((290, 25))
        .position((200, 225))
        .build(&mut settings.combo_fpp)
        .map_err(|e| format!("Failed to create combobox: {:?}", e))?;

    nwg::Label::builder()
        .text("Custom FPP Value:")
        .position((10, 260))
        .size((185, 20))
        .parent(&window)
        .build(&mut settings.lbl_custom_fpp)
        .map_err(|e| format!("Failed to create label: {:?}", e))?;
    settings.lbl_custom_fpp.set_visible(false);

    nwg::TextInput::builder()
        .text("")
        .position((200, 260))
        .size((290, 25))
        .parent(&window)
        .build(&mut settings.txt_custom_fpp)
        .map_err(|e| format!("Failed to create input: {:?}", e))?;
    settings.txt_custom_fpp.set_visible(false);

    nwg::Button::builder()
        .text("Save & Apply")
        .position((60, 320))
        .size((120, 32))
        .parent(&window)
        .build(&mut settings.btn_save)
        .map_err(|e| format!("Failed to create button: {:?}", e))?;

    nwg::Button::builder()
        .text("Cancel")
        .position((300, 320))
        .size((100, 32))
        .parent(&window)
        .build(&mut settings.btn_cancel)
        .map_err(|e| format!("Failed to create button: {:?}", e))?;

    // Load current config into window
    if let Err(e) = settings_window::load_config_into_window(&settings) {
        nwg::modal_error_message(parent_handle, "Settings", &format!("Failed to load config: {}", e));
    }

    // Set up event handler - use Arc<Cell> to communicate between handler and main
    let window_handle = window.handle;
    let btn_save_handle = settings.btn_save.handle;
    let btn_cancel_handle = settings.btn_cancel.handle;

    let should_save = std::sync::Arc::new(std::cell::Cell::new(false));
    let should_close = std::sync::Arc::new(std::cell::Cell::new(false));
    
    let settings_rc = Rc::new(std::cell::RefCell::new(settings));
    let should_save_clone = std::sync::Arc::clone(&should_save);
    let should_close_clone = std::sync::Arc::clone(&should_close);

    let handler = nwg::full_bind_event_handler(
        &window_handle,
        move |evt, _data, handle| {
            use nwg::Event as E;
            match evt {
                E::OnButtonClick => {
                    if handle == btn_save_handle {
                        should_save_clone.set(true);
                        should_close_clone.set(true);
                    } else if handle == btn_cancel_handle {
                        should_close_clone.set(true);
                    }
                }
                E::OnWindowClose => {
                    should_close_clone.set(true);
                }
                _ => {}
            }
        },
    );

    // Show window modally by dispatching events
    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);

    // After user closes window, check if they clicked save
    if should_save.get() {
        let s = settings_rc.borrow();
        match settings_window::save_config_from_window(&s) {
            Ok(()) => {
                shared_cmd.lock().unwrap().reload = true;
                nwg::modal_info_message(parent_handle, "Settings", "Configuration saved. Reload triggered.");
            }
            Err(e) => {
                nwg::modal_error_message(parent_handle, "Settings", &format!("Failed to save config: {}", e));
            }
        }
    }

    Ok(())
}
