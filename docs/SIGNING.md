# Windows code signing

Unsigned installers make Windows SmartScreen warn ("Windows protected your PC") until enough
people have run the file. Signing names the publisher and lets reputation build up; it does not
remove the warning on day one, and EV certificates stopped skipping SmartScreen in 2024. This is
a decision for the maintainer; the release workflow is ready for SignPath (see
`docs/RELEASING.md`). Prices as found in November 2026; check them before buying.

## Options

| Option | Cost | Publisher shown | Fits freeRGB? |
|---|---|---|---|
| [SignPath Foundation](https://signpath.org) | Free for OSI-licensed projects | "SignPath Foundation" | Yes, after the changes below. Each release needs a manual approval in SignPath. |
| [Certum Open Source Code Signing](https://shop.certum.eu/open-source-code-signing.html) | €69 with a crypto card and reader (Certum's shop); a cloud variant is reported from about €49 | "Open Source Developer, Paweł Walda" | Yes: for developers sharing free or open-source software. The card cannot sign in GitHub's hosted CI, so releases would be signed by hand; the cloud variant can, with its one-time-password seed stored as a secret. |
| [Azure Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/) | $9.99 a month | Your validated legal name | Probably not: individuals are reported as eligible only in the USA and Canada. |
| OV certificate from a commercial CA | About $200-500 a year | Your name | Works, but costs the most and still starts with no reputation. |

**Recommendation:** apply to SignPath Foundation; if it declines or takes too long, buy Certum's
Open Source certificate.

## What SignPath requires, and where freeRGB stands

From [signpath.org/terms](https://signpath.org/terms):

| Requirement | Status |
|---|---|
| OSI licence, no commercial dual-licensing | Done: GPL-2.0-or-later. |
| "May not contain any proprietary, non open-source component" | **Blocker.** The installer carries OpenRGB's `dxcompiler.dll` and `dxil.dll` (Microsoft redistributables) and its `PawnIOLib.dll` (PawnIO's official build). See below. |
| Upstream open-source binaries may be bundled unsigned, and must not be signed under our subscription | OK: OpenRGB, Qt, libusb and HIDAPI stay as OpenRGB ships them. Sign only `freeRGB.exe`, `freergb-sensord.exe` and the installer. |
| Built from the repository by an automated build | Done: `.github/workflows/release.yml`. |
| A "Code signing policy" on the home and download pages, with "Free code signing provided by SignPath.io, certificate by SignPath Foundation", the team roles and members, and a privacy statement | To do when accepted: a README section and a line in the release notes. |
| Privacy: say what goes over the network | freeRGB contacts only `raw.githubusercontent.com` (the daily update check) and the OpenRGB server on this PC; the installer downloads PawnIO from its official release when asked. |
| Multi-factor authentication for every team member, on GitHub and SignPath | Check that the GitHub account has 2FA. |
| Product name and version in the files' metadata | Done by Tauri for `freeRGB.exe`; `freergb-sensord.exe` needs a version resource (small change). |
| Already released in the form to be signed | The first beta, unsigned, satisfies this. |

### The three proprietary files

- **`dxcompiler.dll`, `dxil.dll`**: Qt loads them for Direct3D shaders in OpenRGB's window.
  freeRGB runs OpenRGB headless (`--server`), so it most likely never loads them. Test on
  Windows: delete both from the installed `openrgb` folder and check that OpenRGB's service
  still starts and lights devices. If it does, `tools/ci/fetch-openrgb.sh` drops them.
- **`PawnIOLib.dll`**: PawnIO's official builds are proprietary (redistributable); its
  open-source edition is LGPL-2.1. Either build PawnIOLib from its public source in CI and ship
  that instead, or check whether OpenRGB finds the copy the PawnIO installer puts in
  `C:\Program Files\PawnIO` and stop shipping one.

## Application draft (SignPath Foundation)

To paste into the form at [signpath.org/apply](https://signpath.org/apply):

> **Project:** freeRGB, https://github.com/pwalda/freergb
>
> **Description:** freeRGB is a desktop front-end for OpenRGB, the open-source RGB lighting
> controller. It gives OpenRGB a simpler window, software effects across devices, profiles and
> a tray, on Windows and Linux. It is a separate client of OpenRGB's SDK server, not a fork.
>
> **Licence:** GPL-2.0-or-later, matching OpenRGB.
>
> **What we sign:** the Windows installer (NSIS) built by GitHub Actions from the tagged
> source, the app executable `freeRGB.exe`, and `freergb-sensord.exe`, a small service that
> reads the CPU temperature through the PawnIO driver. The installer also carries OpenRGB 1.1
> and its open-source libraries unmodified and unsigned by us, with their source attached to
> every release.
>
> **Maintainer:** Paweł Walda (GitHub: pwalda), author, reviewer and approver.
>
> **Releases:** https://github.com/pwalda/freergb/releases (first beta: v0.1.0-beta.1).
