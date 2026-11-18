#!/usr/bin/env bash
# The Rust crates' licences for the About sheet: ui/generated/rust-licenses.txt, which the UI
# build folds into dist/licenses.txt (ui/licenses.ts). Needs cargo-about.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$root/ui/generated"
cd "$root"
cargo about generate about.hbs -o ui/generated/rust-licenses.txt
