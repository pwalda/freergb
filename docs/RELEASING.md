# Releasing

1. Set `version` in the root `Cargo.toml` (the app takes its version from there;
   `tauri.conf.json` has none). Use `X.Y.Z-beta.N` for pre-releases.
2. Commit, then tag and push: `git tag vX.Y.Z && git push origin main vX.Y.Z`.
3. The `Release` workflow checks that the tag matches the version, builds the
   Windows installer and the Linux AppImage, .deb and .rpm, and opens a **draft**
   GitHub Release with them and `SHA256SUMS.txt`. A version with `-` is marked
   as a pre-release.
4. Fill in the draft's notes from `.github/release-notes.md`: what's new, measured
   tray numbers, known issues. Then publish it.
5. Publishing runs `Update feed`: the release's `latest.json` goes to the
   `updates` branch and running apps offer the update (checked at start and
   daily). Its "What's new" section is what the app shows, so write that before
   publishing. Pre-releases are offered too; drafts never.

## Updater key

Installers carry a `.sig` the app checks against `plugins.updater.pubkey` in
`crates/freergb-app/tauri.conf.json`. Keep the private key out of the repository:

    npx @tauri-apps/cli@2 signer generate -w ~/.tauri/freergb-updater.key

Put the public key (`.key.pub`) in `tauri.conf.json`, and the private key and its
password in the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Without them releases build but offer no
update. Losing the key means installed apps can no longer update themselves.

## Signing

Windows builds ship unsigned until SignPath Foundation approves the project.
Then set the repository variable `SIGNPATH_ENABLED=true`, the secret
`SIGNPATH_API_TOKEN`, and the variables `SIGNPATH_ORGANIZATION_ID`,
`SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`. The workflow then signs the
installer before it is released.
