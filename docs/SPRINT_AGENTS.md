# Sprint Fleet — Background Agent Model Tracking

This document records every background agent dispatched during the sprint-based development
phase of Inferno-Windows (Sprint 1 through Sprint 8). It includes the model used, task,
and outcome for each agent.

Sprint execution strategy:
- Fleet agents implement individual todos in parallel (1 agent per todo)
- GUI-heavy sprints use sub-waves to avoid git conflicts (backend first, then GUI)
- Orchestrator (Claude Sonnet 4.6) integrates results, fixes type errors, builds, and pushes
- Sprints 1–7 implement features; Sprint 8 is a polish-only pass (no new features)

---

## Sprint 1 — Critical Stability (RT Safety + Protocol Correctness)

**Goal**: Eliminate bugs that could cause audio glitches, panics, or silent failures.

| Agent ID | Model | Task | Outcome |
|---|---|---|---|
| `s1-fpp-wire` | claude-haiku-4.5 (Fleet Worker) | Wire config.fpp to actual FPP negotiation in channels_subscriber.rs | ✅ Committed `afa1314` |
| `s1-rt-dealloc` | claude-haiku-4.5 (Fleet Worker) | Add RT-SAFE TODO docs for deferred RT thread deallocation | ✅ Committed `0057975` |
| `s1-panic-fix` | claude-haiku-4.5 (Fleet Worker) | Replace panic-able assert!/unwrap in protocol parsing with Result returns | ✅ Committed `1267880` |
| `s1-hostname-validation` | claude-haiku-4.5 (Fleet Worker) | Enforce 31-char Dante device name limit + char sanitization | ✅ Committed `aed6578` |
| `s1-buffer-size-config` | claude-haiku-4.5 (Fleet Worker) | Make ring buffer size and latency ref configurable in config.toml | ✅ Committed `609715d` |
| `s1-sample-rate-guard` | claude-haiku-4.5 (Fleet Worker) | Add sample rate mismatch detection with debug log and FIXME | ✅ Committed (part of `1267880`) |
| `s1-index-sanity` | claude-haiku-4.5 (Fleet Worker) | Add bounds checks in flows_control_server.rs before array access | ✅ Committed `bc0eaa3` |
| Orchestrator (build fix) | claude-sonnet-4.6 (CLI) | Fix partial-move error (mod.rs) and future-not-Send (flows_control_server.rs) | ✅ Committed `bc0eaa3` |

**Sprint 1 Outcome**: ✅ All 7 todos done. Build green. Pushed `bc0eaa3`.

---

## Sprint 2 — Full Settings GUI

**Goal**: Users can configure everything from the GUI without editing config.toml manually.

Wave A (parallel — different file ownership):

| Agent ID | Model | Task | Outcome |
|---|---|---|---|
| `s2-ipc-getflows` | claude-haiku-4.5 (Fleet Worker) | Add GetFlows IPC command; FlowInfo struct; wire to ServiceState | ✅ Committed |
| `s2-settings-gui` | claude-haiku-4.5 (Fleet Worker) | Full Settings window (device name, sample rate, channels, NIC dropdown, FPP dropdown, channel names, Save & Apply) — covers todos s2-settings-tab + s2-device-dropdown + s2-channel-naming-ui + s2-fpp-dropdown | ✅ Committed `c9673e1` |

Wave B (after Wave A — adds to main window):

| Agent ID | Model | Task | Outcome |
|---|---|---|---|
| `s2-vu-meters` | claude-haiku-4.5 (Fleet Worker) | Real-time per-channel VU meter progress bars in main window (10fps) | ✅ Committed |
| `s2-flows-panel` | claude-haiku-4.5 (Fleet Worker) | Active flows list panel in main window using GetFlows IPC | 🔄 Running |

---

## Sprint 3 — TX Mode Hardening

**Goal**: TX mode works reliably; latency is configurable; VB-Cable auto-detected; status accurately reported.

| Agent ID | Model | Task | Outcome |
|---|---|---|---|
| `s3-tx-latency-config` | claude-haiku-4.5 (Fleet Worker) | Wire flows_tx.rs hardcoded latency to config.latency_ref_samples | ✅ Committed |
| `s3-tx-status-ipc` | claude-haiku-4.5 (Fleet Worker) | Report real TX channel count, active state, and peak levels in IPC | 🔄 Running |
| `s3-vb-cable-autodetect` | claude-haiku-4.5 (Fleet Worker) | Auto-detect VB-Cable / Stereo Mix on startup; warn if not found | ✅ Committed |

---

## Sprint 4 — PTPv1 In-App Clock Sync

**Goal**: Sync to a PTPv1 grandmaster on the network without requiring external tools.

Wave A (parallel — independent):

| Agent ID | Model | Task | Outcome |
|---|---|---|---|
| `s4-ptp-listener` | claude-haiku-4.5 (Fleet Worker) | New inferno_aoip/src/ptp/mod.rs — bind UDP 319, parse PTPv1 Sync, publish offsets via watch channel | ✅ Committed |
| `s4-ptp-clock-mode-ipc` | claude-haiku-4.5 (Fleet Worker) | Wire clock_mode IPC field to "PTP(ip)" when synced, "SafeClock" when free-running | 🔄 Running |

Wave B (after Wave A — depend on ptp module):

| Agent ID | Model | Task | Outcome |
|---|---|---|---|
| `s4-ptp-offset-ema` | claude-haiku-4.5 (Fleet Worker) | EMA filter (alpha=0.1) on PTP offsets; write to ClockOverlay shift_ns | 🔄 Running |
| `s4-ptp-fallback` | claude-haiku-4.5 (Fleet Worker) | Fallback to SafeClock after 5s without PTP Sync; re-engage on resume | 🔄 Running |

---

## Sprint 5 — SYSVAD Virtual Audio Driver

*(Pending Sprint 4 completion)*

---

## Sprint 6 — Installer, CI & Distribution

*(Pending Sprint 5 completion)*

---

## Sprint 7 — DVS Feature Parity & Advanced

*(Pending Sprint 6 completion)*

---

## Sprint 8 — Code Quality & Stability Iteration

*(Runs after all feature sprints complete. No new features — only hardening.)*

---

## Models Summary

| Role | Model | Used For |
|------|-------|----------|
| **Orchestrator** | Claude Sonnet 4.6 | Planning, coordination, build verification, merge conflict resolution, git commits |
| **Fleet Worker** | Claude Haiku 4.5 | Individual todo implementation (parallel sub-agents) |
| **Explore** | Claude Haiku 4.5 | Codebase research, audit, exploration tasks |

All Fleet Worker agents run on **claude-haiku-4.5** for speed and parallelism.
The primary CLI session orchestrator runs on **Claude Sonnet 4.6**.
