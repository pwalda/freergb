# App icons

`icon.svg` is the source: the UI's logo (the lowercase f in LED dots, `ui/src/lib/Logo.svelte`)
on the rail's dark tile. The PNGs and `icon.ico` are generated from it:

    npx @tauri-apps/cli@2 icon crates/freergb-app/icons/icon.svg -o <scratch dir>

then copy `32x32.png`, `128x128.png`, `icon.png` and `icon.ico` here (the ones
`tauri.conf.json` lists).
