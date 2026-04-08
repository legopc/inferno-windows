# Inferno ASIO Host

## Status

**Current**: Stub crate; native ASIO implementation planned for Q3–Q4 2024.

This crate will provide native ASIO driver support for the Inferno audio engine, enabling professional DAW integration (Ableton Live, Reaper, FL Studio, Pro Tools, Cubase, Studio One).

## Quick Start

**Today** (ASIO4ALL bridge):
- Use [ASIO4ALL v2.13+](http://www.asio4all.com/) to expose Inferno as an ASIO device
- Typical latency: 5–10ms round-trip
- Suitable for home and semi-professional studios
- See [../docs/ASIO.md](../docs/ASIO.md) for step-by-step setup

**Future** (native ASIO):
- This crate will implement the Steinberg ASIO SDK interface
- Exclusive ASIO mode: sub-3ms latency
- Professional studio workflows
- Monitor release notes for updates

## Documentation

See [../docs/ASIO.md](../docs/ASIO.md) for comprehensive guides:
- **What is ASIO?** Low-latency protocol for professional audio
- **ASIO4ALL bridge**: How to use it today
- **Native ASIO roadmap**: Future implementation plan
- **DAW compatibility table**: Ableton, Reaper, FL Studio, Pro Tools, etc.

## Architecture (Planned)

```
DAW (Ableton, Reaper, etc.)
    ↓ ASIO API calls
inferno_asio (IASIODriver implementation)
    ↓ Audio buffers
inferno_wasapi (Audio I/O core)
    ↓ WASAPI
Windows Audio System / Hardware
```

## Implementation Plan

### Phase 1: Stub & Documentation ✓
- Create `inferno_asio` crate structure
- Document ASIO4ALL bridge for immediate DAW support
- Outline native ASIO roadmap

### Phase 2: FFI Wrapper (Q3–Q4 2024)
- Wrap Steinberg ASIO SDK via `windows-sys`
- Implement `IASIODriver` trait
- Buffer management and callbacks

### Phase 3: Integration (2025)
- Route ASIO callbacks to Inferno's audio pipeline
- Integrate with `inferno_wasapi` audio core
- Minimal-copy buffer design

### Phase 4: Beta Testing (2025)
- Validate with real DAWs (Reaper, Ableton, Studio One)
- Latency benchmarking
- Stability hardening

## Contributing

Contributions welcome! Areas to help:
- FFI bindings for ASIO SDK
- Audio buffer callback implementations
- DAW testing and latency measurement
- Documentation improvements

## License

Apache 2.0 (compatible with Steinberg ASIO SDK licensing for open-source projects)

## References

- [Inferno ASIO Guide](../docs/ASIO.md)
- [ASIO4ALL Project](http://www.asio4all.com/)
- [Steinberg Developer](https://www.steinberg.net/en/company/developer.html)
