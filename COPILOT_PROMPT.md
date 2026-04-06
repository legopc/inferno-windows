# Copilot CLI Handoff — inferno-windows

Use this as your starting prompt when handing control to Copilot CLI on the Windows build machine.

---

## Paste this into Copilot CLI to start

```
You are continuing development on the inferno-windows project at the local repo root.
Start by reading .github/copilot-instructions.md — it has the full current state,
architecture, build instructions, and task list.

The immediate goal is to get the Dante TX mode working end-to-end:
Windows audio → SYSVAD virtual speaker → WASAPI loopback → Dante network.

Work through the "Priority 1" tasks in copilot-instructions.md in order.
Commit each logical change with a clear message. Do not break RX mode.
```

---

## Quick machine check (run these first to confirm environment)

```powershell
# Confirm Rust toolchain
rustup show

# Confirm build works
cargo check --workspace

# Confirm GitHub auth
gh auth status

# Confirm test signing active (should show "Test Mode" in corner after reboot)
bcdedit | Select-String "testsigning"
```

## Where things stand

| | Status |
|---|---|
| RX mode (Dante → WASAPI speakers) | ✅ Working |
| TX code (WASAPI loopback → Dante) | ✅ Compiled, not yet tested on hardware |
| SYSVAD virtual speaker driver | ⬜ Needs build + install |
| Firewall ports | ⬜ May need opening |
| End-to-end TX test | ⬜ Not done |

## Key commands for testing

```powershell
# Build
cargo build --release

# Test RX (device should appear in Dante Controller)
.\target\release\inferno_wasapi.exe

# Test TX (needs SYSVAD installed first)
.\target\release\inferno_wasapi.exe --tx --tx-device "Tablet Audio" --name MyPC-TX
```
