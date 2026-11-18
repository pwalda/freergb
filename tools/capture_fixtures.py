#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Capture golden packets from a REAL OpenRGB SDK server.

Independent of the Rust codec on purpose (stdlib only, written from the SDK
doc/source), so fixtures cross-check freergb-protocol instead of echoing it.

usage: capture_fixtures.py PORT OUTDIR
Writes OUTDIR/NN_<name>.bin (one raw server packet, header included) and
OUTDIR/manifest.json describing each capture.
"""
import json, socket, struct, sys, time
from pathlib import Path

port, out = int(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
manifest, n = [], 0

def hdr(dev, pid, size): return b"ORGB" + struct.pack("<III", dev, pid, size)

def recv_exact(s, k):
    b = b""
    while len(b) < k:
        c = s.recv(k - len(b))
        if not c: raise EOFError
        b += c
    return b

def recv_packet(s, timeout=1.5):
    s.settimeout(timeout)
    try:
        h = recv_exact(s, 16)
    except (socket.timeout, EOFError):
        return None
    assert h[:4] == b"ORGB", h
    dev, pid, size = struct.unpack("<III", h[4:])
    return dev, pid, recv_exact(s, size) if size else b""

s = socket.create_connection(("127.0.0.1", port))

def send(dev, pid, payload=b""):
    s.sendall(hdr(dev, pid, len(payload)) + payload)

def step(name, dev, pid, payload=b"", note="", expect_reply=True):
    """Send one request; save every reply packet that arrives."""
    global n
    send(dev, pid, payload)
    got = []
    while True:
        p = recv_packet(s, 1.5 if expect_reply or got else 0.4)
        if p is None: break
        d, rpid, body = p
        n += 1
        fn = f"{n:02d}_{name}_resp{len(got)}.bin"
        (out / fn).write_bytes(hdr(d, rpid, len(body)) + body)
        manifest.append(dict(file=fn, step=name, request_pkt_id=pid, request_dev_id=dev,
                             request_payload_hex=payload.hex(), response_pkt_id=rpid,
                             response_dev_id=d, size=len(body), note=note))
        got.append(p)
    print(f"{name:28} -> {[(g[1], len(g[2])) for g in got]}")
    return got

cstr = lambda t: t.encode() + b"\0"
step("protocol_version", 0, 40, struct.pack("<I", 6), "client advertises 6")
step("client_name", 0, 50, cstr("freergb-capture"), expect_reply=False)
# rgbcontroller|log|profile|plugin|settings | request local client
FLAGS = 0b11111 | (1 << 16)
step("client_flags", 0, 52, struct.pack("<I", FLAGS), "server flags reply expected")
cc = step("controller_count", 0, 0)
ids = struct.unpack_from("<%dI" % struct.unpack_from("<I", cc[0][2])[0], cc[0][2], 4)
print("controller ids:", ids)
names = {}
for cid in ids:
    r = step(f"controller_data_id{cid}", cid, 1, struct.pack("<I", 6))
    body = r[0][2]
    (nlen,) = struct.unpack_from("<H", body, 8)
    names[cid] = body[10:10 + nlen - 1].decode()
print(names)
argb = next((c for c, nm in names.items() if nm == "Debug ARGB"), None)
strip = next((c for c, nm in names.items() if nm == "Resizable Strip"), None)
if strip is not None:
    # Real finding: ResizeZone on a zone WITHOUT the configurable-size flag is a
    # silent no-op that is still ACKed OK. Capture that, then a real resize.
    step("resize_unflagged_zone", strip, 1000, struct.pack("<ii", 0, 90), "no flag: ACK ok, nothing changes")
    step("redecode_after_unflagged_resize", strip, 1, struct.pack("<I", 6))
if argb is not None:
    step("resize_flagged_zone", argb, 1000, struct.pack("<ii", 1, 40), "Debug ARGB zone 1 (Resizable Zone) 0 -> 40")
    step("redecode_after_resize", argb, 1, struct.pack("<I", 6))
if strip is not None:
    step("set_custom_mode", strip, 1100)
    cols = b"".join(bytes([i % 256, 0, 255 - i % 256, 0]) for i in range(90))
    body = struct.pack("<IH", 4 + 2 + len(cols), 90) + cols
    step("update_leds", strip, 1050, body, "90 colors")
    step("update_zone_leds", strip, 1051, struct.pack("<IIH", 4 + 4 + 2 + 8, 0, 2) + b"\xff\x00\x00\x00\x00\xff\x00\x00")
    step("update_single_led", strip, 1052, struct.pack("<i", 3) + b"\x01\x02\x03\x00")
step("profile_list", 0, 150)
step("profile_active", 0, 156)
step("plugin_list", 0, 200)
step("settings_schema_all", 0, 251, cstr(""))
for key in ("General", "Server", "LogManager", "Detectors", "ProfileManager"):
    step(f"settings_get_{key}", 0, 250, cstr(key))
step("ack_invalid_controller", 99999, 1050, struct.pack("<IH", 6, 0), "UPDATELEDS to unknown id")
step("ack_unknown_packet", 0, 9999, b"", "unknown packet id")
step("log_level", 0, 302)
step("i2c_bus_info", 0, 120)
step("usb_device_info", 0, 122)
(out / "manifest.json").write_text(json.dumps(manifest, indent=2))
print(f"{n} packets saved to {out}")
