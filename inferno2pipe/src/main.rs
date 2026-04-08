//! inferno2pipe — pipe InfernoAoIP Dante audio to stdout as raw PCM.
//! 
//! Usage:
//!   inferno2pipe [--device <name>] [--channels <n>] [--rate <hz>]
//!   
//! Output: raw signed 16-bit PCM, little-endian, interleaved channels
//! 
//! Example (pipe to FFmpeg):
//!   inferno2pipe | ffmpeg -f s16le -ar 48000 -ac 2 -i pipe:0 output.wav

use anyhow::{anyhow, Result};
use clap::Parser;
use std::io::Write;
use wasapi::*;

#[derive(Parser, Debug)]
#[command(about = "Pipe InfernoAoIP audio to stdout as raw PCM")]
struct Args {
    /// WASAPI device name to capture from (default: default loopback device)
    #[arg(long, default_value = "")]
    device: String,
    
    /// Number of channels
    #[arg(long, default_value_t = 2)]
    channels: u16,
    
    /// Sample rate in Hz
    #[arg(long, default_value_t = 48000)]
    rate: u32,
    
    /// Duration in seconds (0 = infinite)
    #[arg(long, default_value_t = 0)]
    duration: u64,
}

fn open_wasapi_device(device_name_filter: &Option<String>) -> Result<Device> {
    let _ = initialize_mta();
    
    if let Some(ref filter) = device_name_filter {
        if !filter.is_empty() {
            // Find first device whose name contains the filter string
            let collection = DeviceCollection::new(&Direction::Render)
                .map_err(|e| anyhow!("Failed to enumerate WASAPI devices: {e}"))?;
            
            for dev_result in &collection {
                if let Ok(device) = dev_result {
                    if let Ok(name) = device.get_friendlyname() {
                        if name.contains(filter.as_str()) {
                            tracing::info!("Using WASAPI device: {}", name);
                            return Ok(device);
                        }
                    }
                }
            }
            tracing::warn!("Device filter '{}' not found; using default", filter);
        }
    }
    
    // Use default render (loopback) device
    let device = get_default_device(&Direction::Render)
        .map_err(|e| anyhow!("Failed to get default WASAPI device: {e}"))?;
    let name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());
    tracing::info!("Using default WASAPI loopback device: {}", name);
    Ok(device)
}

fn capture_loop(channels: u16, _rate: u32, duration_secs: u64, device_name_filter: Option<String>) -> Result<()> {
    let device = open_wasapi_device(&device_name_filter)?;
    let device_name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());

    let mut audio_client = device.get_iaudioclient()
        .map_err(|e| anyhow!("get_iaudioclient: {e}"))?;

    // Get the device's mix format
    let mix_fmt = audio_client.get_mixformat()
        .map_err(|e| anyhow!("get_mixformat: {e}"))?;
    
    tracing::info!(
        "WASAPI loopback mix format: {}bit {}Hz {}ch",
        mix_fmt.get_bitspersample(),
        mix_fmt.get_samplespersec(),
        mix_fmt.get_nchannels()
    );

    let blockalign = mix_fmt.get_blockalign() as usize;
    let dev_channels = mix_fmt.get_nchannels() as usize;
    let dev_rate = mix_fmt.get_samplespersec();
    let bytes_per_sample = if dev_channels > 0 { blockalign / dev_channels } else { 4 };
    let is_float = mix_fmt
        .get_subformat()
        .map(|s| matches!(s, SampleType::Float))
        .unwrap_or(true);

    // Initialize for loopback: Direction::Capture on a render device triggers loopback.
    // Use the device's native format and period 0 for shared mode.
    audio_client.initialize_client(
        &mix_fmt,
        0,
        &Direction::Capture,
        &ShareMode::Shared,
        true,  // loopback flag
    )
    .map_err(|e| anyhow!("initialize_client (loopback): {e}"))?;

    let capture_client = audio_client.get_audiocaptureclient()
        .map_err(|e| anyhow!("get_audiocaptureclient: {e}"))?;
    let h_event = audio_client.set_get_eventhandle()
        .map_err(|e| anyhow!("set_get_eventhandle: {e}"))?;
    audio_client.start_stream()
        .map_err(|e| anyhow!("start_stream: {e}"))?;

    tracing::info!(
        "WASAPI loopback capture started: {} ({}ch, {}bit, {}Hz, {})",
        device_name,
        dev_channels,
        mix_fmt.get_bitspersample(),
        dev_rate,
        if is_float { "float" } else { "int" }
    );

    let start = std::time::Instant::now();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let mut frames_written: u64 = 0;
    let mut events: u64 = 0;
    let mut last_stats = std::time::Instant::now();

    loop {
        if duration_secs > 0 && start.elapsed().as_secs() >= duration_secs {
            break;
        }

        match h_event.wait_for_event(200) {
            Err(_) => {
                // timeout — periodic stats log
                if last_stats.elapsed().as_secs() >= 5 {
                    tracing::info!("WASAPI loopback: alive (events={}, frames written={}, {}s elapsed)",
                        events, frames_written, start.elapsed().as_secs());
                    last_stats = std::time::Instant::now();
                }
                continue;
            }
            Ok(()) => {}
        }
        events += 1;

        // Drain all packets for this event
        loop {
            let nbr_frames = match capture_client.get_next_nbr_frames() {
                Ok(Some(n)) if n > 0 => n as usize,
                Ok(_) => break,
                Err(e) => {
                    tracing::warn!("get_next_nbr_frames: {}", e);
                    break;
                }
            };

            let buf_size = nbr_frames * blockalign;
            let mut data = vec![0u8; buf_size];
            
            let (frames_read, flags) = match capture_client.read_from_device(&mut data) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("read_from_device: {}", e);
                    break;
                }
            };

            if flags.silent {
                tracing::debug!("WASAPI loopback: silent buffer");
            }

            if frames_read == 0 {
                break;
            }
            let frames_read = frames_read as usize;
            frames_written += frames_read as u64;

            // Convert captured device samples to i16 PCM (16-bit, little-endian, interleaved)
            // For simplicity, we:
            // 1. Take only the first `channels` channels (pad or discard as needed)
            // 2. Convert everything to i16 (downsample from float, or truncate from 24/32-bit)
            // 3. Write interleaved samples to stdout

            let mut out_buffer = Vec::new();
            for frame in 0..frames_read {
                for ch in 0..channels as usize {
                    let sample_i32 = if ch < dev_channels {
                        let offset = frame * blockalign + ch * bytes_per_sample;
                        if offset + bytes_per_sample <= data.len() {
                            let slice = &data[offset..offset + bytes_per_sample];
                            if is_float && bytes_per_sample == 4 {
                                // f32 -> i32 (range: [-1, 1] -> [-32767, 32767])
                                let f = f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
                                (f.clamp(-1.0, 1.0) * 32767.0_f32) as i32
                            } else {
                                // PCM int: read bytes, left-shift to fill i32
                                let mut bytes4 = [0u8; 4];
                                let copy_len = bytes_per_sample.min(4);
                                bytes4[..copy_len].copy_from_slice(&slice[..copy_len]);
                                let shift = 32usize.saturating_sub(bytes_per_sample * 8);
                                (i32::from_le_bytes(bytes4) >> shift) >> 16
                            }
                        } else {
                            0
                        }
                    } else {
                        0 // pad missing channels with silence
                    };
                    // Clamp to i16 range
                    let sample_i16 = sample_i32.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                    out_buffer.extend_from_slice(&sample_i16.to_le_bytes());
                }
            }

            out.write_all(&out_buffer)
                .map_err(|e| anyhow!("Failed to write to stdout: {e}"))?;
        }
    }

    audio_client.stop_stream().ok(); // best-effort cleanup
    tracing::info!(
        "WASAPI loopback stopped after {:.1}s ({} frames written)",
        start.elapsed().as_secs_f64(),
        frames_written
    );

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)  // logs to stderr, audio to stdout
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!(
        "inferno2pipe starting: {}ch @ {}Hz, duration={}s",
        args.channels, args.rate, args.duration
    );

    let device_filter = if args.device.is_empty() {
        None
    } else {
        Some(args.device)
    };

    capture_loop(args.channels, args.rate, args.duration, device_filter)
}
