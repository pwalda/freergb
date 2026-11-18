# UI browser check

Runs the built UI (`ui/dist`) in headless Chromium against a fake Tauri backend (a stub of
`window.__TAURI_INTERNALS__`), and checks what the UI sends to the backend: per-device
isolation, debounced sliders (and an edit still waiting when you switch target), Undo, Stop all
and Resume, number-key shortcuts, device modes, zone sizes, stage shapes and clicking a device on
the stage, painting by hand, profiles (save, apply with Undo, rename, delete), a late echo, a rejected edit.
It is not in CI because it needs a browser download. Run it before changing `ui/src/lib`.

```
npm --prefix ui run build
cd tools/ui-e2e
npm install
npx playwright-core install chromium-headless-shell     # once; ~100 MB
node run.mjs [screenshot-dir]                           # prints PASS/FAIL lines; exit 1 on any FAIL
```

Options: `CHROMIUM_EXE=/path/to/chrome` uses that binary instead of playwright's download.
With a screenshot directory it also writes `user-960.png` (the default 960x640 window),
`desktop.png`, `mobile.png`, `960-light.png` and the painter (`paint-960.png`, `paint-960-light.png`,
`paint-mobile.png`) and the profile sheet (`profile-sheet.png`, `profile-sheet-light.png`).

On a Linux box without sudo and without `libnspr4`/`libnss3`: `apt-get download libnspr4 libnss3`,
extract both debs with `dpkg -x`, and run with `LD_LIBRARY_PATH=<extract>/usr/lib/x86_64-linux-gnu`.

The fake backend only models the commands the UI calls (`server_state`, `devices`, `scenes`,
`set_scenes`, `resize_zone`, `retry_server`, and the profile commands); an unknown command throws, so a new IPC call fails loudly here
until the stub learns it.
