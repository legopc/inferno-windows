# InfernoAoIP — Master Roadmap (Living Document)

> **Vision**: A complete, production-quality free alternative to Audinate Dante Virtual Soundcard
> (DVS) for Windows — full TX/RX audio, a proper virtual audio driver, robust PTP clock sync,
> and a polished GUI. Suitable for live sound, broadcast, and studio use.
>
> **Current completion estimate**: ~65% of protocol/infrastructure; ~30% of production features.
> Fully working RX. TX functional but not production-hardened. SYSVAD driver not yet built.

---

## Audit Summary (2026-04-08)

| Area | State | Notes |
|------|-------|-------|
| RX audio | ✅ ~95% | Working, Dante Controller visible |
| TX audio | ⚠️ ~70% | Protocol complete, RT bugs, no kernel driver |
| SYSVAD virtual driver | 🔴 0% | The "audio I/O driver" — not started |
| PTP clock sync | �� Stub | SafeClock only; no PTPv1 discipline |
| GUI | ⚠️ Basic | Status + 3 buttons; no settings editing |
| FPP negotiation | ⚠️ Partial | Config field added; wiring TODO |
| Installer (MSI) | ✅ 90% | WiX config done; WiX Toolset not in CI |
| Tests | ✅ 25+ | Passing; no TX/PTP/driver CI tests |
| Docs | ✅ Good | README, CONTRIBUTING, per-crate READMEs all current |

**Outstanding issues**: 65 TODOs, 11 FIXMEs — 8 are RT memory safety, 12 are protocol/negotiation.

---

## Sprint Structure

Each sprint is a self-contained unit of work with a clear goal. Sprints are ordered by value/risk.
Fleet agents implement individual todos in parallel. Each sprint ends with a build + push.

---

## Sprint 1 — Critical Stability (RT Safety + Protocol Correctness)

**Goal**: Eliminate bugs that could cause audio glitches, panics, or silent failures in production.

| ID | Task | File(s) | Priority |
|----|------|---------|----------|
| s1-fpp-wire | Wire FPP config to actual negotiation in channels_subscriber.rs:946 | channels_subscriber.rs | HIGH |
| s1-rt-dealloc | Move Arc/VecDeque drops out of RT audio threads → background drop channel | flows_rx.rs:423, flows_tx.rs:397 | HIGH |
| s1-panic-fix | Replace MAY PANIC assertions in req_resp.rs:24,29 with proper error returns | protocol/req_resp.rs | HIGH |
| s1-hostname-validation | Enforce 31-char Dante device name limit; truncate + warn on startup | device_info.rs:19 | MEDIUM |
| s1-buffer-size-config | Make ring buffer size and latency reference configurable in config.toml | device_server/mod.rs:97 | MEDIUM |
| s1-sample-rate-guard | Detect TX device sample rate mismatch; refuse flow with clear log message | channels_subscriber.rs:906 | MEDIUM |
| s1-index-sanity | Add index bounds checks in flows_control_server.rs:79 before array access | flows_control_server.rs | MEDIUM |

---

## Sprint 2 — Full Settings GUI

**Goal**: Users can configure everything from the GUI without editing config.toml manually.

| ID | Task | Notes |
|----|------|-------|
| s2-settings-tab | Add "Settings" tab/window to inferno_gui with all config fields editable | Replaces config.toml hand-edit |
| s2-device-dropdown | Network interface dropdown (if_addrs enumeration) and WASAPI device dropdown | Calls --list-devices IPC |
| s2-channel-naming-ui | Per-channel name editor in GUI (text fields, save to config) | |
| s2-fpp-dropdown | FPP mode selector: Auto / Min Latency / Max Efficiency / Custom | Wires to s1-fpp-wire |
| s2-vu-meters | Real-time per-channel VU meter bars (tx_peak_db from IPC, 10fps refresh) | NWG ProgressBar or custom draw |
| s2-flows-panel | Active flows list: show name, source IP, channels, state per flow | New IPC message: GetFlows |
| s2-ipc-getflows | Add GetFlows IPC command returning active RX/TX flows from DeviceServer | New IPC message type |
| s2-settings-save-reload | "Save & Apply" button writes config.toml then sends ReloadConfig IPC | |

---

## Sprint 3 — TX Mode Hardening

**Goal**: TX mode works reliably without a kernel driver using WASAPI loopback or VB-Cable.

| ID | Task | Notes |
|----|------|-------|
| s3-tx-end-to-end | Manual test TX mode with Dante Controller + receiver; document test steps | Needs real or virtual Dante receiver |
| s3-vb-cable-autodetect | Auto-detect VB-Cable on startup; warn if neither VB-Cable nor Stereo Mix found | Improves TX first-run experience |
| s3-tx-latency-config | Wire flows_tx.rs:108 latency (currently hardcoded) to config.latency_ms | |
| s3-tx-channel-map | TX channel routing: allow mapping Windows audio channels to Dante TX channels | New config field: tx_channel_map |
| s3-tx-status-ipc | Report TX channel count, active flows, and peak levels accurately in IPC status | Was hardcoded; partly fixed Wave 7 |
| s3-tx-ci-test | Add integration test for TX IPC commands (CreateFlow, DeleteFlow) | |

---

## Sprint 4 — PTPv1 In-App Clock Sync

**Goal**: Sync to a PTPv1 grandmaster on the network without requiring external tools.

| ID | Task | Notes |
|----|------|-------|
| s4-ptp-listener | inferno_aoip/src/ptp/: bind UDP 319, join multicast 224.0.1.129, parse PTPv1 Sync | New module, feature-gated |
| s4-ptp-offset-ema | EMA filter (alpha=0.1) on measured offset; write to ClockOverlay shift_ns | Stable clock discipline |
| s4-ptp-fallback | If no PTPv1 Sync seen for >5s, reset shift=0 → SafeClock fallback | Robustness |
| s4-ptp-clock-mode-ipc | Report "PTP" vs "SafeClock" in IPC status clock_mode field | Already a field, wire it |
| s4-ptp-gui-status | GUI shows grandmaster IP, sync quality (offset ppm), and fallback state | |
| s4-ptp-docs | Update docs/PTP_WINDOWS.md with in-app PTP section + disable PTPSync note | |

---

## Sprint 5 — SYSVAD Virtual Audio Driver

**Goal**: A proper Windows virtual audio device that appears in sound settings — the real
"audio input/output driver". This is the largest and most complex sprint.

| ID | Task | Notes |
|----|------|-------|
| s5-wdk-setup | Set up WDK build environment; verify TabletAudioSample builds unmodified | Prerequisite for all driver work |
| s5-sysvad-fork | Fork SYSVAD TabletAudioSample into inferno-windows as `inferno_driver/` crate | Kernel C++ project |
| s5-shm-write | Modify SYSVAD DMA buffer callback to write audio to named shared memory | Win32 `CreateFileMapping` |
| s5-shm-read | Rust module in inferno_wasapi to read from named shared memory → DeviceServer | `MapViewOfFile` + ring buffer |
| s5-driver-testsign | Build driver in test-signing mode; bcdedit script; test on VM | Dev/test only |
| s5-driver-install-ui | GUI: detect if driver installed; "Install Driver" button (runs inf installer) | UX for first-run |
| s5-driver-docs | Update SETUP.md with SYSVAD build/install steps; update DRIVER_SIGNING.md | |
| s5-attest-sign | Guide: obtain EV cert, submit to Partner Center for attestation signing | Production distribution |

---

## Sprint 6 — Installer, CI & Distribution

**Goal**: Fully automated build pipeline producing signed, installable artifacts.

| ID | Task | Notes |
|----|------|-------|
| s6-wix-ci | Install WiX Toolset v3 in CI runner; add MSI build step to ci.yml | Unblocks MSI in CI |
| s6-release-artifact | GitHub Actions: upload MSI as release artifact on tag push | `actions/upload-artifact` |
| s6-version-bump | Semantic versioning script; bump Cargo.toml versions on release | cargo-release or manual |
| s6-autoupdate-check | On startup, check GitHub releases API for newer version; notify in GUI | Non-blocking HTTP GET |
| s6-first-run-wizard | Detect first run (no config.toml); GUI wizard: NIC → firewall → service start | Improves first-run UX |
| s6-firewall-setup | Auto-add Windows Firewall rules (netsh) for ports 4440/4455/5353/6000-6015 | Run once on install |

---

## Sprint 7 — DVS Feature Parity & Advanced

**Goal**: Match Dante Virtual Soundcard feature set; add power-user capabilities.

| ID | Task | Notes |
|----|------|-------|
| s7-asio-scaffold | docs/ASIO.md: ASIO4ALL bridge option; scaffold inferno_asio crate stub | DAW users need ASIO |
| s7-96khz-validate | Test and document 96kHz operation; warn when >32 channels at 96kHz | DVS supports 48k/96k |
| s7-aes67-mode | AES67 interop mode scaffold (separate RTP profile, different multicast TTL) | Broadcast interop |
| s7-ipv6 | IPv6 support for mDNS and RTP flows | Future-proofing |
| s7-redundancy | Dual NIC redundancy (Dante RSTP mode): send/receive on two interfaces | Live sound reliability |
| s7-multidevice | Support multiple Dante devices on one machine (multiple DeviceServer instances) | Studio use case |
| s7-windows-notifications | Windows toast notifications: device connected, glitch, service error | `windows::Win32::UI::Shell` |
| s7-soak-test | Overnight stability test harness: run RX for 8h, check for underruns/crashes | CI quality gate |

---

## Known Deferred Items (not in any sprint yet)

- ARC opcode 0x2204 full implementation (harmless, Dante Controller still works)
- Eco mode / MIN_SLEEP CPU optimization (flows_tx.rs:44)
- Ring buffer silence-fill on data drought (ring_buffer.rs:116)
- Statime port to Windows (Tier 4 PTP — 2-3 month effort, post Sprint 4 assessment)
- IPv6 mDNS (searchfire currently IPv4 only)

---

## Completed Waves

- **Wave 1-3**: 59-item improvement plan (mDNS, IPC, CLI, config, metrics, tray)
- **Wave 4**: CLI overrides, peer list, clock offset IPC, ring buffer fixes
- **Wave 5**: GUI scaffold (eframe → NWG migration), IPC real server
- **Wave 6**: Documentation rewrite, SETUP/NOTES/PTP/DRIVER_SIGNING docs
- **Wave 7**: FPP config, RT socket retry, WASAPI sample format, 7 tests, agent tracking doc

---

## How Sprints Are Executed

1. Plan sprint → insert todos into SQL → dispatch fleet agents (1 per todo, parallel)
2. Review agent output → fix failures → final `cargo build --workspace`
3. Commit + push → checkpoint → update this doc
4. Repeat with next sprint

Next sprint to execute: **Sprint 1 — Critical Stability**
