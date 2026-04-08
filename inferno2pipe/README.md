# inferno2pipe

Pipe Dante AoIP audio from the default Windows render device to **stdout** as raw PCM.

Useful for integrating Dante audio with other tools (FFmpeg, SoX, custom DSP pipelines)
without needing a virtual audio device driver.

---

## How It Works

`inferno2pipe` opens the Windows default **render** device (or a named device) in WASAPI
**loopback** mode. Loopback capture records everything playing through that device — including
audio received from a Dante network by the running `inferno_wasapi` service.

Captured audio is converted to **signed 16-bit PCM, little-endian, interleaved** and written
to stdout. This format is compatible with most audio tools.

---

## Usage

```powershell
# Pipe to FFmpeg → WAV file
inferno2pipe | ffmpeg -f s16le -ar 48000 -ac 2 -i pipe:0 output.wav

# Pipe to FFmpeg → MP3 stream
inferno2pipe | ffmpeg -f s16le -ar 48000 -ac 2 -i pipe:0 -b:a 192k output.mp3

# Capture 10 seconds
inferno2pipe --duration 10 | ffmpeg -f s16le -ar 48000 -ac 2 -i pipe:0 clip.wav

# Use a specific device (partial name match)
inferno2pipe --device "Speakers" --channels 2 --rate 48000
```

---

## CLI Reference

```
inferno2pipe [OPTIONS]

Options:
  --device <NAME>       WASAPI device name filter (default: system default render device)
  --channels <N>        Number of output channels (default: 2)
  --rate <HZ>           Sample rate in Hz (default: 48000)
  --duration <SECS>     Capture duration in seconds, 0 = infinite (default: 0)
  -h, --help            Print help
```

---

## Output Format

| Property | Value |
|----------|-------|
| Encoding | Signed 16-bit integer PCM |
| Byte order | Little-endian |
| Channel layout | Interleaved |
| Sample rate | As specified by `--rate` (default 48000 Hz) |
| Channels | As specified by `--channels` (default 2) |

The output matches the `-f s16le` format in FFmpeg.

---

## Prerequisites

- `inferno_wasapi` must be running in RX mode and receiving Dante audio
- The WASAPI render device must be set to 48 kHz (or match `--rate`)
- No virtual audio driver is needed — loopback capture works on any render device

---

## Building

```powershell
cargo build -p inferno2pipe --release
```

---

## Architecture

```
inferno2pipe
└── WASAPI loopback capture (wasapi crate)
    ├── Open default render device (Direction::Render, loopback=true)
    ├── Auto-detect mix format (get_mixformat + is_supported)
    ├── Capture loop: read audio packets from WASAPI
    │   └── Convert f32/i32 samples → i16 PCM
    └── Write raw bytes to stdout (BufWriter for efficiency)
```

Audio from the Dante network flows:
```
Network → inferno_wasapi (RX) → WASAPI render device → inferno2pipe (loopback) → stdout
```

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `wasapi 0.15` | WASAPI loopback capture |
| `windows` | Win32 COM/audio feature flags |
| `clap` | CLI argument parsing |
| `anyhow` | Error handling |
| `tracing` | Diagnostic logging |
