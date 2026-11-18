# Golden fixtures

`real_server_1_1/` holds raw packets captured from a **real OpenRGB 1.1** server (official
x86_64 AppImage, headless, Linux) by `tools/capture_fixtures.py`. The server was configured with
`DebugDevices` so it reports fake devices without hardware: motherboard, DRAM, keyboard (real
keyboard matrix map), mouse, an ARGB device with a resizable zone, a resizable strip, a 300-LED
device and a small matrix pad. `manifest.json` lists each capture with its triggering request.

The capture script is independent of the Rust codec (stdlib Python, written from the SDK doc and
serializer source), so `tests/real_server.rs` is a genuine cross-check.

To regenerate: run an OpenRGB 1.1 server with a `DebugDevices` config on port N, then
`python3 tools/capture_fixtures.py N crates/freergb-protocol/tests/fixtures/real_server_1_1`.
Captures from real *hardware* are still wanted at the first hardware milestone.
