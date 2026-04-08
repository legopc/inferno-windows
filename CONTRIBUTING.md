# Contributing to Inferno-Windows

Thank you for your interest in contributing! Inferno-Windows is a free, open-source
Windows implementation of the Dante AoIP protocol stack.

---

## Getting Started

### Prerequisites

- **Rust** 1.75+ (install via [rustup](https://rustup.rs))
- **Windows 10/11** (WASAPI and named pipe APIs are Windows-only)
- **SYSVAD virtual audio driver** (for TX mode testing without hardware — see [SETUP.md](SETUP.md))
- WiX Toolset v3.x (only required to build the MSI installer)

### Build & Test

```powershell
# Clone
git clone https://github.com/legopc/inferno-windows.git
cd inferno-windows

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run checks (CI equivalent)
cargo check --workspace
cargo clippy --workspace
cargo fmt --check
```

### Run Locally (RX mode)

```powershell
cargo run -p inferno_wasapi -- --name "MyDevice" --rx-channels 2
```

See [README.md](README.md) for full CLI reference.

---

## Code Style

- **Format**: `cargo fmt` before every commit. CI enforces this.
- **Lint**: `cargo clippy -- -D warnings`. Fix all warnings before submitting.
- **Unsafe**: Avoid `unsafe` blocks unless absolutely necessary. Document any `unsafe` with a `// SAFETY:` comment explaining the invariant.
- **Error handling**: Use `anyhow` for application-level errors. Avoid `unwrap()` in non-test code; prefer `?` or explicit error handling.
- **Logging**: Use `tracing::{error!, warn!, info!, debug!, trace!}` — never `println!` in production paths.
- **Comments**: Explain *why*, not *what*. Code should be self-documenting for the *what*.

---

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <short summary>

[optional body]

Co-authored-by: Your Name <you@example.com>
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`

**Examples:**
```
feat: add FPP config field to config.toml
fix: ring_buffer power-of-2 alignment in new_owned()
docs: update README with TX mode CLI flags
test: add IPC serde round-trip integration tests
```

Keep the subject line under 72 characters. Use the body for breaking changes or detailed rationale.

---

## Pull Requests

1. **Branch** from `master` with a descriptive name: `fix/ring-buffer-alignment`, `feat/fpp-negotiation`
2. **Keep PRs focused** — one logical change per PR. Split large changes into a series.
3. **Tests**: Add or update tests for any new behaviour or bug fix.
4. **CI must pass** — all three CI jobs (check, build, test) must be green before merge.
5. **Describe your change** in the PR body: what problem does it solve, how, and what testing was done.
6. **Reference issues** with `Fixes #N` or `Relates to #N` where applicable.

---

## Project Structure

```
inferno-windows/
├── inferno_aoip/        # Core Dante protocol stack (RTP, ARC, CMC, mDNS)
├── inferno_wasapi/      # Main Windows binary: WASAPI + IPC + service mode
├── inferno_gui/         # Win32 GUI status window (native-windows-gui, no GPU)
├── inferno2pipe/        # Utility: pipe Dante audio to stdout as raw PCM
├── searchfire/          # mDNS server/client library (vendored + modified)
├── usrvclock/           # PTP clock stub (non-functional on Windows currently)
└── docs/                # Extended documentation
```

For architecture details see [NOTES.md](NOTES.md) and crate-level README files.

---

## Areas Needing Help

- **TX mode testing** — requires a Dante receiver on the network or Dante Controller
- **FPP negotiation wiring** — `channels_subscriber.rs:946` has a documented TODO
- **Realtime memory management** — `flows_rx.rs:423`, `flows_tx.rs:397` have FIXME notes about deallocation in RT threads
- **PTP clock on Windows** — `usrvclock` is a stub; a real PTP implementation would significantly improve clock accuracy
- **MSI CI integration** — WiX Toolset needs to be installed in the CI runner

---

## Reporting Issues

Please include:
- Windows version (`winver`)
- Rust version (`rustc --version`)
- Dante hardware/software in use (Dante Controller version, device names)
- Relevant log output (`RUST_LOG=debug cargo run -p inferno_wasapi -- ...`)
- Steps to reproduce

---

## License

By contributing you agree your changes are licensed under the same terms as the project.
See the [LICENSE](LICENSE) file (if present) or the root `Cargo.toml` for the license identifier.
