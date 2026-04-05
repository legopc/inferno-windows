# Inferno Windows — Copilot Instructions

This is a Windows port of the Inferno Dante AoIP library. Key crates:

- `inferno_aoip`: Core Dante protocol library (ported from Linux)
- `searchfire`: mDNS discovery (already cross-platform)
- `usrvclock`: Clock overlay protocol (Windows stub — full PTP support is a future task)
- `inferno_wasapi`: Windows binary — receives Dante audio, plays via WASAPI

## Key Constraints
- Keep changes to `inferno_aoip/` minimal — only fix build errors
- PTP clock (Statime) is NOT implemented on Windows — document in NOTES.md
- All fixes go in this repo only
