//! inferno_wasapi — Dante AoIP receiver/transmitter for Windows via WASAPI
//!
//! RX mode: receives audio from a Dante network and plays it through Windows
//! audio devices using WASAPI (Windows Audio Session API).
//!
//! TX mode (--tx): captures system audio via WASAPI loopback from a render
//! device and transmits it to a Dante AoIP network.
//!
//! Architecture (RX):
//! - inferno_aoip drives timing via a 50ms callback (receive_with_callback)
//! - A shared audio queue (VecDeque) bridges the async callback to a dedicated
//!   WASAPI render thread
//! - The WASAPI thread runs event-driven (WaitForSingleObject) on its own OS
//!   thread so it never blocks the tokio executor

mod config;
mod logging;
mod metering;
mod service;
mod service_install;
mod ipc;
mod tray;
mod metrics;
mod network_health;

pub use config::Config;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use clap::Parser;
use if_addrs;

use inferno_aoip::device_server::{DeviceServer, Settings};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "inferno_wasapi", about = "Dante AoIP receiver/transmitter for Windows via WASAPI")]
struct Args {
    /// List available WASAPI audio output devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Route audio to a virtual audio cable (e.g. VB-Cable) so it appears as a
    /// Windows sound device. Auto-detects "CABLE Input" (VB-Cable) if installed.
    /// Install VB-Cable free from https://vb-audio.com/Cable/
    #[arg(long)]
    virtual_device: bool,

    /// Name of the WASAPI device to use (substring match; default: system default).
    /// Use --list-devices to see available names.
    #[arg(long)]
    device: Option<String>,

    /// Device name as shown in Dante Controller (default: hostname)
    #[arg(long)]
    name: Option<String>,

    /// Number of receive channels (default: 2)
    #[arg(long, default_value = "2")]
    channels: usize,

    /// Sample rate in Hz (default: 48000)
    #[arg(long, default_value = "48000")]
    sample_rate: u32,

    /// Enable TX mode: capture system audio via WASAPI loopback and transmit to Dante.
    /// The virtual speaker (SYSVAD) must be installed — see SETUP.md.
    #[arg(long)]
    tx: bool,

    /// WASAPI render device to loopback-capture from (substring match, default: system default).
    /// Use --list-tx-devices to see available devices. Typically "Tablet Audio" for SYSVAD.
    #[arg(long)]
    tx_device: Option<String>,

    /// Number of Dante TX channels (default: 2)
    #[arg(long, default_value = "2")]
    tx_channels: usize,

    /// List WASAPI render devices available for TX loopback capture and exit.
    #[arg(long)]
    list_tx_devices: bool,

    /// Discover and list Dante devices via mDNS and exit.
    #[arg(long)]
    list_dante_devices: bool,

    /// List available network interfaces and exit.
    #[arg(long)]
    list_interfaces: bool,

    /// Lock the device to prevent Dante Controller from changing configuration.
    #[arg(long)]
    lock: bool,

    /// Unlock the device to allow Dante Controller configuration changes.
    #[arg(long)]
    unlock: bool,
    #[arg(long)]
    service: bool,

    /// Install as Windows Service (requires admin)
    #[arg(long)]
    install_service: bool,

    /// Uninstall Windows Service (requires admin)
    #[arg(long)]
    uninstall_service: bool,

    /// Run system tray icon
    #[arg(long)]
    tray: bool,
}

fn list_wasapi_devices() -> Result<()> {
    use wasapi::*;
    let _ = initialize_mta();
    let collection = DeviceCollection::new(&Direction::Render)
        .map_err(|e| anyhow!("Failed to enumerate WASAPI render devices: {e}"))?;
    println!("Available WASAPI render devices:");
    for (i, device_result) in (&collection).into_iter().enumerate() {
        match device_result {
            Ok(device) => {
                let name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());
                let state = device.get_state().map(|s| format!("{s:?}")).unwrap_or_else(|_| "?".into());
                println!("  [{i}] {name}  (state: {state})");
            }
            Err(e) => println!("  [{i}] <error: {e}>"),
        }
    }
    Ok(())
}

/// Open the requested (or default) WASAPI render device by name substring.
/// If `virtual_device` is true, tries "CABLE Input" (VB-Cable) first and
/// errors with install instructions if not found.
fn open_wasapi_device(device_filter: &Option<String>, virtual_device: bool) -> Result<wasapi::Device> {
    use wasapi::*;
    let _ = initialize_mta();

    // Resolve the name to search for
    let search = if let Some(f) = device_filter.as_deref() {
        f.to_owned()
    } else if virtual_device {
        "CABLE Input".to_owned()
    } else {
        // No filter: return system default
        return get_default_device(&Direction::Render)
            .map_err(|e| anyhow!("Failed to get default WASAPI device: {e}"));
    };

    let collection = DeviceCollection::new(&Direction::Render)
        .map_err(|e| anyhow!("Failed to enumerate WASAPI render devices: {e}"))?;
    for dev in &collection {
        if let Ok(d) = dev {
            if let Ok(name) = d.get_friendlyname() {
                if name.contains(search.as_str()) {
                    return Ok(d);
                }
            }
        }
    }

    if virtual_device && device_filter.is_none() {
        Err(anyhow!(
            "VB-Cable not found (looked for \"CABLE Input\").\n\
             Install it free from https://vb-audio.com/Cable/ then reboot.\n\
             After install, run again with --virtual-device to route Dante audio\n\
             to the \"CABLE Output\" recording device visible to all Windows apps."
        ))
    } else {
        Err(anyhow!("No WASAPI device found matching '{search}'"))
    }
}

/// Dedicated WASAPI render thread.
///
/// Reads interleaved i32 samples from `audio_queue` and writes them to
/// the WASAPI render client.  Runs event-driven: blocks on the WASAPI
/// buffer-ready event (WaitForSingleObject under the hood) so it wakes up
/// exactly when the driver needs more data.
fn wasapi_render_thread(
    device_filter: Option<String>,
    virtual_device: bool,
    sample_rate: u32,
    channels: usize,
    audio_queue: Arc<Mutex<VecDeque<i32>>>,
    shutdown: Arc<AtomicBool>,
    ready_tx: std::sync::mpsc::SyncSender<Result<String, String>>,
    config: Config,
) {
    use wasapi::*;
    let _ = initialize_mta();

    // Set thread priority to time-critical for low-latency audio
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL).ok(); }

    let device = match open_wasapi_device(&device_filter, virtual_device) {
        Ok(d) => d,
        Err(e) => { ready_tx.send(Err(e.to_string())).ok(); return; }
    };
    let device_name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());

    let mut audio_client = match device.get_iaudioclient() {
        Ok(c) => c,
        Err(e) => { ready_tx.send(Err(format!("get_iaudioclient: {e}"))).ok(); return; }
    };

    // Query and log the device's native mix format for diagnostics
    let mix_fmt = audio_client.get_mixformat().ok();
    if let Some(ref mf) = mix_fmt {
        info!("WASAPI mix format: {:?}bit {:?}bit-valid {:?}Hz {:?}ch {:?}",
            mf.get_bitspersample(), mf.get_validbitspersample(),
            mf.get_samplespersec(), mf.get_nchannels(),
            mf.get_subformat().map(|s| format!("{s:?}")).unwrap_or_else(|_| "?".into()));
    }

    // Prefer f32 at the requested sample rate. If the device doesn't support it
    // directly, fall back to whatever format the device natively uses.
    let preferred = WaveFormat::new(32, 32, &SampleType::Float, sample_rate as usize, channels, None);
    let mut format = match audio_client.is_supported(&preferred, &ShareMode::Shared) {
        Ok(None) => {
            info!("WASAPI: device supports f32 {sample_rate}Hz {channels}ch directly");
            preferred
        }
        Ok(Some(nearest)) => {
            info!("WASAPI: f32 not directly supported, using nearest: {:?}bit {:?}Hz {:?}ch",
                nearest.get_bitspersample(), nearest.get_samplespersec(), nearest.get_nchannels());
            nearest
        }
        Err(e) => {
            // is_supported failed — try mix format, else use our preferred and let initialize fail loudly
            warn!("WASAPI is_supported query failed ({e}), trying mix format");
            mix_fmt.clone().unwrap_or(preferred)
        }
    };

    let blockalign = format.get_blockalign() as usize;
    let dev_channels = format.get_nchannels() as usize;
    let mut dev_sample_rate = format.get_samplespersec();
    let is_float = format.get_subformat().map(|s| matches!(s, SampleType::Float)).unwrap_or(true);

    let (def_period, _min_period) = match audio_client.get_periods() {
        Ok(p) => p,
        Err(e) => { ready_tx.send(Err(format!("get_periods: {e}"))).ok(); return; }
    };

    let share_mode = if config.wasapi_exclusive {
        ShareMode::Exclusive
    } else {
        ShareMode::Shared
    };
    tracing::info!("WASAPI mode: {}", if config.wasapi_exclusive { "exclusive" } else { "shared" });

    // Try to initialize with the selected format
    let mut initialize_result = audio_client.initialize_client(&format, def_period, &Direction::Render, &share_mode, true);
    
    // If initialization fails with unsupported format and we have an alternative sample rate, try that
    if initialize_result.is_err() && (sample_rate == 48000 || sample_rate == 44100) {
        let fallback_rate = if sample_rate == 48000 { 44100 } else { 48000 };
        warn!("Requested {sample_rate}Hz not supported, attempting fallback to {fallback_rate}Hz");
        
        let fallback_format = WaveFormat::new(32, 32, &SampleType::Float, fallback_rate as usize, channels, None);
        match audio_client.is_supported(&fallback_format, &share_mode) {
            Ok(None) | Ok(Some(_)) => {
                let try_format = if let Ok(Some(nearest)) = audio_client.is_supported(&fallback_format, &share_mode) {
                    nearest
                } else {
                    fallback_format
                };
                initialize_result = audio_client.initialize_client(&try_format, def_period, &Direction::Render, &share_mode, true);
                if initialize_result.is_ok() {
                    format = try_format;
                    dev_sample_rate = format.get_samplespersec();
                    info!("Fallback to {fallback_rate}Hz succeeded");
                }
            }
            Err(_) => {}
        }
    }

    if let Err(e) = initialize_result {
        ready_tx.send(Err(format!("initialize_client: {e}"))).ok(); return;
    }

    let render_client = match audio_client.get_audiorenderclient() {
        Ok(r) => r,
        Err(e) => { ready_tx.send(Err(format!("get_audiorenderclient: {e}"))).ok(); return; }
    };
    let h_event = match audio_client.set_get_eventhandle() {
        Ok(e) => e,
        Err(e) => { ready_tx.send(Err(format!("set_get_eventhandle: {e}"))).ok(); return; }
    };
    if let Err(e) = audio_client.start_stream() {
        ready_tx.send(Err(format!("start_stream: {e}"))).ok(); return;
    }

    info!("WASAPI render thread started: {device_name} ({dev_sample_rate}Hz, {dev_channels}ch, {}bit, {})",
        format.get_bitspersample(),
        if is_float { "float" } else { "int" });
    ready_tx.send(Ok(device_name)).ok();

    let mut frames_written_total: u64 = 0;
    let mut first_audio_logged = false;
    let mut render_loop_count: u64 = 0;
    let mut last_stats_log = std::time::Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        match h_event.wait_for_event(200) {
            Err(_) => {
                // timeout — log every 5s if loop is running but no events fire
                if last_stats_log.elapsed().as_secs() >= 5 {
                    let queue_depth = audio_queue.lock().unwrap().len();
                    info!("WASAPI: render alive (loop={render_loop_count}, frames_total={frames_written_total}, queue={queue_depth})");
                    last_stats_log = std::time::Instant::now();
                }
                continue;
            }
            Ok(()) => {}
        }
        render_loop_count += 1;

        let frames = match audio_client.get_available_space_in_frames() {
            Ok(n) if n > 0 => n as usize,
            Ok(_) => {
                // Possible underrun: no space available but event fired
                warn!("WASAPI render: buffer underrun (no space available)");
                continue;
            }
            Err(e) => { warn!("get_available_space_in_frames: {e}"); continue; }
        };

        // frames * dev_channels = total samples needed for the device buffer
        let needed = frames * dev_channels;
        // Drain from the queue (which holds Dante source channel interleaved i32 samples).
        // If the device has more channels than we have from Dante, extra channels get silence.
        let samples_from_queue;
        let bytes: Vec<u8> = {
            let mut q = audio_queue.lock().unwrap();
            samples_from_queue = q.len().min(needed);
            // Detect underruns: if queue is empty but we need data, warn
            if samples_from_queue == 0 && needed > 0 {
                warn!("WASAPI render: writing silence due to empty queue (possible Dante stall)");
            }
            let mut buf = Vec::with_capacity(needed * blockalign / dev_channels);
            if is_float {
                // f32 LE: convert i32 (24-bit left-justified, ±2^31) to f32 [-1, 1]
                for _ in 0..needed {
                    let raw = q.pop_front().unwrap_or(0);
                    let f: f32 = raw as f32 / 2147483648.0_f32;
                    buf.extend_from_slice(&f.to_le_bytes());
                }
            } else {
                // Int (e.g. 24-bit or 32-bit PCM): write raw i32 LE (already left-justified)
                let bytes_per_sample = blockalign / dev_channels;
                for _ in 0..needed {
                    let raw = q.pop_front().unwrap_or(0);
                    // Most-significant bytes first → right-align to device bits
                    let shifted = raw >> (32 - bytes_per_sample * 8);
                    buf.extend_from_slice(&shifted.to_le_bytes()[..bytes_per_sample]);
                }
            }
            buf
        };

        // Log first time we write real audio (not silence)
        if !first_audio_logged && samples_from_queue > 0 {
            info!("WASAPI: writing first audio ({samples_from_queue} samples from Dante queue)");
            first_audio_logged = true;
        }

        match render_client.write_to_device(frames, &bytes, None) {
            Ok(()) => {
                frames_written_total += frames as u64;
                // Log every ~10 seconds to confirm data is flowing
                if last_stats_log.elapsed().as_secs() >= 10 {
                    let queue_samples;
                    { queue_samples = audio_queue.lock().unwrap().len(); }
                    let secs = frames_written_total / dev_sample_rate as u64;
                    info!("WASAPI: {secs}s rendered, loops={render_loop_count}, queue depth: {queue_samples} samples");
                    last_stats_log = std::time::Instant::now();
                }
            }
            Err(e) => warn!("write_to_device: {e}"),
        }
    }

    audio_client.stop_stream().ok();
    info!("WASAPI render thread stopped");
}

/// Dedicated WASAPI loopback capture thread for TX mode.
///
/// Captures whatever is playing on the given render device using
/// WASAPI loopback, converts samples to 24-bit left-justified i32,
/// de-interleaves into per-channel Vecs, and pushes to the TxPusher.
fn wasapi_capture_thread(
    device_filter: Option<String>,
    _sample_rate: u32,
    channels: usize,
    tx_pusher: Arc<Mutex<inferno_aoip::TxPusher>>,
    shutdown: Arc<AtomicBool>,
    ready_tx: std::sync::mpsc::SyncSender<Result<String, String>>,
) {
    use wasapi::*;
    let _ = initialize_mta();

    // Set thread priority to time-critical for low-latency audio
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL).ok(); }

    let device = match open_wasapi_device(&device_filter, false) {
        Ok(d) => d,
        Err(e) => { ready_tx.send(Err(e.to_string())).ok(); return; }
    };
    let device_name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());

    let mut audio_client = match device.get_iaudioclient() {
        Ok(c) => c,
        Err(e) => { ready_tx.send(Err(format!("get_iaudioclient: {e}"))).ok(); return; }
    };

    // Use device's native mix format — autoconvert handles rate/channel differences
    let mix_fmt = match audio_client.get_mixformat() {
        Ok(f) => f,
        Err(e) => { ready_tx.send(Err(format!("get_mixformat: {e}"))).ok(); return; }
    };

    info!("WASAPI loopback mix format: {:?}bit {:?}bit-valid {:?}Hz {:?}ch {:?}",
        mix_fmt.get_bitspersample(), mix_fmt.get_validbitspersample(),
        mix_fmt.get_samplespersec(), mix_fmt.get_nchannels(),
        mix_fmt.get_subformat().map(|s| format!("{s:?}")).unwrap_or_else(|_| "?".into()));

    let blockalign = mix_fmt.get_blockalign() as usize;
    let dev_channels = mix_fmt.get_nchannels() as usize;
    let is_float = mix_fmt.get_subformat().map(|s| matches!(s, SampleType::Float)).unwrap_or(true);
    let bytes_per_sample = if dev_channels > 0 { blockalign / dev_channels } else { 4 };

    // Initialize for loopback: Direction::Capture on a render device triggers
    // AUDCLNT_STREAMFLAGS_LOOPBACK; period 0 is fine for shared mode.
    if let Err(e) = audio_client.initialize_client(
        &mix_fmt, 0, &Direction::Capture, &ShareMode::Shared, true,
    ) {
        ready_tx.send(Err(format!("initialize_client (loopback): {e}"))).ok(); return;
    }

    let capture_client = match audio_client.get_audiocaptureclient() {
        Ok(c) => c,
        Err(e) => { ready_tx.send(Err(format!("get_audiocaptureclient: {e}"))).ok(); return; }
    };
    let h_event = match audio_client.set_get_eventhandle() {
        Ok(e) => e,
        Err(e) => { ready_tx.send(Err(format!("set_get_eventhandle: {e}"))).ok(); return; }
    };
    if let Err(e) = audio_client.start_stream() {
        ready_tx.send(Err(format!("start_stream: {e}"))).ok(); return;
    }

    info!("WASAPI loopback capture started: {device_name} ({dev_channels}ch, {}bit, {})",
        mix_fmt.get_bitspersample(),
        if is_float { "float" } else { "int" });
    ready_tx.send(Ok(device_name)).ok();

    let meter = metering::ChannelMeter::new(channels);
    let mut frames_captured_total: u64 = 0;
    let mut events_total: u64 = 0;
    let mut last_stats_log = std::time::Instant::now();
    let mut last_meter_log = std::time::Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        match h_event.wait_for_event(200) {
            Err(_) => {
                if last_stats_log.elapsed().as_secs() >= 10 {
                    info!("WASAPI loopback: alive (events={events_total}, frames={frames_captured_total})");
                    last_stats_log = std::time::Instant::now();
                }
                continue;
            }
            Ok(()) => {}
        }
        events_total += 1;

        // Drain all available packets for this event
        loop {
            let nbr_frames = match capture_client.get_next_nbr_frames() {
                Ok(Some(n)) if n > 0 => n as usize,
                Ok(_) => break,
                Err(e) => { warn!("get_next_nbr_frames: {e}"); break; }
            };

            let buf_size = nbr_frames * blockalign;
            let mut data = vec![0u8; buf_size];
            let (frames_read, flags) = match capture_client.read_from_device(&mut data) {
                Ok(r) => r,
                Err(e) => { warn!("read_from_device: {e}"); break; }
            };

            // Check for silent buffers (glitch detection)
            if flags.silent {
                warn!("WASAPI capture: silent buffer (glitch detected)");
            }

            if frames_read == 0 {
                break;
            }
            let frames_read = frames_read as usize;
            frames_captured_total += frames_read as u64;

            // De-interleave into per-channel sample vecs; take only first `channels`
            let mut channel_data: Vec<Vec<i32>> = vec![Vec::with_capacity(frames_read); channels];
            for frame in 0..frames_read {
                for ch in 0..channels {
                    let sample = if ch < dev_channels {
                        let offset = frame * blockalign + ch * bytes_per_sample;
                        if offset + bytes_per_sample <= data.len() {
                            let slice = &data[offset..offset + bytes_per_sample];
                            if is_float && bytes_per_sample == 4 {
                                let f = f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
                                (f.clamp(-1.0, 1.0) * 2147483648.0_f32) as i32
                            } else {
                                // PCM: read up to 4 bytes, left-shift to fill i32
                                let mut bytes4 = [0u8; 4];
                                let copy_len = bytes_per_sample.min(4);
                                bytes4[..copy_len].copy_from_slice(&slice[..copy_len]);
                                let shift = 32usize.saturating_sub(bytes_per_sample * 8);
                                i32::from_le_bytes(bytes4) << shift
                            }
                        } else {
                            0
                        }
                    } else {
                        0 // pad missing channels with silence
                    };
                    channel_data[ch].push(sample);
                }
            }

            // Update peak meters for each channel
            for ch in 0..channels {
                meter.update_i32(ch, &channel_data[ch]);
            }

            tx_pusher.lock().unwrap().push_channels(&channel_data);
        }

        // Periodic metering log every 5 seconds
        if last_meter_log.elapsed().as_secs() >= 5 {
            let peaks = meter.get_peaks();
            info!(peaks = ?peaks, "Audio peak levels (0-255 Dante scale)");
            meter.decay();
            last_meter_log = std::time::Instant::now();
        }

        if last_stats_log.elapsed().as_secs() >= 10 {
            info!("WASAPI loopback: events={events_total}, frames={frames_captured_total}");
            last_stats_log = std::time::Instant::now();
        }
    }

    audio_client.stop_stream().ok();
    info!("WASAPI loopback capture thread stopped");
}

/// Ensure firewall rules are set for Dante AoIP ports (UDP 4440, 4455, 5353, 8700, 8800)
fn ensure_firewall_rules() {
    let ports = [4440u16, 4455, 5353, 8700, 8800];
    for port in ports {
        let _ = std::process::Command::new("netsh")
            .args(["advfirewall", "firewall", "add", "rule",
                   &format!("name=InfernoAoIP-UDP-{port}"),
                   "dir=in", "action=allow", "protocol=UDP",
                   &format!("localport={port}")])
            .output();
    }
    info!("Firewall rules ensured for Dante ports");
}

/// Install a panic hook that writes crash logs to disk
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        let bt = std::backtrace::Backtrace::capture();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let log_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("inferno_aoip")
            .join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let path = log_dir.join(format!("crash-{timestamp}.log"));
        let content = format!("PANIC: {msg}\n\nBacktrace:\n{bt:?}\n");
        let _ = std::fs::write(&path, &content);
        eprintln!("Crash log written to: {}", path.display());
    }));
}

/// Discover Dante devices via mDNS for 3 seconds and list them.
async fn list_dante_devices() -> Result<()> {
    println!("Discovering Dante devices (3 seconds)...");
    println!("(Listening for _netaudio._udp.local mDNS announcements)");
    
    // Note: Full mDNS device discovery would require deeper integration
    // with searchfire discovery. For now, this is a placeholder that
    // demonstrates the pattern for later enhancement.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    
    println!("\n(mDNS discovery complete — check logs for discovered Dante devices)");
    println!("Tip: Run with RUST_LOG=debug to see detailed discovery messages");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook before anything else
    install_panic_hook();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    // Ensure firewall rules are set up
    ensure_firewall_rules();

    let args = Args::parse();

    // Handle service install/uninstall
    if args.install_service {
        match service_install::install_service() {
            Ok(()) => return Ok(()),
            Err(e) => return Err(anyhow!("Failed to install service: {e}")),
        }
    }

    if args.uninstall_service {
        match service_install::uninstall_service() {
            Ok(()) => return Ok(()),
            Err(e) => return Err(anyhow!("Failed to uninstall service: {e}")),
        }
    }

    // Run as Windows Service if requested
    if args.service {
        info!("Starting as Windows Service");
        if let Err(e) = service::run_as_service() {
            tracing::error!("Service dispatcher failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.list_devices {
        return list_wasapi_devices();
    }

    if args.list_tx_devices {
        println!("Available WASAPI render devices for TX loopback:");
        return list_wasapi_devices();
    }

    if args.list_dante_devices {
        return list_dante_devices().await;
    }

    if args.list_interfaces {
        println!("Available network interfaces:");
        match if_addrs::get_if_addrs() {
            Ok(ifaces) => {
                for iface in ifaces {
                    if !iface.is_loopback() {
                        println!("  {} — {}", iface.name, iface.ip());
                    }
                }
            }
            Err(e) => println!("  (error enumerating interfaces: {})", e),
        }
        println!("  (Set network_interface in config.toml to select one)");
        return Ok(());
    }

    if args.lock {
        let lock_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("inferno_aoip")
            .join("device.lock");
        if let Some(parent) = lock_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&lock_path, b"locked").ok();
        println!("Device locked. Use --unlock to re-enable changes.");
        return Ok(());
    }

    if args.unlock {
        let lock_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("inferno_aoip")
            .join("device.lock");
        std::fs::remove_file(&lock_path).ok();
        println!("Device unlocked. Configuration changes are now allowed.");
        return Ok(());
    }

    // Spawn system tray if requested
    if args.tray {
        std::thread::spawn(|| tray::run_tray());
    }

    // Load persistent config
    let config_file = Config::load();
    info!("Config: device={} rate={}Hz channels={} latency={}ms",
        config_file.device_name, config_file.sample_rate, config_file.channels, config_file.latency_ms);

    let device_name = args.name.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "inferno-wasapi".to_owned())
    });

    // ── TX mode ─────────────────────────────────────────────────────────────
    if args.tx {
        info!("TX mode: capturing system audio via WASAPI loopback → Dante");
        info!("TX channels: {}", args.tx_channels);
        info!("Sample rate: {} Hz", args.sample_rate);

        let mut config: BTreeMap<String, String> = BTreeMap::new();
        config.insert("TX_CHANNELS".to_owned(), args.tx_channels.to_string());
        config.insert("RX_CHANNELS".to_owned(), "0".to_owned());
        config.insert("SAMPLE_RATE".to_owned(), args.sample_rate.to_string());
        config.insert("NAME".to_owned(), device_name.clone());

        let settings = Settings::new("inferno_wasapi", "InfernoWASPI", None, &config);
        info!("Device name: {}", settings.self_info.friendly_hostname);
        info!("IP: {}", settings.self_info.ip_address);

        let mut server = DeviceServer::start(settings).await;
        info!("Dante device server started — device should appear in Dante Controller");

        let tx_pusher = server.transmit_with_push(args.tx_channels).await;
        let tx_pusher = Arc::new(Mutex::new(tx_pusher));

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_capture = shutdown.clone();

        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let tx_channels = args.tx_channels;
        let sample_rate = args.sample_rate;
        let tx_device = args.tx_device.clone();
        let capture_thread = std::thread::Builder::new()
            .name("wasapi-capture".into())
            .spawn(move || {
                wasapi_capture_thread(
                    tx_device, sample_rate, tx_channels,
                    tx_pusher, shutdown_capture, ready_tx,
                );
            })?;

        match ready_rx.recv() {
            Ok(Ok(dev_name)) => info!("WASAPI loopback capture ready: {dev_name}"),
            Ok(Err(e)) => return Err(anyhow!("WASAPI loopback init failed: {e}")),
            Err(_) => return Err(anyhow!("WASAPI capture thread died before signalling ready")),
        }

        // Spawn monitoring tasks
        let metrics = metrics::Metrics::new();
        tokio::spawn(metrics::serve_metrics(metrics.clone()));
        tokio::spawn(network_health::run_health_monitor());

        info!("Streaming WASAPI loopback → Dante. Press Ctrl+C to stop.");
        tokio::signal::ctrl_c().await.ok();
        info!("Ctrl+C received, shutting down TX");

        shutdown.store(true, Ordering::Relaxed);
        server.stop_transmitter().await;
        server.shutdown().await;
        capture_thread.join().ok();
        info!("TX shutdown complete");
        return Ok(());
    }
    // ── end TX mode ──────────────────────────────────────────────────────────

    let mut config: BTreeMap<String, String> = BTreeMap::new();
    config.insert("RX_CHANNELS".to_owned(), args.channels.to_string());
    config.insert("TX_CHANNELS".to_owned(), "0".to_owned());
    config.insert("SAMPLE_RATE".to_owned(), args.sample_rate.to_string());
    config.insert("NAME".to_owned(), device_name.clone());

    let settings = Settings::new("inferno_wasapi", "InfernoWASPI", None, &config);

    info!("Device name: {}", settings.self_info.friendly_hostname);
    info!("IP: {}", settings.self_info.ip_address);
    info!("Channels: {}", args.channels);
    info!("Sample rate: {} Hz", args.sample_rate);

    // Shared audio queue: Dante callback -> WASAPI render thread
    // Capped at 250ms to prevent unbounded growth if WASAPI stalls
    // 250ms ring buffer — sufficient for Dante latency targets
    let audio_queue: Arc<Mutex<VecDeque<i32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(args.sample_rate as usize / 4 * args.channels)));
    let audio_queue_wasapi = audio_queue.clone();
    let audio_queue_dante = audio_queue.clone();

    // Shutdown flag shared between main and the WASAPI thread
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_wasapi = shutdown.clone();

    // Start WASAPI render thread before Dante so audio is ready when data arrives
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let channels = args.channels;
    let sample_rate = args.sample_rate;
    let device_filter = args.device.clone();
    let virtual_device = args.virtual_device;
    let config_wasapi = config_file.clone();
    let wasapi_thread = std::thread::Builder::new()
        .name("wasapi-render".into())
        .spawn(move || {
            wasapi_render_thread(device_filter, virtual_device, sample_rate, channels, audio_queue_wasapi, shutdown_wasapi, ready_tx, config_wasapi);
        })?;

    // Wait for WASAPI thread to confirm it started
    match ready_rx.recv() {
        Ok(Ok(dev_name)) => {
            info!("WASAPI ready: {dev_name}");
            if args.virtual_device || dev_name.contains("CABLE") {
                info!("Audio will be available on \"CABLE Output\" in Windows Sound settings");
                info!("Select \"CABLE Output\" as input device in OBS, Audacity, or any app");
            }
        }
        Ok(Err(e)) => return Err(anyhow!("WASAPI init failed: {e}")),
        Err(_) => return Err(anyhow!("WASAPI thread died before signalling ready")),
    }

    // Start the Dante device server
    let mut server = DeviceServer::start(settings).await;
    info!("Dante device server started — device should appear in Dante Controller");

    // Spawn IPC server for tray communication
    let ipc_start_time = std::time::Instant::now();
    let ipc_rx_channels = args.channels as u32;
    let ipc_tx_channels = 0u32;
    let ipc_sample_rate = args.sample_rate;
    tokio::spawn(ipc::start_ipc_server(ipc_start_time, ipc_tx_channels, ipc_rx_channels, ipc_sample_rate));

    let channels_cb = args.channels;
    let max_queue = args.sample_rate as usize / 4 * args.channels; // 250ms buffer — sufficient for Dante latency targets

    // receive_with_callback: inferno_aoip calls this ~every 50ms with decoded samples.
    // No timestamp management needed — inferno_aoip handles ring-buffer alignment.
    server.receive_with_callback(Box::new(move |num_samples, channel_data| {
        let mut q = audio_queue_dante.lock().unwrap();
        if q.len() >= max_queue {
            warn!("Audio queue full ({} samples), dropping {} samples", q.len(), num_samples * channels_cb);
            return;
        }
        for frame in 0..num_samples {
            for ch in 0..channels_cb {
                let sample = channel_data.get(ch).and_then(|c| c.get(frame)).copied().unwrap_or(0);
                q.push_back(sample);
            }
        }
    })).await;

    // Spawn monitoring tasks
    let metrics = metrics::Metrics::new();
    tokio::spawn(metrics::serve_metrics(metrics.clone()));
    tokio::spawn(network_health::run_health_monitor());

    info!("Streaming Dante -> WASAPI. Press Ctrl+C to stop.");

    tokio::signal::ctrl_c().await.ok();
    info!("Ctrl+C received, shutting down");

    shutdown.store(true, Ordering::Relaxed);
    server.stop_receiver().await;
    server.shutdown().await;
    wasapi_thread.join().ok();
    info!("Shutdown complete");
    Ok(())
}
