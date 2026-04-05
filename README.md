# inferno-windows

Windows port of [Inferno](https://github.com/teodly/inferno) — an unofficial open-source implementation of the Dante AoIP protocol.

`inferno_wasapi.exe` receives audio from a Dante network and plays it through Windows speakers via WASAPI.

## Quick Start

1. Install Rust: https://rustup.rs/
2. Open firewall: UDP ports 4455, 8700, 4400, 8800, 5353
3. Build: `cargo build --release -p inferno_wasapi`
4. List audio devices: `target\release\inferno_wasapi.exe --list-devices`
5. Run: `target\release\inferno_wasapi.exe`

## Notes

See `NOTES.md` for known issues and limitations, especially regarding PTP clock synchronization.
