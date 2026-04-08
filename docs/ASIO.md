# ASIO Support for Inferno

## What is ASIO?

**ASIO (Audio Stream Input/Output)** is a low-latency audio protocol developed by Steinberg for professional Digital Audio Workstations (DAWs). Unlike standard Windows audio APIs (WASAPI, DirectSound), ASIO provides:

- **Direct hardware access**: Bypasses the OS audio mixer for lower latency
- **Lower latency**: Typical 5–15ms round-trip vs. WASAPI's 10–30ms
- **Professional standard**: Required for recording, live performance, and virtual instruments in modern DAWs
- **Cross-platform**: Windows, macOS, Linux implementations available

ASIO is the de facto standard for audio professionals on Windows and is essential for DAW integration.

---

## Current Status: Inferno + WASAPI

Inferno currently uses **WASAPI (Windows Audio Session API)** for audio I/O:

- **Advantages**: Native Windows API, good latency for consumer applications (10–30ms)
- **Limitation**: DAWs and professional audio software prefer ASIO; Inferno cannot be directly used as an audio device in Ableton Live, Reaper, Pro Tools, etc.
- **User workaround**: Inferno works as a virtual device but not exposed via ASIO

To enable professional DAW workflows, Inferno needs either:
1. **Short-term**: ASIO4ALL bridge (software solution)
2. **Long-term**: Native ASIO host implementation

---

## ASIO4ALL Bridge: Immediate Solution

### What is ASIO4ALL?

**ASIO4ALL** (v2) is free, open-source software that wraps any Windows audio device (including WASAPI devices like Inferno) and exposes it as an ASIO driver. This allows DAWs to use Inferno as an ASIO audio interface.

### How to Use ASIO4ALL with Inferno

#### Step 1: Install ASIO4ALL
- Download ASIO4ALL v2.13+ from [ASIO4ALL project page](http://www.asio4all.com/)
- Run the installer and follow default settings
- ASIO4ALL registers itself as an ASIO driver on your Windows system

#### Step 2: Configure ASIO4ALL to Use Inferno
1. Open ASIO4ALL Control Panel (found in Windows audio settings or start menu)
2. In the **Devices** list, ensure the checkbox is enabled for **Inferno** (or Inferno's virtual device name)
3. Click **Options** and configure:
   - **Buffer size**: 256 or 512 samples (lower = more latency risk, higher = more latency)
   - **Sample rate**: Match your DAW's session (44.1 kHz, 48 kHz, etc.)
4. Click **OK** to save

#### Step 3: Select ASIO4ALL in Your DAW
1. Open your DAW (Ableton Live, Reaper, FL Studio, Pro Tools, etc.)
2. Go to **Audio Settings** or **Preferences**
3. Select **ASIO4ALL** as the audio driver
4. In ASIO4ALL configuration, confirm Inferno is listed in input/output devices
5. Route audio to Inferno's device

#### Step 4: Test
- Play audio in your DAW; you should hear it through Inferno
- Monitor latency: expect 5–10ms round-trip (Inferno WASAPI + ASIO4ALL overhead)

### Typical Latency

- **ASIO4ALL + Inferno WASAPI**: 5–10ms round-trip (5ms Inferno + 5ms ASIO4ALL overhead)
- **Direct WASAPI** (consumer apps): 10–30ms
- **Exclusive ASIO** (native driver): <3ms (goal for native implementation)

### Limitations of ASIO4ALL

- **Shared mode only**: No exclusive ASIO access; WASAPI remains in shared mode
- **Multi-device complexity**: If multiple devices are active, ASIO4ALL may mix them (can cause crosstalk)
- **No DSD support**: ASIO4ALL does not support DSD (Direct Stream Digital)
- **Stability**: Depends on underlying WASAPI stability; ASIO4ALL adds one software layer

**Recommendation**: ASIO4ALL is suitable for home studio and semi-professional setups. For full professional use, a native ASIO implementation is preferable.

---

## Future Plan: Native ASIO Host

### Rationale

A native ASIO implementation would provide:
- **Sub-3ms latency**: Direct hardware access via Steinberg ASIO SDK
- **Exclusive mode**: True exclusive ASIO, not shared via WASAPI
- **Professional stability**: No intermediate software wrapper
- **DSD future**: Foundation for lossless formats

### Technical Plan

#### Licensing
- **ASIO SDK**: Steinberg provides the ASIO SDK free of charge for:
  - Commercial products
  - GPL-compatible projects (Inferno's Apache 2.0 is compatible)
  - Academic/research use
- **Process**: Register with Steinberg, agree to license terms, download SDK

#### Architecture: `inferno_asio` Crate

```
inferno_asio/
├── Cargo.toml                 # Depends on inferno_wasapi for audio core
├── src/
│   ├── lib.rs                 # ASIO host wrapper
│   ├── driver.rs              # ASIO driver interface (Win32 FFI)
│   ├── callbacks.rs           # Audio buffer callbacks
│   └── config.rs              # ASIO configuration (buffer sizes, sample rates)
└── README.md                  # Usage guide
```

#### Implementation Steps
1. **Wrap Steinberg ASIO SDK** via `windows-sys` or `winapi` crate for FFI
2. **Implement ASIO device interface**:
   - `IASIODriver` trait (init, start, stop, getChannels, getBufferSize, etc.)
   - Audio buffer callbacks synchronized with Inferno's WASAPI thread
3. **Reuse Inferno audio pipeline**:
   - Route ASIO calls → Inferno DSP → system audio output
4. **Buffer management**: Minimize copies; use ring buffers for real-time audio
5. **Testing**: Validate latency in Reaper, Ableton, Pro Tools

#### Dependencies (to Add)
```toml
windows-sys = { version = "0.59", features = ["Win32_Audio"] }
inferno_wasapi = { path = "../inferno_wasapi" }  # Audio core
```

### Timeline
- **Phase 1**: Stub and documentation (current: `inferno_asio` crate)
- **Phase 2**: FFI wrapper for ASIO SDK (Q3–Q4 2024)
- **Phase 3**: Integration with Inferno audio thread (2025)
- **Phase 4**: Beta testing with DAWs (2025)

---

## DAW Compatibility

| DAW | ASIO4ALL Support | Native ASIO (Future) | Notes |
|-----|------------------|----------------------|-------|
| **Ableton Live** | ✓ Yes | ✓ Planned | ASIO4ALL works; live sets can route to Inferno |
| **Reaper** | ✓ Yes | ✓ Planned | Excellent ASIO4ALL support; low-latency mode |
| **FL Studio** | ✓ Yes | ✓ Planned | ASIO latency compensation available |
| **Pro Tools** | ✓ Limited | ✓ Planned | ASIO4ALL requires kernel streaming (experimental) |
| **Studio One** | ✓ Yes | ✓ Planned | Good ASIO4ALL integration |
| **Cubase** | ✓ Yes | ✓ Planned | Native ASIO is standard (why native Inferno ASIO matters) |

### Recommendation
- **Today**: Use ASIO4ALL for DAW integration, suitable for home/semi-pro studios
- **Future**: Native ASIO for professional studios requiring <3ms latency and exclusive mode

---

## Getting Started

### Short Term (ASIO4ALL)
1. Install ASIO4ALL v2.13+
2. Configure Inferno in ASIO4ALL settings
3. Select ASIO4ALL in your DAW
4. Enjoy sub-10ms latency for professional workflows

### Long Term (Native ASIO)
- Monitor `inferno_asio` crate development in the Inferno repository
- Native ASIO support will be announced in release notes
- Expect improved latency (<3ms) and exclusive mode benefits

---

## References

- [ASIO4ALL Project](http://www.asio4all.com/)
- [Steinberg ASIO Documentation](https://www.steinberg.net/en/company/developer.html)
- [Reaper ASIO Guide](https://www.reaper.fm/guides/asio.php)
- [Windows Audio Architecture](https://docs.microsoft.com/en-us/windows-hardware/drivers/audio/)
