# Windows Virtual Audio Device — Research Handoff

> **For:** Claude Sonnet 4.6 (Copilot CLI), to be interpreted in context of the existing Rust/Dante project  
> **Topic:** How to create a Windows virtual audio endpoint that the Rust app can capture from  
> **Constraint:** No external user-facing programs. Open source. Integratable as a module.

---

## Core Finding: A Kernel Driver Is Unavoidable

A device that appears as a speaker in Windows Sound settings **requires a kernel-mode WDM audio minidriver**. There is no userspace-only path to creating a first-class Windows audio endpoint. This is the central architectural constraint everything else flows from.

---

## Option Evaluated: Scream

**Repo:** https://github.com/duncanthrax/scream  
**License:** MIT

Scream is an open-source WDM virtual sound card. Its architecture is exactly right — the kernel driver captures the render stream and passes PCM to a userspace receiver over the network. The driver and the sender are decoupled, meaning the network sender can be replaced with the Dante pipeline.

**Blocker:** The driver signing is broken on recent Windows (Win10 21H2+ / Win11 with Secure Boot / HVCI). End users cannot load it without enabling test signing mode (`bcdedit /set testsigning on`), which is a significant barrier.

**Still worth reading:** The Scream driver source shows exactly where in the WDM render path to intercept PCM — specifically in the DMA buffer callbacks and `CMiniportWaveCyclicStream::SetState`. This is useful reference even if SYSVAD is used as the base.

---

## Preferred Foundation: Microsoft SYSVAD

**Repo:** https://github.com/microsoft/Windows-driver-samples/tree/main/audio/sysvad  
**Docs:** https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/wavert-miniport-driver  
**License:** MIT (Windows Driver Samples)

SYSVAD is Microsoft's official reference WDM audio minidriver sample. It is the most complete and maintainable starting point.

### Relevant sub-project: `TabletAudioSample`

This is the target sub-project within SYSVAD. It implements:

- Virtual **render** endpoint — this is what Windows sees as a speaker
- Virtual **capture** endpoint
- Virtual **loopback** endpoint
- Full format negotiation (sample rate, bit depth, channel count)
- KSPROPERTY support (volume, mute, jack detection)

The driver renders audio into a **kernel-space DMA buffer**. The integration task is bridging that buffer to the Rust userspace process.

### SYSVAD vs Scream Comparison

| | Scream | SYSVAD |
|---|---|---|
| Driver model | WDM legacy (PortCls) | AVStream / KS (modern) |
| Signing status | Broken on modern Windows | You sign it yourself |
| Customisability | Limited | Complete |
| Virtual endpoint types | Render only | Render + Capture + Loopback |
| Format negotiation | Basic | Full Windows audio API |
| Reference quality | Community | Official Microsoft sample |

---

## Bridging Kernel PCM to Rust Userspace

SYSVAD by itself silences audio. One of the following mechanisms is needed to get PCM into the Rust process:

### Option A — Named Shared Memory Ring Buffer (Recommended)
Driver writes PCM into a named shared memory section. Rust opens the same section and reads it.  
- Lowest latency  
- This is essentially what Scream does internally  
- Win32 API: `CreateFileMapping` / `OpenFileMapping` / `MapViewOfFile`

### Option B — WASAPI Loopback on the Virtual Device
Since SYSVAD registers a proper Windows audio endpoint, WASAPI loopback mode works cleanly on it. The Rust process opens the SYSVAD virtual speaker in loopback capture mode.  
- No driver modification needed for the bridge  
- Slightly higher latency than shared memory  
- Rust crate: [`wasapi`](https://crates.io/crates/wasapi)

### Option C — Custom IOCTL
Define a `DeviceIoControl` interface in the driver. Rust calls it to pull PCM chunks on demand.  
- Most explicit and controlled  
- More complex to implement  
- Win32 API: `DeviceIoControl`

---

## The Signing Problem

This affects both Scream and any SYSVAD-based driver. On modern Windows, all kernel drivers must be:

1. Signed with an **EV code signing certificate** (~€300–500/year, requires a registered legal entity)
2. Submitted to **Microsoft Hardware Dev Center** — either WHQL (full hardware certification) or **attestation signing** (lighter, sufficient for audio drivers)

**Attestation signing** is the practical path:
- Requires a Microsoft Partner Center account: https://partner.microsoft.com/en-us/dashboard
- Requires an EV cert from DigiCert or Sectigo
- Submit a driver CAB file; Microsoft countersigns it
- Does not require hardware lab testing
- Turnaround typically 1–3 business days

**For development / internal use only:** test signing mode bypasses this entirely:
```
bcdedit /set testsigning on
```
This is acceptable during development but not for end-user distribution.

---

## Rust Crates for the Userspace Side

| Crate | Purpose | Notes |
|---|---|---|
| [`wasapi`](https://crates.io/crates/wasapi) | WASAPI loopback capture | Best fit for Option B above |
| [`cpal`](https://crates.io/crates/cpal) | Cross-platform audio I/O | Easier API, less control over loopback |
| [`windows`](https://crates.io/crates/windows) | Raw Windows API bindings | Needed for shared memory, IOCTL, etc. |
| [`wdk`](https://crates.io/crates/wdk) | WDK bindings (experimental) | If writing any driver parts in Rust |

---

## Recommended Architecture

```
Windows application (e.g. DAW, browser)
            │
            ▼
  [SYSVAD virtual speaker endpoint]   ← kernel WDM driver, signed via attestation
            │  DMA buffer
            ▼
  Named shared memory / WASAPI loopback
            │
            ▼
  Rust userspace process
            │  PCM samples
            ▼
  Existing Dante / AES67 RTP send pipeline
            │
            ▼
  Dante network / Dante Controller
```

---

## Key Documentation URLs

### Windows Driver Kit (WDK) & Audio Drivers
- WDK install and setup: https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk
- Audio drivers overview: https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/audio-drivers-overview
- WaveRT miniport (the render model SYSVAD uses): https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/wavert-miniport-driver
- AVStream overview: https://learn.microsoft.com/en-us/windows-hardware/drivers/stream/avstream-overview
- PortCls overview (legacy, what Scream uses): https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/introduction-to-port-class

### SYSVAD Specific
- SYSVAD sample top-level: https://github.com/microsoft/Windows-driver-samples/tree/main/audio/sysvad
- TabletAudioSample: https://github.com/microsoft/Windows-driver-samples/tree/main/audio/sysvad/TabletAudioSample
- SYSVAD on Microsoft Docs: https://learn.microsoft.com/en-us/samples/microsoft/windows-driver-samples/sysvad-virtual-audio-device-driver-sample/

### WASAPI (Userspace Capture)
- WASAPI overview: https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi
- Loopback recording: https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording
- Capturing a stream: https://learn.microsoft.com/en-us/windows/win32/coreaudio/capturing-a-stream

### Driver Signing & Distribution
- Attestation signing walkthrough: https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/attestation-signing-a-kernel-driver-for-public-release
- Hardware Dev Center submission: https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/hardware-submission-create
- EV certificate requirements: https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/get-a-code-signing-certificate

### Scream (Reference for DMA intercept pattern)
- Scream driver source: https://github.com/duncanthrax/scream/tree/master/driver

### Rust Crate Docs
- wasapi crate: https://docs.rs/wasapi
- windows crate: https://docs.rs/windows
- wdk crate: https://docs.rs/wdk
- cpal crate: https://docs.rs/cpal

---

## Suggested Implementation Order

1. **Set up WDK build environment** — Visual Studio 2022 + WDK extension
2. **Build SYSVAD `TabletAudioSample` unmodified** — verify it loads under test signing and appears in Windows Sound settings
3. **Study Scream driver source** for the DMA buffer intercept pattern
4. **Modify SYSVAD** to write PCM to a named shared memory ring buffer instead of discarding it
5. **Write Rust shared memory reader** using the `windows` crate (`OpenFileMapping` / `MapViewOfFile`)
6. **Feed PCM into existing Dante send pipeline**
7. **Pursue attestation signing** once the driver is stable, for distribution
