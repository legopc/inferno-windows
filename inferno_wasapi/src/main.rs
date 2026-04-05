//! inferno_wasapi — Dante AoIP receiver for Windows via WASAPI
//!
//! Receives audio from a Dante network and plays it through Windows audio
//! devices using WASAPI (Windows Audio Session API).

use std::collections::BTreeMap;

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

    /// Name of the WASAPI device to use (substring match; default: system default)
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
    initialize_mta().ok();
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

#[tokio::main]
async fn main() -> Result<()> {
    let logenv = env_logger::Env::default().default_filter_or("info");
    env_logger::init_from_env(logenv);

    let args = Args::parse();

    if args.list_devices {
        return list_wasapi_devices();
    }

    info!("Starting inferno_wasapi");

    // Build Dante device settings
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

    // Start the Dante device server
    let mut server = DeviceServer::start(settings).await;
    info!("Dante device server started -- device should appear in Dante Controller");

    // Open WASAPI output device
    use wasapi::*;
    initialize_mta().ok();

    let device = if let Some(ref dev_name) = args.device {
        let collection = DeviceCollection::new(&Direction::Render)
            .map_err(|e| anyhow!("Failed to enumerate WASAPI render devices: {e}"))?;
        let mut found = None;
        for device_result in &collection {
            if let Ok(d) = device_result {
                if let Ok(name) = d.get_friendlyname() {
                    if name.contains(dev_name.as_str()) {
                        found = Some(d);
                        break;
                    }
                }
            }
        }
        found.ok_or_else(|| anyhow!("No WASAPI device found matching '{dev_name}'"))?
    } else {
        get_default_device(&Direction::Render)
            .map_err(|e| anyhow!("Failed to get default WASAPI device: {e}"))?
    };

    let device_name_str = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());
    info!("Using WASAPI device: {device_name_str}");

    let mut audio_client = device.get_iaudioclient()
        .map_err(|e| anyhow!("Failed to get IAudioClient: {e}"))?;

    let desired_format = WaveFormat::new(
        32,               // bits per sample
        32,               // valid bits per sample
        &SampleType::Int,
        args.sample_rate as usize,
        args.channels,
        None,
    );

    let (def_time, _min_time) = audio_client.get_periods()
        .map_err(|e| anyhow!("Failed to get device periods: {e}"))?;

    audio_client
        .initialize_client(
            &desired_format,
            def_time,
            &Direction::Render,
            &ShareMode::Shared,
            true,
        )
        .map_err(|e| anyhow!("Failed to initialize WASAPI audio client: {e}"))?;

    let blockalign = desired_format.get_blockalign() as usize;
    let render_client = audio_client.get_audiorenderclient()
        .map_err(|e| anyhow!("Failed to get AudioRenderClient: {e}"))?;
    let h_event = audio_client.set_get_eventhandle()
        .map_err(|e| anyhow!("Failed to create WASAPI event handle: {e}"))?;
    audio_client.start_stream()
        .map_err(|e| anyhow!("Failed to start WASAPI stream: {e}"))?;
    info!("WASAPI stream started (blockalign={blockalign} bytes/frame)");

    // Subscribe to Dante audio receiver
    let mut rt_receiver = server.receive_realtime().await;

    // Track our position in sample-time
    let mut current_timestamp: inferno_aoip::device_server::Clock = 0;
    let mut clock_synced = false;

    // Signal handler for graceful shutdown
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    info!("Streaming Dante -> WASAPI. Press Ctrl+C to stop.");

    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                info!("Ctrl+C received, shutting down");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(1)) => {
                // Sync timestamp from media clock when available
                if !clock_synced {
                    let media_clock = rt_receiver.clock();
                    if let Some(ts) = media_clock.wrapping_now_in_timebase(args.sample_rate as u64) {
                        current_timestamp = ts;
                        clock_synced = true;
                        info!("Media clock synchronized, starting at timestamp {ts}");
                    }
                }

                // Ask WASAPI how many frames we can write
                let frames_available = match audio_client.get_available_space_in_frames() {
                    Ok(n) if n > 0 => n as usize,
                    Ok(_) => continue,
                    Err(e) => {
                        warn!("get_available_space_in_frames error: {e}");
                        continue;
                    }
                };

                // Build interleaved i32 audio buffer
                let mut pcm = vec![0i32; frames_available * args.channels];

                if clock_synced {
                    for ch in 0..args.channels {
                        let mut ch_buf = vec![0i32; frames_available];
                        rt_receiver.get_samples(current_timestamp, ch, &mut ch_buf);
                        for (frame, sample) in ch_buf.into_iter().enumerate() {
                            pcm[frame * args.channels + ch] = sample;
                        }
                    }
                }

                // Convert i32 samples to bytes and write to WASAPI
                let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
                if let Err(e) = render_client.write_to_device(frames_available, &bytes, None) {
                    warn!("write_to_device error: {e}");
                    continue;
                }

                current_timestamp = current_timestamp.wrapping_add(frames_available);

                // Wait for WASAPI buffer-ready event
                if let Err(e) = h_event.wait_for_event(100) {
                    warn!("WASAPI event wait error: {e}");
                }
            }
        }
    }

    audio_client.stop_stream().ok();
    server.stop_receiver().await;
    server.shutdown().await;
    info!("Shutdown complete");
    Ok(())
}
