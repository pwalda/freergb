#!/usr/bin/env bash
# Builds the sensor service into crates/freergb-app/sensord/, where the Windows bundle takes
# it from (tauri.windows.conf.json). Windows only.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cargo build --release -p freergb-sensord --manifest-path "$root/Cargo.toml"
mkdir -p "$root/crates/freergb-app/sensord"
cp "$root/target/release/freergb-sensord.exe" "$root/crates/freergb-app/sensord/"
