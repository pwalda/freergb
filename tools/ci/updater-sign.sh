#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Writes <file>.sig, the signature the in-app updater checks, for every file given.
# Without the TAURI_SIGNING_PRIVATE_KEY secret the release goes out without them, and
# publishing it offers no update (see .github/workflows/updates.yml).
set -euo pipefail
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "::warning::TAURI_SIGNING_PRIVATE_KEY is not set: no updater signatures"
  exit 0
fi
for f in "$@"; do
  npx --yes @tauri-apps/cli@2 signer sign "$f"
done
