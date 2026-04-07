//! inferno2pipe — pipe InfernoAoIP Dante audio to stdout as raw PCM.
//! 
//! Usage:
//!   inferno2pipe [--device <name>] [--channels <n>] [--rate <hz>]
//!   
//! Output: raw signed 16-bit PCM, little-endian, interleaved channels
//! 
//! Example (pipe to FFmpeg):
//!   inferno2pipe | ffmpeg -f s16le -ar 48000 -ac 2 -i pipe:0 output.wav

use anyhow::Result;
use clap::Parser;

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

fn main() -> Result<()> {
    let args = Args::parse();
    
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)  // logs to stderr, audio to stdout
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();
    
    tracing::info!("inferno2pipe starting: {}ch @ {}Hz", args.channels, args.rate);
    
    // Placeholder: This is a stub implementation.
    // Full WASAPI implementation requires using windows-rs with proper COM activation.
    // For now, we log the parameters and exit successfully.
    
    tracing::info!("audio capture stub (loopback) — would write PCM to stdout");
    tracing::info!("Full WASAPI implementation pending");
    
    Ok(())
}
