# Upstream facts (verified from source)

Verified 2026-11-06 against the `release_1.1` tag of OpenRGB (commit
`8e16de27858cf2be6af54a33ce003df74cb9fd3e`, 2026-10-16), shallow-cloned read-only.
These answer the open questions of the original plan.

| Question | Answer | Evidence |
|---|---|---|
| Repo location | Codeberg `OpenRGB/OpenRGB` and GitLab `CalcProgrammer1/OpenRGB` are both live and `master` is at the same commit (`4f2d892`) on both on 2026-11-06. Treat Codeberg as primary, GitLab as mirror. README badges/links still point at GitLab. | `git ls-remote` on both |
| License | **GPL-2.0-or-later.** The original plan assumed GPL-2.0-only. Source files carry `SPDX-License-Identifier: GPL-2.0-or-later`; LICENSE is the standard GPLv2 text with the usual "any later version" clause. | `NetworkServer.cpp`, `NetworkProtocol.h` headers |
| Latest release | 1.1 (tag `release_1.1`). | `git ls-remote --tags` |
| SDK protocol version | **6** in 1.1. The SDK doc still labels 6 "unreleased"; that is stale. `OPENRGB_SDK_PROTOCOL_VERSION 6`. | `NetworkProtocol.h:28` |
| Profile + settings over SDK | Yes at protocol 6: ProfileManager (150-161) and SettingsManager (250-254) are handled by `NetworkServer.cpp`. | packet switch in `NetworkServer.cpp` |
| "Local client" rule | A client is *local* when its peer address is loopback. It becomes a **local client** by sending `NET_CLIENT_FLAG_REQUEST_LOCAL_CLIENT` (bit 16) in SET_CLIENT_FLAGS; the server confirms with `NET_SERVER_FLAG_LOCAL_CLIENT` in its flags reply. **Not exclusive**: every loopback client that asks is granted it, so a running OpenRGB GUI does not block us. Flags are only exchanged at protocol >= 6. | `SendReply_ServerFlags`, `NetworkServer.cpp:3725-3745` |
| Handlers gated on local client | Exactly these: I2C/HID/USB/serial/USB-serial info queries (120-124), all LogManager packets (300-303), profile save/delete/upload (151, 153, 154), settings modify/set/save (252-254). RGBController commands, profile list/load/download, settings *get* and rescan are **not** gated. | `if(!client_info->client_is_local_client)` sites, 15 handlers |
| ACKs | At protocol >= 6 the server ACKs **every** client packet (including UPDATELEDS) with `(acked_pkt_id, status)` and the packet's `dev_id` in the header. No sequence number: correlate by (dev_id, pkt_id) in FIFO order. A high-rate effects client must drain ACKs without blocking. Below v6 no ACKs are sent. | `SendAck` call sites, `NetworkServer.cpp:1159,1227,1587` |
| Server packets during handshake | Reply to REQUEST_PROTOCOL_VERSION is **three packets**: version (40), SET_SERVER_NAME (51, "OpenRGB 1.1"), ACK. SET_CLIENT_FLAGS gets server flags (53, `0x1007f` for a loopback requester) then ACK. | live capture |
| Matrix map | Upstream "empty" = height 0, width 0, no data; written as length 0. Segment, LED and color-list layouts match the doc. | `RGBController.cpp`, `RGBControllerInterface.h:265` |
| Wire layout vs doc | Controller-data layout matches `Documentation/OpenRGBSDK.md`. The `protocol_version < 6` branches in the serializer only clamp values (zone types, diagonal directions) for older clients; they do not change the layout. | `RGBController.cpp` `Get*DescriptionData` |
| `data_size` fields | Include their own 4 bytes (`reply_size = sizeof(reply_size)` first). | `NetworkServer.cpp:3659` |
| Update reason codes | Sequential from 0: UPDATELEDS, UPDATEMODE, SAVEMODE, CONFIGUREZONE, CLEARSEGMENTS, ADDSEGMENT, HIDDEN, UNHIDDEN, SETDEVICESPECIFICCONFIGURATION, SETDEVICESPECIFICZONECONFIGURATION, CONFIGUREDEVICE, DEVICE_CHANGED. The SDK doc leaves the values blank. | `RGBControllerInterface.h:41-57` |
| Event-triggered profiles | Stored as ProfileManager settings keys (e.g. `resume_profile`, `suspend_profile`), so they are reachable via SettingsManager at protocol 6. Full key list still to be enumerated against a running server. | `ProfileManager.cpp:66-141` |
| Server CLI | `--server`, `--server-host`, `--server-port`, `--config`, `--localconfig`, `--nodetect`, `--noautoconnect`, `--loglevel`. | `cli.cpp:420-461` |

## Existing Rust crates (crates.io, checked 2026-11-06)

| Crate | Version | Last update | Verdict |
|---|---|---|---|
| `openrgb` | 0.1.2 | 2022-05-14 | Unmaintained, predates protocol 5/6. Not used. |
| `openrgb2` | 0.4.1 | 2026-09-05 | Maintained. Protocol 6 / Profile+Settings coverage **unverified**. |

Decision: write our own codec (`freergb-protocol`). The mock server needs a codec anyway, and
Profile/Settings/unique-ID support is central to this app. `openrgb2` may be used as an
independent cross-check of our encoding.

## Known risk: mock and client sharing one reading of the spec

Both are built from the same codec, so a misreading would pass tests and fail against a real
server. **Mitigated 2026-11-06** by golden fixtures captured from a real OpenRGB 1.1 server (see
`crates/freergb-protocol/tests/fixtures/README.md`); remaining risk is hardware-only behaviour,
checked at each phase end.

## Verified live against a real OpenRGB 1.1 server (2026-11-06)

Server: official `OpenRGB_1.1_x86_64` AppImage, headless (`--server`), Linux, with fake devices
from the `DebugDevices` setting (works without hardware; types motherboard/dram/keyboard/mouse/
argb plus `CustomDevices` with arbitrary zones). Captures are in
`crates/freergb-protocol/tests/fixtures/real_server_1_1/`, produced by `tools/capture_fixtures.py`.

- **Codec validated:** all 8 real controller blocks (including a 300-LED device, 10 KB) decode and
  re-encode **byte-for-byte** with `freergb-protocol` at protocol 6.
- **ACK ordering:** for requests with a reply the order is reply, then ACK. For commands that change
  device state the server may send `SIGNAL_UPDATE` (1150) before or after the ACK. Both are normal.
- **ACK status quirks:** an unknown packet ID gets status 2. **UPDATELEDS / UPDATEZONELEDS accept
  fewer colors than LEDs (partial update from LED 0) and reject more colors than LEDs with status 5**
  (source: `SetColorDescription`; confirmed live). Packet `data_size` must equal the header size. A profile save without the local-client
  flag gets status 3 (not allowed). An UPDATELEDS to an unknown device ID was ACKed with status 0
  in our capture (surprising; do not rely on INVALID_ID for stale IDs, check the device store).
- **ResizeZone is a silent no-op unless the zone has `CONFIGURABLE_SIZE` (or effects-only) set**,
  but is still ACKed OK. The client must check zone flags before offering or sending a resize.
- **Profiles work headlessly** for a local client: save/list/delete, with `PROFILE_LIST_UPDATED` (160)
  pushed after save/delete. No profile management without the local-client flag.
- **Profile load + `PROFILE_ABOUT_TO_LOAD` (159):** the server sends 159 first and *waits* for the
  client. If we never answer, the load stalls about 1 s then continues. **An ACK does not clear it
  (still ~1.1 s); echoing an empty 159 packet clears it instantly (3 ms).** The SDK doc says
  "ACK"; the real server wants a 159 echo (which is what upstream's own client sends).
- **Profile load traffic is large:** about 30 KB `PROFILE_LOADED` (158) with the profile JSON, 3x
  `ACTIVE_PROFILE_CHANGED` (157), then one `SIGNAL_UPDATE` per device change, each carrying a full
  device block (45 packets for 8 devices). The client must parse these cheaply.
- **Event-triggered profiles are real settings keys** in the served `ProfileManager` schema:
  `open_profile`, `exit_profile`, `suspend_profile`, `resume_profile`,
  `service_startup_profile`, `service_shutdown_profile` (each of type `profile`). Whether a headless
  server *fires* suspend/resume is still unverified (needs OS power events; Windows hardware test).
- Settings: `GET_SETTINGS_SCHEMA` with an empty key returns every registered schema
  (`Detectors`, `LogManager`, `ProfileManager`, `Server`). `GET_SETTINGS` for an unset key returns the
  string `null`.
- A fresh headless server reports the `DebugDevices` it was configured with via normal detection;
  `--nodetect` yields zero devices.

## Caveat for the mock server

Upstream clamps values for clients below protocol 6 (zone types LINEAR_LOOP/MATRIX_LOOP_*/SEGMENTED
become LINEAR/MATRIX, diagonal directions become LEFT, the diagonal mode flag is stripped, and
effects-only resizable zones report size 0/1 below v5). `freergb-protocol` does **not** apply this
on encode, so a mock serving older-protocol scenarios must apply it itself or it will emit values a
real server never would.

## Still unverified

- Whether the headless server fires suspend/resume profile triggers.
- Whether a profile load reaches real hardware without client help. Source says yes: the server's load
  path calls `UpdateMode`/`UpdateLEDs`/`UpdateZoneMode` (`ProfileManager.cpp` 1475-1558), so the SDK doc's
  "client must call UpdateMode on all controllers" looks stale. Confirm on Paweł's hardware.
- Reply matching: every real reply echoes the request's `dev_id` (checked over all 30+ reply/request
  pairs in the fixtures), so the client matches replies on `(dev_id, pkt_id)`.
- Real-hardware behaviour of everything above on Windows.

## Settings writes (verified live against 1.1, 2026-11-06)

- Wire format: MODIFY (252) and SET (253) carry a NUL-terminated JSON **object** `{"<Key>": <value>}`;
  SAVE (254) has an empty payload. All three are ACKed and local-client only. GET (250) replies with
  the key's JSON (`null` when unset); schema (251) with an empty key returns every key's schema,
  with a non-empty key only that key's `properties`.
- MODIFY merges members into the stored object (nlohmann `update`); SET replaces the key. Both run
  `FilterSettingsAgainstSchema`: members absent from the schema or of the wrong type are dropped
  with a log warning. Event-profile members (type `profile`) are objects `{"enabled": bool, "name": str}`;
  a string is dropped.
- **Server abort:** if no member survives the filter (including a bare `{}` or a non-object), the
  filter returns JSON `null` and MODIFY calls `update(null)`, an uncaught `type_error`; the whole
  OpenRGB process terminates (`terminate called after throwing ... cannot use update() with null`).
  Reproduced twice. `freergb-sdk` validates every write against the schema first and refuses
  anything the server would drop. The mock reproduces the abort (`settings_aborts()`).
- SAVE persists to `OpenRGB.json` in the config dir (key appeared in the file after SAVE).

## Running the server headless, and its lifecycle (verified live against 1.1, 2026-11-06)

- `OpenRGB --server --noautoconnect --server-port N [--config DIR]` runs with **no window and no
  display** (checked with `DISPLAY`, `WAYLAND_DISPLAY` and `QT_QPA_PLATFORM` unset): no `--gui`
  means CLI mode. Ready to answer in well under a second with only debug devices; real SMBus
  detection takes longer, so the supervisor allows 60 s.
- `--noautoconnect` ("do not try to autoconnect to a local server at startup") is kept: without it a
  fresh instance may attach as a client to a server that appeared between our probe and our launch.
- SIGTERM stops it cleanly (listener gone within 3 s). `--config DIR` redirects `OpenRGB.json`,
  `profiles/` and logs.
- **Controller IDs are per server process.** `NetworkServer.cpp` keeps `controller_next_idx`
  (starts at 0, +1 per newly seen controller object), so IDs are stable across hotplug and rescans
  within one process but are assigned in detection order after a restart. Verified: restarting
  with one device removed shifted the IDs of every device after it. The spec's "use stable IDs"
  holds within one connection only; persisted state must use `freergb_sdk::DeviceKey`
  (name, location, serial, nth-of-identical).
- **`location` may not be stable for real hardware (UNVERIFIED, from source only).** Controllers
  build it from the transport, e.g. `"HID: " + path` (a `/dev/hidrawN` node on Linux, which can
  change on replug/reboot), `"USB: "`, `"PCI: "`, `"IP: "`. Debug devices are the only ones tested.
  `freergb_sdk::resolve` therefore matches persisted keys in tiers (exact; then name+serial; then
  name in order), one-to-one. Needs real-hardware captures before it can be trusted.
- **"Connected" is not "detection finished".** A freshly (re)started server can complete the SDK
  handshake before detection has produced any device: an empty device list at connect in 3 of 150
  supervised restarts. The devices then arrive as list updates (packet 100), which the client
  applied (first live confirmation of list updates). UI and effects must treat the device list as
  live and never conclude "no devices" from a fresh connection.
- A peer that accepts TCP but never speaks the protocol is indistinguishable from OpenRGB < 0.5
  (which also never answers a version request). The SDK reports both as `NotOpenRgb`.

## Windows, unverified

- A closed loopback port reportedly takes ~2 s to refuse on Windows. The supervisor treats a
  connect timeout as "nothing listening" (not "something dropped us"), and the TCP connect has its
  own timeout (`ConnectOptions::connect_timeout`, 5 s) apart from the handshake timeout. The
  mapping is unit-tested; the Windows behaviour itself has not been run.
- Process-group stop/kill, `kill_on_drop` of a wrapper's grandchildren, and the SIGTERM grace are
  Unix-only code paths (`cfg(unix)`); on Windows only the direct child is killed. A Job Object
  would be the equivalent; not built.

## Supervisor decisions to confirm with the owner

- If a *foreign* server (e.g. the user's OpenRGB GUI) disappears, the supervisor waits
  `foreign_grace` (default 5 s) for it to return, then launches the bundled server. The alternative
  is to keep waiting forever. Default chosen so a tray app keeps controlling lights after the user
  quits OpenRGB.
- An OpenRGB orphaned by a crashed backend looks foreign: it is adopted and never killed (pinned by
  a test). A pidfile with an exe-path check, and a Windows Job Object, would let us reclaim it; not
  built.

## Streaming LEDs from the effects engine (verified live against 1.1, 2026-11-07)

- A rainbow on all 8 debug devices (41, 10, 104, 11, 70, 60, 300 and 6 LEDs) at 30 fps for 5 s:
  1208 frames accepted, 0 dropped, no ACK stall. The client's flow control (`max_unacked` 64)
  and a second client reading controller data both behaved. Debug devices do no hardware I/O,
  so this shows the protocol path keeps up, not that real controllers do: slow USB or SMBus
  devices set their own limit and are UNVERIFIED.
- `SET_CUSTOM_MODE` leaves each device in "Direct" (index 0). **Restoring the previous mode is
  NOT verified live:** every debug device has exactly one mode ("Direct"), so there is nothing
  to switch away from or back to. From the source, `UPDATE_MODE` sets the mode values and the
  active mode, and `Client::update_mode` refreshes the device afterwards, so the client's cache
  follows; the mock and fake-backend tests cover the engine's logic. Needs a device with two or
  more modes (any real hardware) to confirm.
- A second client's `REQUEST_CONTROLLER_DATA` returns the colours the server currently holds,
  which is how the real-server test reads back what was streamed.

## Phase 4 packets (verified from release_1.1 source, 2026-11-13..15; not yet live)

- **102 DETECTION_PROGRESS:** `data_size`, u32 percent, string16 running detector name. Some
  servers send no end packet, so the app gives up waiting after 120 s.
- **1001/1002 segments:** the server checks no zone flag and calls `SaveConfiguration`.
- **1102 save mode:** sets the mode values and calls only `SaveMode()`, never `UpdateMode()`;
  the client offers it only for `MANUAL_SAVE` modes.
- **1103 zone mode:** mode index -1 means "follow the device" and carries no mode data.
- **1004/1005:** rename works only on `MANUALLY_CONFIGURABLE_NAME` devices (only the debug
  controller); the hidden flag is not saved. freeRGB keeps both itself.
- **120-124 (local only):** 120 sends raw `i2c_smbus_info` structs (char[512] name, six i32);
  121 u16 vid/pid/release/usage_page/usage, i32 interface, string16 serial/manufacturer/
  product/path; 122 u16 vid/pid + three string16; 123 string16 port names; 124 u16 vid/pid +
  string16 port and USB path. Each list is `data_size` + u16 count.
- **300-304 log (local only):** the server buffers lines for clients only while
  `LogManager.log_console` is true, and reads that setting only at startup (default false).
  301 makes the server send the buffer as 304 packets, then the ACK; 304 = `data_size`, u32
  level/line/timestamp_ms, string16 file and text. 302/303 set the level of OpenRGB's own log
  file only. Levels: 0 fatal .. 6 trace, 7 dialog.
