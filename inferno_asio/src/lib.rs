//! # Inferno ASIO Host
//!
//! This crate provides native ASIO support for the Inferno audio engine.
//! Currently a stub; full implementation planned for Q3–Q4 2024.
//!
//! ## Status
//!
//! **Current**: Placeholder for native ASIO driver integration.
//!
//! **Planned Features**:
//! - Wrap Steinberg ASIO SDK via FFI
//! - Route ASIO callbacks to Inferno's audio pipeline
//! - Support exclusive ASIO mode with sub-3ms latency
//! - DAW compatibility: Ableton Live, Reaper, FL Studio, Pro Tools, Cubase, Studio One
//!
//! ## Rationale
//!
//! Inferno currently uses WASAPI for audio I/O. While WASAPI provides good latency
//! (10–30ms), professional DAWs expect ASIO drivers for low-latency audio.
//!
//! **Short-term solution**: Use [ASIO4ALL](http://www.asio4all.com/) to bridge
//! Inferno's WASAPI device as an ASIO driver (5–10ms latency).
//!
//! **Long-term solution**: Native ASIO implementation in this crate will provide
//! exclusive ASIO mode and sub-3ms latency for professional studio workflows.
//!
//! ## Architecture
//!
//! ```text
//! DAW (Ableton, Reaper, etc.)
//!     ↓ ASIO API calls
//! inferno_asio (IASIODriver implementation)
//!     ↓ Audio buffers
//! inferno_wasapi (Audio I/O core)
//!     ↓ WASAPI
//! Windows Audio System / Hardware
//! ```
//!
//! ## Dependencies (to Add)
//!
//! - `windows-sys`: FFI bindings for Windows ASIO interfaces
//! - `inferno_wasapi`: Audio pipeline integration
//!
//! ## Getting Started
//!
//! See [../../docs/ASIO.md](../../docs/ASIO.md) for:
//! - ASIO4ALL bridge usage (immediate solution)
//! - Native ASIO implementation roadmap
//! - DAW compatibility matrix
