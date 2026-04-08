# inferno_gui

Win32 status window for the Inferno AoIP Windows service.

Built with [`native-windows-gui`](https://github.com/gabdube/native-windows-gui) — pure Win32
GDI, **no GPU required**. Works in virtual machines and headless environments.

---

## Features

- Live status display (RX/TX active, sample rate, channels, clock mode, uptime)
- Dante peer count from mDNS discovery
- **Reload Config** — sends `ReloadConfig` IPC message to the running service
- **Shutdown Service** — sends `Shutdown` IPC message and exits the GUI
- **View Logs** — opens the log directory in Explorer
- **Autostart** checkbox — adds/removes the service from the Windows registry run key
- Minimize to system tray on close (window hides, does not destroy)

---

## Architecture

```
inferno_gui (main thread — Win32 event loop)
│
├── nwg::Window + Labels + Buttons + AnimationTimer (2s tick)
│
└── Background tokio thread
    └── Every 2s: connect to \\.\pipe\inferno
        ├── Send {"type":"GetStatus"}\n
        └── Parse StatusMessage → update SharedStatus (Arc<Mutex<>>)

On timer tick (main thread):
    Read SharedStatus → update label text
    Read SharedCmd → send IPC command if button was pressed
```

The background thread is a `std::thread` running a `tokio` single-threaded runtime.
All GUI controls are `Rc<T>` — not `Send` — so they stay on the main thread.
Communication between threads uses `Arc<Mutex<>>`.

---

## IPC Protocol

Connects to the named pipe `\\.\pipe\inferno` (served by `inferno_wasapi`).

**Request/response** format: newline-delimited JSON.

```jsonc
// Request
{"type":"GetStatus"}\n

// Response
{"type":"Status","tx_active":false,"rx_active":true,"tx_channels":0,"rx_channels":2,"sample_rate":48000,"clock_mode":"SafeClock","tx_peak_db":[],"uptime_secs":137}\n
```

**Commands (fire-and-forget, no response expected):**
```jsonc
{"type":"ReloadConfig"}\n
{"type":"Shutdown"}\n
```

If the service is not running the pipe connection fails silently — the GUI shows "Service: Offline".

---

## Building

```powershell
cargo build -p inferno_gui
```

Produces `target\debug\inferno_gui.exe`. The `#![windows_subsystem = "windows"]` attribute
suppresses the console window on launch.

---

## Running

Start the `inferno_wasapi` service first, then:

```powershell
.\target\debug\inferno_gui.exe
```

The GUI polls IPC every 2 seconds and updates automatically.

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `native-windows-gui 1.0.13` | Win32 window, controls, event loop |
| `tokio` | Async runtime for the IPC background thread |
| `serde` / `serde_json` | IPC message serialisation |
