# Inferno Windows — Copilot Instructions

> **Read this file completely before doing anything else.**
> It describes the full current state, the build machine, and what still needs to be done.

---

## What this project is

A Windows port of [Inferno AoIP](https://gitlab.com/lumifaza/inferno) — an open-source
reverse-engineered implementation of the Dante AoIP protocol. Goal: a free alternative to
Audinate's Dante Virtual Soundcard (DVS) for Windows.

**Current capability:**
- ✅ RX mode: receive audio from a Dante network → play via WASAPI speakers
- ✅ TX mode (code complete, not yet tested on hardware): capture Windows audio via WASAPI
  loopback → send to Dante network as a source device
- ❌ PTP clock: Windows stub uses SystemTime (sufficient for PoC, not production)

---

## Workspace layout

```
inferno_windows/
├── inferno_aoip/          # Core Dante protocol (ported from Linux — keep changes minimal)
│   └── src/device_server/ # DeviceServer, TxPusher, FlowsTransmitter, ring buffers
├── inferno_wasapi/        # The Windows binary (RX + TX modes, WASAPI)
│   └── src/main.rs        # CLI entry point — THIS is the main file to work on
├── usrvclock/             # Windows clock stub (SystemTime-based, not true PTP)
├── searchfire/            # mDNS discovery (cross-platform, don't touch)
├── scripts/               # PowerShell setup scripts for the build machine
│   ├── setup-dev-machine.ps1   # winget installs (Git, gh, rustup, VS Build Tools)
│   ├── enable-testsigning.ps1  # bcdedit /set testsigning on
│   ├── install-sysvad.ps1      # Build + install SYSVAD virtual audio driver
│   ├── setup-github-runner.ps1 # Register self-hosted Actions runner
│   └── open-firewall-admin.ps1 # Open Dante UDP ports
├── SETUP.md               # Full step-by-step Windows machine setup guide
├── NOTES.md               # Runtime status, known issues, architecture decisions
└── .github/
    ├── copilot-instructions.md  # THIS FILE
    └── workflows/
        └── copilot-setup-steps.yml  # Copilot cloud agent setup (self-hosted runner)
```

---

## Build machine

| | |
|---|---|
| **OS** | Windows 10 Pro x64 |
| **VS** | Visual Studio 2022 Community (Desktop C++ workload) |
| **SDK** | Windows SDK (latest) |
| **WDK** | Windows Driver Kit (installed as VS2022 extension) |
| **EWDK** | Enterprise WDK ISO mounted/installed |
| **Rust** | Installed via rustup (stable x86_64-pc-windows-msvc) |
| **Git** | Installed |
| **gh** | GitHub CLI, authenticated |
| **Secure Boot** | DISABLED (required for test-signed kernel drivers) |

> **Windows 10 note:** test signing with `bcdedit /set testsigning on` works on Win10 without
> Secure Boot being disabled (unlike Win11 which requires firmware-level Secure Boot off).
> Run `.\scripts\enable-testsigning.ps1` if not done yet, then reboot.

---

## How to build

```powershell
# Debug build (fast, for iteration)
cargo build

# Release build (for testing performance)
cargo build --release

# Check only (no binary output, fastest)
cargo check --workspace
```

The workspace currently builds clean (`cargo check --workspace` exits 0).

---

## CLI usage

```powershell
# List WASAPI render devices (for RX output or TX loopback source)
.\target\release\inferno_wasapi.exe --list-devices

# RX mode: receive Dante audio, play through default speakers
.\target\release\inferno_wasapi.exe

# RX mode: play through a specific device
.\target\release\inferno_wasapi.exe --device "Realtek"

# TX mode: capture system audio, transmit to Dante network
.\target\release\inferno_wasapi.exe --tx

# TX mode: capture from SYSVAD virtual speaker specifically
.\target\release\inferno_wasapi.exe --tx --tx-device "Tablet Audio" --tx-channels 2 --name MyPC-TX

# List devices available for TX loopback capture
.\target\release\inferno_wasapi.exe --list-tx-devices
```

---

## Key constraints (IMPORTANT — do not violate these)

1. **Keep `inferno_aoip/` changes minimal** — only fix bugs directly caused by your changes.
   Don't refactor upstream code.
2. **All changes stay in this repo** — do not touch any other repos or remote machines.
3. **PTP is not implemented** — `usrvclock` uses SystemTime. This is intentional for the PoC.
   Document timing issues in `NOTES.md` but don't try to implement PTP.
4. **Commit each logical change separately** with a clear message.
5. **Do not break RX mode** when working on TX, and vice versa.

---

## TX architecture (how it works)

```
Windows apps
    │  play to SYSVAD virtual speaker (kernel WDM driver)
    ▼
WASAPI audio engine mix (render device)
    │  WASAPI loopback capture (Direction::Capture on render device)
    │  → AUDCLNT_STREAMFLAGS_LOOPBACK
    ▼
wasapi_capture_thread (in inferno_wasapi/src/main.rs)
    │  f32 → i32 (24-bit left-justified), de-interleave
    ▼
TxPusher::push_channels()  (in inferno_aoip/src/device_server/mod.rs)
    │  writes into OwnedBuffer ring buffers (2^17 = 131072 samples per channel)
    │  timestamps seeded from SystemTime to align with media clock
    ▼
FlowsTransmitter  (inferno_aoip/src/device_server/flows_tx.rs)
    │  reads ring buffers, packetises into Dante RTP UDP packets
    ▼
Dante network  →  any Dante receiver / Dante Controller
```

**SYSVAD virtual speaker** is a Microsoft open-source kernel audio driver from the
[Windows Driver Samples](https://github.com/microsoft/Windows-driver-samples) repo,
path `audio/sysvad/TabletAudioSample`. It creates a virtual "Tablet Audio Speaker" in
Windows Sound settings. Set it as the default playback device so all Windows audio
flows through it and becomes capturable via WASAPI loopback.

---

## What still needs to be done (remaining tasks)

### Priority 1 — Get TX working end-to-end

1. **Install SYSVAD virtual audio driver** (if not done):
   ```powershell
   # Clone Windows driver samples alongside this repo:
   git clone https://github.com/microsoft/Windows-driver-samples.git ..\windows-driver-samples
   # Then build and install:
   .\scripts\install-sysvad.ps1
   ```
   After install, open Sound settings and set "Tablet Audio Speaker" as default playback.

2. **Open firewall ports** (if not done):
   ```powershell
   # Run as Administrator:
   .\scripts\open-firewall-admin.ps1
   ```

3. **Build release binary:**
   ```powershell
   cargo build --release
   ```

4. **Test TX:**
   ```powershell
   .\target\release\inferno_wasapi.exe --tx --tx-device "Tablet Audio" --name MyPC-TX
   ```
   Then open Dante Controller and verify "MyPC-TX" appears as a transmitter with 2 channels.
   Route those channels to a Dante receiver to hear audio.

5. **Test RX (regression):**
   ```powershell
   .\target\release\inferno_wasapi.exe
   ```
   Device should appear in Dante Controller as a receiver.

### Priority 2 — Known likely issues to fix during TX testing

- **Timestamp drift:** The ring buffer uses SystemTime for alignment. If audio dropouts occur,
  check logs for "tx lag" or "readable" debug messages. May need to pre-buffer more frames.
- **Format mismatch:** WASAPI loopback captures in device's native format (usually f32 at
  system sample rate). If the device runs at 44100 Hz and Dante expects 48000 Hz, the
  `convert=true` flag in `initialize_client` should handle it, but verify in logs.
- **Channel count:** SYSVAD TabletAudioSample creates a stereo device. If you need more
  channels, a different SYSVAD variant (e.g. `sysvad\PhoneAudioSample`) supports more.

### Priority 3 — Quality improvements (after PoC works)

- Add `--rx-channels` and `--tx-channels` validation against device capabilities
- Improve TX timestamp seeding: subscribe to the DeviceServer's MediaClock instead of
  using SystemTime directly in `transmit_with_push`
- Reduce TX latency: current ring buffer is ~2.7s; for lower latency reduce BUF_SIZE
  (must remain power of 2, e.g. 16384 = ~341ms at 48kHz)

---

## Troubleshooting

**"Test Mode" watermark not showing:**
→ Run `.\scripts\enable-testsigning.ps1` as admin and reboot.

**SYSVAD device not in Sound settings:**
→ Check Device Manager for driver load errors.
→ Ensure test signing is enabled and you rebooted after.
→ Re-run `.\scripts\install-sysvad.ps1` as admin.

**Device not appearing in Dante Controller:**
→ Run `.\scripts\open-firewall-admin.ps1` as admin.
→ Confirm the machine is on the same subnet as other Dante devices.
→ Check NOTES.md for port list.

**TX audio is silent / all zeros in Dante Controller:**
→ Check that SYSVAD is set as the default Windows playback device.
→ Play audio through it and verify WASAPI loopback is capturing (check logs for "first audio").

**`cargo build` errors:**
→ Ensure rustup target `x86_64-pc-windows-msvc` is active: `rustup show`
→ Ensure MSVC build tools are in PATH (run from VS Developer Command Prompt if needed)

---

## Key files to know

| File | Purpose |
|------|---------|
| `inferno_wasapi/src/main.rs` | Main binary — RX + TX modes, all WASAPI I/O |
| `inferno_aoip/src/device_server/mod.rs` | DeviceServer API, TxPusher, transmit_with_push |
| `inferno_aoip/src/device_server/flows_tx.rs` | Dante TX engine (reads ring buffers, sends RTP) |
| `inferno_aoip/src/ring_buffer.rs` | Ring buffer (must be power-of-2 length) |
| `usrvclock/src/lib.rs` | Windows clock stub (SystemTime → ClockOverlay) |
| `NOTES.md` | Runtime findings, known issues, architecture notes |
| `SETUP.md` | Step-by-step Windows machine setup guide |
| `scripts/install-sysvad.ps1` | SYSVAD driver build + install |
| `scripts/open-firewall-admin.ps1` | Open Dante UDP firewall ports |
