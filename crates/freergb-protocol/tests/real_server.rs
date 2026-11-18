// SPDX-License-Identifier: GPL-2.0-or-later
//! Golden tests against packets captured from a REAL OpenRGB 1.1 server
//! (tools/capture_fixtures.py, debug devices; see fixtures/README.md).
//! These are what stop the codec and the mock agreeing on a wrong format.

use freergb_protocol::ids::*;
use freergb_protocol::messages::*;
use freergb_protocol::*;
use std::fs;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real_server_1_1")
}

fn captures() -> Vec<(String, Packet)> {
    let mut v: Vec<_> = fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    v.sort();
    v.into_iter()
        .map(|p| {
            let bytes = fs::read(&p).unwrap();
            let (pkt, used) = Packet::decode(&bytes).unwrap().expect("complete packet");
            assert_eq!(used, bytes.len(), "{p:?} has trailing bytes");
            (p.file_name().unwrap().to_string_lossy().into_owned(), pkt)
        })
        .collect()
}

#[test]
fn every_capture_frames_and_reencodes_identically() {
    let caps = captures();
    assert!(caps.len() > 40, "fixtures missing?");
    for (name, pkt) in caps {
        let again = pkt.encode();
        let (back, _) = Packet::decode(&again).unwrap().unwrap();
        assert_eq!(back, pkt, "{name}");
    }
}

#[test]
fn real_controller_data_decodes_and_reencodes_byte_for_byte() {
    let mut seen = Vec::new();
    for (name, pkt) in captures() {
        if pkt.pkt_id != pkt::REQUEST_CONTROLLER_DATA {
            continue;
        }
        let c = ControllerData::decode_packet(&pkt.payload, 6)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let again = c.encode_packet(6).unwrap();
        assert_eq!(
            again, pkt.payload,
            "{name}: re-encode differs from the real server's bytes"
        );
        seen.push(c.name);
    }
    for want in [
        "Debug Motherboard",
        "Debug Keyboard",
        "Resizable Strip",
        "Big 300",
        "Matrix Pad",
    ] {
        assert!(
            seen.iter().any(|n| n == want),
            "no fixture decoded for {want}: {seen:?}"
        );
    }
}

fn decoded(step_contains: &str) -> Vec<ControllerData> {
    captures()
        .into_iter()
        .filter(|(n, p)| n.contains(step_contains) && p.pkt_id == pkt::REQUEST_CONTROLLER_DATA)
        .map(|(_, p)| ControllerData::decode_packet(&p.payload, 6).unwrap())
        .collect()
}

fn by_name(name: &str) -> ControllerData {
    decoded("controller_data_id")
        .into_iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no {name}"))
}

#[test]
fn real_big_device_has_300_leds() {
    assert_eq!(by_name("Big 300").leds.len(), 300);
}

#[test]
fn real_keyboard_exposes_a_matrix_map_with_empty_cells() {
    let kb = by_name("Debug Keyboard");
    let m = kb
        .zones
        .iter()
        .find_map(|z| z.matrix_map.as_ref())
        .expect("keyboard matrix map");
    assert!(m.height > 1 && m.width > 1);
    assert_eq!(m.data.len(), (m.height * m.width) as usize);
    assert!(
        m.data.contains(&u32::MAX),
        "real keyboard maps use 0xFFFFFFFF for empty cells"
    );
    let used: Vec<_> = m.data.iter().filter(|&&c| c != u32::MAX).collect();
    assert!(used.iter().all(|&&c| (c as usize) < kb.leds.len()));
}

#[test]
fn real_matrix_pad_decodes() {
    let pad = by_name("Matrix Pad");
    assert_eq!(pad.leds.len(), 6);
    let m = pad.zones[0].matrix_map.as_ref().expect("map");
    let mut cells = m.data.clone();
    cells.sort_unstable();
    assert_eq!(cells, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn real_unflagged_resize_is_acked_ok_but_changes_nothing() {
    // Documents upstream behaviour the UI must respect: only zones with
    // CONFIGURABLE_SIZE (or effects-only) can be resized; others silently ignore it.
    let before = &decoded("controller_data_id5")[0];
    let after = &decoded("redecode_after_unflagged_resize")[0];
    assert_eq!(
        before.zones[0].flags & (zone_flag::CONFIGURABLE_SIZE | zone_flag::SIZE_EFFECTS_ONLY),
        0
    );
    assert_eq!(before.zones[0].leds_count, after.zones[0].leds_count);
    let caps = captures();
    let (_, ack) = caps
        .iter()
        .find(|(n, p)| n.contains("resize_unflagged_zone") && p.pkt_id == pkt::ACK)
        .unwrap();
    assert_eq!(
        decode_ack(&ack.payload).unwrap(),
        (pkt::RESIZE_ZONE, status::OK)
    );
}

#[test]
fn real_flagged_resize_changes_zone_and_led_count() {
    let before = by_name("Debug ARGB");
    let after = &decoded("redecode_after_resize")[0];
    let (zi, z0) = before
        .zones
        .iter()
        .enumerate()
        .find(|(_, z)| z.flags & zone_flag::CONFIGURABLE_SIZE != 0)
        .unwrap();
    assert_eq!(zi, 1);
    assert_eq!((z0.leds_min, z0.leds_max, z0.leds_count), (0, 100, 0));
    assert_eq!(after.zones[zi].leds_count, 40);
    assert_eq!(after.leds.len(), before.leds.len() + 40);
}

#[test]
fn real_handshake_values() {
    let caps = captures();
    let find = |step: &str, id: u32| {
        caps.iter()
            .find(|(n, p)| n.contains(step) && p.pkt_id == id)
            .map(|(_, p)| p.clone())
            .unwrap()
    };
    assert_eq!(
        decode_protocol_version(&find("protocol_version", pkt::REQUEST_PROTOCOL_VERSION).payload)
            .unwrap(),
        6
    );
    assert_eq!(
        decode_name(&find("protocol_version", pkt::SET_SERVER_NAME).payload).unwrap(),
        "OpenRGB 1.1"
    );
    let flags = decode_flags(&find("client_flags", pkt::SET_SERVER_FLAGS).payload).unwrap();
    assert_ne!(
        flags & server_flag::LOCAL_CLIENT,
        0,
        "loopback client requesting local status is granted it"
    );
    assert_ne!(flags & server_flag::SUPPORTS_PROFILEMANAGER, 0);
    let list = decode_controller_list(
        &find("controller_count", pkt::REQUEST_CONTROLLER_COUNT).payload,
        6,
    )
    .unwrap();
    assert_eq!(list.count, 8);
    assert_eq!(list.ids, (0..8).collect::<Vec<_>>());
}

#[test]
fn real_acks_decode_with_expected_status() {
    let caps = captures();
    let ack = |step: &str| {
        let (_, p) = caps
            .iter()
            .find(|(n, p)| n.contains(step) && p.pkt_id == pkt::ACK)
            .unwrap();
        (p.dev_id, decode_ack(&p.payload).unwrap())
    };
    assert_eq!(
        ack("ack_unknown_packet"),
        (0, (9999, status::ERROR_UNSUPPORTED))
    );
    // Invalid controller IDs are ACKed OK-status by this server build; record reality.
    assert_eq!(ack("ack_invalid_controller").0, 99999);
    assert_eq!(ack("resize_flagged_zone").1 .0, pkt::RESIZE_ZONE);
    assert_eq!(ack("update_leds").1 .0, pkt::UPDATE_LEDS);
}

#[test]
fn real_signal_update_carries_device_block_for_mode_change() {
    let caps = captures();
    let sigs: Vec<_> = caps
        .iter()
        .filter(|(n, p)| n.contains("set_custom_mode") && p.pkt_id == pkt::SIGNAL_UPDATE)
        .collect();
    assert_eq!(sigs.len(), 2);
    for (name, p) in sigs {
        let mut r = freergb_protocol::wire::Reader::new(&p.payload);
        let size = r.u32().unwrap() as usize;
        assert_eq!(size, p.payload.len(), "{name}");
        let reason = r.u32().unwrap();
        assert_ne!(reason, update_reason::UPDATE_LEDS);
        let c = ControllerData::decode_block(&mut r, 6).unwrap();
        assert_eq!(c.name, "Resizable Strip", "{name} reason {reason}");
        r.finish().unwrap();
    }
}

#[test]
fn real_settings_and_profile_payloads_are_nul_terminated_strings() {
    for (name, p) in captures() {
        if matches!(p.pkt_id, pkt::SETTINGS_GET | pkt::SETTINGS_GET_SCHEMA) {
            let s = freergb_protocol::wire::Reader::new(&p.payload)
                .rest_cstr()
                .unwrap();
            assert!(s == "null" || s.starts_with('{'), "{name}: {s:.40}");
        }
    }
}

#[test]
fn real_profile_list_and_active_profile_decode() {
    let caps = captures();
    let list = caps
        .iter()
        .find(|(n, p)| n.contains("profile_list") && p.pkt_id == pkt::PROFILE_GET_LIST)
        .unwrap();
    assert_eq!(
        decode_profile_list(&list.1.payload).unwrap(),
        Vec::<String>::new()
    );
    // Round trip against the bytes the real server produced for an empty list.
    assert_eq!(encode_profile_list(&[]).unwrap(), list.1.payload);
    let active = caps
        .iter()
        .find(|(_, p)| p.pkt_id == pkt::PROFILE_GET_ACTIVE)
        .unwrap();
    assert_eq!(decode_cstring(&active.1.payload).unwrap(), "");
}

#[test]
fn real_replies_echo_the_request_device_id() {
    // The SDK client matches replies on (dev_id, pkt_id); this is the evidence it is safe.
    let m: serde_json::Value =
        serde_json::from_slice(&fs::read(dir().join("manifest.json")).unwrap()).unwrap();
    let mut checked = 0;
    for e in m.as_array().unwrap() {
        if e["response_pkt_id"] == e["request_pkt_id"] {
            assert_eq!(e["response_dev_id"], e["request_dev_id"], "{}", e["step"]);
            checked += 1;
        }
    }
    assert!(checked > 20, "only {checked} reply/request pairs checked");
}
