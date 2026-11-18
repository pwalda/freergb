# Installing freeRGB

freeRGB is a front-end for [OpenRGB](https://openrgb.org): OpenRGB talks to the hardware, freeRGB
gives it a calmer window, effects and a tray. Downloads are on the
[releases page](https://github.com/pwalda/freergb/releases); `SHA256SUMS.txt` there lists a
checksum for every file.

## Windows 10 and 11

Run `freeRGB_<version>_x64-setup.exe`. It asks for administrator rights once, because it sets up
two background services.

**"Windows protected your PC".** While freeRGB is in beta its installer is not code-signed, so
SmartScreen warns about it. Check the file against `SHA256SUMS.txt`
(`Get-FileHash freeRGB_*-setup.exe` in PowerShell), then choose **More info**, **Run anyway**.

What the installer does:

- **Installs OpenRGB 1.1** in freeRGB's folder and starts it as the service *OpenRGB for
  freeRGB*, so your lights work from the moment Windows starts, before anyone logs in. If OpenRGB
  is already installed, it asks first: replace it with the included version (this uninstalls
  the existing one) or keep it and use it.
- **Offers PawnIO**, the driver OpenRGB needs for RGB memory and many motherboards. It is
  downloaded from PawnIO's official release, never bundled. If PawnIO is already installed, it
  offers a reinstall, which is rarely needed.
- **Starts the sensor service** *freeRGB sensors*. It reads the CPU temperature through PawnIO
  for looks that follow it, because Windows gives normal programs no CPU temperature. It reads
  nothing else. Without PawnIO, CPU temperature looks can use
  [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) instead,
  while it runs.

**Updates.** freeRGB checks for a new version when it starts and once a day. When one is out,
"Update ready" appears beside the settings button; nothing installs until you choose Install.

**Uninstalling** (Settings > Apps) removes freeRGB, its OpenRGB and both services. PawnIO stays,
since other programs may use it; it has its own entry in Settings > Apps. freeRGB's settings
stay in `%APPDATA%\dev.freergb.app` in case you come back; delete that folder to remove them.

## Linux

| Package | For |
|---|---|
| `freergb_<version>_amd64.deb` | Debian, Ubuntu, Linux Mint, Pop!_OS |
| `freergb-<version>-1.x86_64.rpm` | Fedora, openSUSE |
| `freergb_<version>_amd64.AppImage` | Any distro: make it executable and run it |

The AppImage needs FUSE 2 on some distros (the `libfuse2` or `fuse2` package). The packages keep
themselves up to date through freeRGB's own updater, which asks for your password to install.

### OpenRGB

freeRGB does not include OpenRGB on Linux; install it from your distro (the .deb and .rpm
recommend it):

    sudo apt install openrgb        # Debian, Ubuntu
    sudo dnf install openrgb        # Fedora

or from [openrgb.org](https://openrgb.org/releases.html). When no OpenRGB is running, freeRGB
starts the `openrgb` it finds on your PATH in the background. An OpenRGB AppImage or Flatpak is
not on the PATH: start it yourself with its SDK server on (in OpenRGB, the SDK Server tab, or
`--server`), and freeRGB connects to it.

### Device access (udev rules)

OpenRGB running as you can reach USB, HID and SMBus devices only through OpenRGB's udev rules.
Distro packages of OpenRGB install them; an OpenRGB AppImage, Flatpak or own build does not.

When they are missing and no devices show up, freeRGB offers **Set up device access** (on the
empty stage, and in Settings > Diagnostics > Hardware). It asks for your password once, then:

- installs OpenRGB 1.1's own rules as `/etc/udev/rules.d/60-openrgb.rules`,
- loads the `i2c-dev` kernel module now and at every boot
  (`/etc/modules-load.d/i2c-dev.conf`), which RGB memory and motherboards need,
- reloads udev and has OpenRGB look for devices again.

To do the same by hand, take `60-openrgb.rules` from OpenRGB's release or source, then:

    sudo cp 60-openrgb.rules /etc/udev/rules.d/
    echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf
    sudo modprobe i2c-dev
    sudo udevadm control --reload-rules && sudo udevadm trigger

### RGB memory and motherboards

These sit on the SMBus. Besides `i2c-dev`, the bus needs its chipset driver, which most distros
load by themselves: `i2c-i801` (Intel) or `i2c-piix4` (AMD). OpenRGB's
[SMBus guide](https://codeberg.org/OpenRGB/OpenRGB/src/commit/8e16de27858cf2be6af54a33ce003df74cb9fd3e/Documentation/SMBusAccess.md)
covers the rest, including two known cases:

- Some Gigabyte/Aorus motherboards have an ACPI conflict with the SMBus controller, bypassed
  with the `acpi_enforce_resources=lax` kernel parameter. It changes how the kernel shares
  hardware with the firmware, so read the guide first.
- Kingston Fury DDR5 can be claimed by the `spd5118` kernel driver (`i2cdetect` shows `UU` on
  the memory's bus); unloading it (`sudo rmmod spd5118`) frees the memory for OpenRGB.

### Where things live

Settings: `~/.config/dev.freergb.app`. Logs from OpenRGB: Settings > Diagnostics > Log in freeRGB.
