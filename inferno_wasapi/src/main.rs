//! inferno_wasapi — Dante AoIP receiver for Windows via WASAPI
//!
//! Receives audio from a Dante network and plays it through Windows audio
//! devices using WASAPI (Windows Audio Session API).
//!
//! Architecture:
//! - inferno_aoip drives timing via a 50ms callback (receive_with_callback)
//! - A shared audio queue (VecDeque) bridges the async callback to a dedicated
//!   WASAPI render thread
//! - The WASAPI thread runs event-driven (WaitForSingleObject) on its own OS
//!   thread so it never blocks the tokio executor

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use clap::Parser;
use log::{info, warn};

use inferno_aoip::device_server::{DeviceServer, Settings};

#[derive(Parser, Debug)]
#[command(name = "inferno_wasapi", about = "Dante AoIP receiver for Windows via WASAPI")]
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
) {
    use wasapi::*;
    let _ = initialize_mta();

    let device = match open_wasapi_device(&device_filter, virtual_device) {
        Ok(d) => d,
        Err(e) => { ready_tx.send(Err(e.to_string())).ok(); return; }
    };
    let device_name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());

    let mut audio_client = match device.get_iaudioclient() {
        Ok(c) => c,
        Err(e) => { ready_tx.send(Err(format!("get_iaudioclient: {e}"))).ok(); return; }
    };

    // WASAPI Shared mode mix format is always f32 on modern Windows/Realtek.
    // inferno_aoip delivers 24-bit samples LEFT-JUSTIFIED in i32 (top 24 bits,
    // bottom 8 bits = 0), so the range is ±2^31. Divide by 2^31 to get f32 [-1, 1].
    let format = WaveFormat::new(32, 32, &SampleType::Float, sample_rate as usize, channels, None);
    let blockalign = format.get_blockalign() as usize;

    let (def_period, _min_period) = match audio_client.get_periods() {
        Ok(p) => p,
        Err(e) => { ready_tx.send(Err(format!("get_periods: {e}"))).ok(); return; }
    };

    if let Err(e) = audio_client.initialize_client(&format, def_period, &Direction::Render, &ShareMode::Shared, true) {
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

    info!("WASAPI render thread started: {device_name} (blockalign={blockalign}, format=f32)");
    ready_tx.send(Ok(device_name)).ok();

    while !shutdown.load(Ordering::Relaxed) {
        match h_event.wait_for_event(200) {
            Err(_) => continue, // timeout, loop and check shutdown
            Ok(()) => {}
        }

        let frames = match audio_client.get_available_space_in_frames() {
            Ok(n) if n > 0 => n as usize,
            Ok(_) => continue,
            Err(e) => { warn!("get_available_space_in_frames: {e}"); continue; }
        };

        let needed = frames * channels;
        // Convert from i32 (24-bit left-justified, range ±2^31) to f32 [-1.0, 1.0]
        let mut pcm_f32 = vec![0.0f32; needed];
        {
            let mut q = audio_queue.lock().unwrap();
            for s in pcm_f32.iter_mut() {
                let raw = q.pop_front().unwrap_or(0);
                *s = raw as f32 / 2147483648.0_f32;
            }
        }

        let bytes: Vec<u8> = pcm_f32.iter().flat_map(|s| s.to_le_bytes()).collect();
        if let Err(e) = render_client.write_to_device(frames, &bytes, None) {
            warn!("write_to_device: {e}");
        }
    }

    audio_client.stop_stream().ok();
    info!("WASAPI render thread stopped");
}

#[tokio::main]
async fn main() -> Result<()> {
    let logenv = env_logger::Env::default().default_filter_or("info");
    env_logger::init_from_env(logenv);

    let args = Args::parse();

    if args.list_devices {
        return list_wasapi_devices();
    }

    info!("Starting inferno_wasapi");

    let device_name = args.name.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "inferno-wasapi".to_owned())
    });

    let mut config: BTreeMap<String, String> = BTreeMap::new();
    config.insert("RX_CHANNELS".to_owned(), args.channels.to_string());
    config.insert("SAMPLE_RATE".to_owned(), args.sample_rate.to_string());
    config.insert("NAME".to_owned(), device_name.clone());

    let settings = Settings::new("inferno_wasapi", "InfernoWASPI", None, &config);

    info!("Device name: {}", settings.self_info.friendly_hostname);
    info!("IP: {}", settings.self_info.ip_address);
    info!("Channels: {}", args.channels);
    info!("Sample rate: {} Hz", args.sample_rate);

    // Shared audio queue: Dante callback -> WASAPI render thread
    // Capped at 2 seconds to prevent unbounded growth if WASAPI stalls
    let audio_queue: Arc<Mutex<VecDeque<i32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(args.sample_rate as usize * args.channels * 2)));
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
    let wasapi_thread = std::thread::Builder::new()
        .name("wasapi-render".into())
        .spawn(move || {
            wasapi_render_thread(device_filter, virtual_device, sample_rate, channels, audio_queue_wasapi, shutdown_wasapi, ready_tx);
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

    let channels_cb = args.channels;
    let max_queue = args.sample_rate as usize * args.channels * 2; // 2-second cap

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

    info!("Streaming Dante -> WASAPI. Press Ctrl+C to stop.");

    tokio::signal::ctrl_c().await?;
    info!("Ctrl+C received, shutting down");

    shutdown.store(true, Ordering::Relaxed);
    server.stop_receiver().await;
    server.shutdown().await;
    wasapi_thread.join().ok();
    info!("Shutdown complete");
    Ok(())
}
