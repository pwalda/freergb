// SPDX-License-Identifier: GPL-2.0-or-later

use freergb_mock::{devices, MockServer};
use freergb_protocol::ids::*;
use freergb_protocol::messages::*;
use freergb_protocol::Packet;
use freergb_sdk::*;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;

async fn setup() -> (MockServer, Client) {
    let server = MockServer::start(devices::default_set()).await.unwrap();
    let client = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .unwrap();
    (server, client)
}

async fn next_event(rx: &mut Receiver<Event>, want: impl Fn(&Event) -> bool) -> Event {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let e = rx.recv().await.expect("event stream closed");
            if want(&e) {
                return e;
            }
        }
    })
    .await
    .expect("timed out waiting for event")
}

async fn wait_for(mut cond: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !cond() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("condition not reached");
}

fn id_of(c: &Client, name: &str) -> u32 {
    c.devices()
        .into_iter()
        .find(|(_, d)| d.name == name)
        .unwrap_or_else(|| panic!("no {name}"))
        .0
}

#[tokio::test]
async fn connects_lists_devices_and_gets_local_status() {
    let (_s, c) = setup().await;
    assert!(c.is_local_client());
    assert_eq!(c.server_name(), "OpenRGB 1.1 (mock)");
    let names: Vec<_> = c.devices().into_iter().map(|(_, d)| d.name).collect();
    assert_eq!(names.len(), 6);
    assert!(names.contains(&"Mock Motherboard".to_string()));
    let ids: Vec<_> = c.devices().into_iter().map(|(i, _)| i).collect();
    assert_eq!(ids, vec![0, 1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn large_300_led_device_updates_roundtrip() {
    let (s, c) = setup().await;
    let big = id_of(&c, "Mock Big 300");
    assert_eq!(c.device(big).unwrap().leds.len(), 300);
    let cols: Vec<Color> = (0..300)
        .map(|i| Color::new((i % 256) as u8, 7, 9))
        .collect();
    c.update_leds(big, &cols).await.unwrap();
    assert_eq!(s.device(big).unwrap().colors, cols);
    // The SIGNAL_UPDATE broadcast keeps our local store in sync.
    wait_for(|| c.device(big).unwrap().colors == cols).await;
}

#[tokio::test]
async fn led_count_rule_matches_real_server_partial_ok_excess_rejected() {
    let (s, c) = setup().await;
    let big = id_of(&c, "Mock Big 300");
    // Fewer colors than LEDs: accepted, updates from LED 0 (real server behaviour).
    c.update_leds(big, &[Color::new(1, 2, 3), Color::new(4, 5, 6)])
        .await
        .unwrap();
    let got = s.device(big).unwrap().colors;
    assert_eq!(
        (got[0], got[1], got[2]),
        (Color::new(1, 2, 3), Color::new(4, 5, 6), Color::default())
    );
    // More colors than LEDs: invalid data (status 5).
    let err = c
        .update_leds(big, &vec![Color::default(); 301])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            SdkError::Rejected {
                pkt_id: 1050,
                status: 5
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn resize_respects_zone_flags_like_the_real_server() {
    let (_s, c) = setup().await;
    let strip = id_of(&c, "Mock Strip");
    c.resize_zone(strip, 0, 90).await.unwrap();
    assert_eq!(c.device(strip).unwrap().zones[0].leds_count, 90);
    assert_eq!(c.device(strip).unwrap().leds.len(), 90);
    assert!(matches!(
        c.resize_zone(strip, 0, 999).await,
        Err(SdkError::SizeOutOfRange { .. })
    ));
    // Unflagged zone: we refuse client-side because the real server would silently ignore it.
    let big = id_of(&c, "Mock Big 300");
    assert!(matches!(
        c.resize_zone(big, 0, 100).await,
        Err(SdkError::ZoneNotResizable)
    ));
    assert!(matches!(
        c.resize_zone(strip, 7, 10).await,
        Err(SdkError::UnknownZone { .. })
    ));
}

#[tokio::test]
async fn mode_changes_apply_and_update_store() {
    let (s, c) = setup().await;
    let kb = id_of(&c, "Mock Keyboard");
    c.set_custom_mode(kb).await.unwrap();
    assert_eq!(c.device(kb).unwrap().active_mode, 0);
    let mut m = c.device(kb).unwrap().modes[2].clone();
    m.speed = 77;
    c.update_mode(kb, 2, &m).await.unwrap();
    assert_eq!(c.device(kb).unwrap().active_mode, 2);
    assert_eq!(s.device(kb).unwrap().modes[2].speed, 77);
}

#[tokio::test]
async fn a_zone_takes_its_own_mode_and_goes_back_to_the_devices() {
    let (s, c) = setup().await;
    let hub = s.add_device(devices::fan_hub());
    let e = &mut c.events();
    next_event(e, |e| matches!(e, Event::DeviceListChanged { .. })).await;
    let mut pulse = c.device(hub).unwrap().zones[2].modes[1].clone();
    pulse.speed = 30;
    c.update_zone_mode(hub, 2, 1, Some(&pulse)).await.unwrap();
    let z = &c.device(hub).unwrap().zones[2];
    assert_eq!((z.active_mode, z.modes[1].speed), (1, 30));
    c.update_zone_mode(hub, 2, -1, None).await.unwrap();
    assert_eq!(s.device(hub).unwrap().zones[2].active_mode, -1);
    // Refused before it reaches the server: no such zone mode, no such zone.
    assert!(matches!(
        c.update_zone_mode(hub, 2, 3, Some(&pulse)).await,
        Err(SdkError::UnknownZoneMode { .. })
    ));
    assert!(matches!(
        c.update_zone_mode(hub, 9, -1, None).await,
        Err(SdkError::UnknownZone { .. })
    ));
}

#[tokio::test]
async fn a_strip_is_split_into_segments_and_back() {
    let (s, c) = setup().await;
    let strip = id_of(&c, "Mock Big 300");
    let seg = |name: &str, start: u32, n: u32| Segment {
        name: name.into(),
        segment_type: zone_type::LINEAR,
        start_idx: start,
        leds_count: n,
        ..Default::default()
    };
    let runs = [seg("Desk", 0, 100), seg("Shelf", 100, 50)];
    c.set_segments(strip, 0, &runs).await.unwrap();
    assert_eq!(c.device(strip).unwrap().zones[0].segments, runs);
    assert_eq!(s.device(strip).unwrap().zones[0].segments, runs);
    c.set_segments(strip, 0, &[]).await.unwrap();
    assert!(c.device(strip).unwrap().zones[0].segments.is_empty());
    // Refused before it reaches the server: overlapping, past the end, empty, unnamed.
    for bad in [
        vec![seg("A", 0, 10), seg("B", 5, 10)],
        vec![seg("A", 290, 20)],
        vec![seg("A", 0, 0)],
        vec![seg(" ", 0, 4)],
    ] {
        assert!(matches!(
            c.set_segments(strip, 0, &bad).await,
            Err(SdkError::BadSegments)
        ));
    }
}

#[tokio::test]
async fn hotplug_add_and_remove_keep_stable_ids() {
    let (s, c) = setup().await;
    let mut ev = c.events();
    let before: Vec<u32> = c.devices().iter().map(|(i, _)| *i).collect();
    let new_id = s.add_device(devices::ram_stick(2));
    let e = next_event(&mut ev, |e| matches!(e, Event::DeviceListChanged { .. })).await;
    assert_eq!(
        e,
        Event::DeviceListChanged {
            added: vec![new_id],
            removed: vec![]
        }
    );
    assert!(c.device(new_id).is_some());
    // Existing devices keep their IDs (never list indexes).
    assert!(before.iter().all(|i| c.device(*i).is_some()));

    let victim = before[1];
    assert!(s.remove_device(victim));
    let e = next_event(&mut ev, |e| matches!(e, Event::DeviceListChanged { .. })).await;
    assert_eq!(
        e,
        Event::DeviceListChanged {
            added: vec![],
            removed: vec![victim]
        }
    );
    assert!(c.device(victim).is_none());
    // Remaining devices still address correctly after the removal shifted list positions.
    let strip = id_of(&c, "Mock Strip");
    c.update_leds(
        strip,
        &vec![Color::new(9, 9, 9); c.device(strip).unwrap().leds.len()],
    )
    .await
    .unwrap();
    // IDs are never reused.
    assert!(s.add_device(devices::flaky()) > new_id);
}

#[tokio::test]
async fn device_disappearing_mid_session_gives_clear_errors() {
    let server = MockServer::start({
        let mut d = devices::default_set();
        d.push(devices::flaky());
        d
    })
    .await
    .unwrap();
    let c = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .unwrap();
    let flaky = id_of(&c, "Mock Flaky USB");
    server.remove_device(flaky);
    // Race: before our resync lands the server says "invalid id"; afterwards we say "unknown device".
    let err = c
        .update_single_led(flaky, 0, Color::new(1, 1, 1))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            SdkError::Rejected { status: 4, .. } | SdkError::UnknownDevice(_)
        ),
        "{err:?}"
    );
    wait_for(|| c.device(flaky).is_none()).await;
    assert!(matches!(
        c.update_single_led(flaky, 0, Color::new(1, 1, 1)).await,
        Err(SdkError::UnknownDevice(_))
    ));
}

#[tokio::test]
async fn protocol_mismatch_is_refused_with_a_clear_message() {
    let server = MockServer::start(devices::default_set()).await.unwrap();
    server.set_max_protocol(5);
    let err = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .err()
        .unwrap();
    assert!(
        matches!(err, SdkError::UnsupportedProtocol { server: 5 }),
        "{err:?}"
    );
    assert!(err.to_string().contains("OpenRGB 1.0"));
}

#[tokio::test]
async fn crash_disconnects_then_restart_allows_reconnect() {
    let (s, c) = setup().await;
    let mut ev = c.events();
    s.crash();
    next_event(&mut ev, |e| *e == Event::Disconnected).await;
    assert!(!c.is_connected());
    assert!(matches!(c.rescan().await, Err(SdkError::Disconnected)));
    assert!(Client::connect(
        s.addr(),
        ConnectOptions {
            request_timeout: Duration::from_millis(300),
            ..Default::default()
        }
    )
    .await
    .is_err());
    s.restart().await.unwrap();
    let c2 = Client::connect(s.addr(), ConnectOptions::default())
        .await
        .unwrap();
    assert_eq!(c2.devices().len(), 6);
}

#[tokio::test]
async fn in_flight_request_fails_cleanly_when_server_crashes() {
    let (s, c) = setup().await;
    s.set_latency(Duration::from_millis(400));
    let c2 = c.clone();
    let strip = id_of(&c, "Mock Strip");
    let pending = tokio::spawn(async move { c2.rescan().await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    s.crash();
    let r = pending.await.unwrap();
    assert!(matches!(r, Err(SdkError::Disconnected)), "{r:?}");
    let _ = strip;
}

#[tokio::test]
async fn slow_server_times_out_instead_of_hanging() {
    let (s, _c) = setup().await;
    let c = Client::connect(
        s.addr(),
        ConnectOptions {
            request_timeout: Duration::from_millis(150),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    s.set_latency(Duration::from_millis(600));
    let t = Instant::now();
    let r = c.rescan().await;
    assert!(matches!(r, Err(SdkError::Timeout(_))), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn malformed_stream_drops_the_connection_without_panicking() {
    let (s, c) = setup().await;
    let mut ev = c.events();
    s.inject_raw(b"this is definitely not an ORGB packet....");
    next_event(&mut ev, |e| *e == Event::Disconnected).await;
    assert!(matches!(c.rescan().await, Err(SdkError::Disconnected)));
}

#[tokio::test]
async fn oversized_packet_claim_drops_the_connection() {
    let (s, c) = setup().await;
    let mut ev = c.events();
    let mut bogus = b"ORGB".to_vec();
    bogus.extend_from_slice(&0u32.to_le_bytes());
    bogus.extend_from_slice(&pkt::REQUEST_CONTROLLER_DATA.to_le_bytes());
    bogus.extend_from_slice(&u32::MAX.to_le_bytes());
    s.inject_raw(&bogus);
    next_event(&mut ev, |e| *e == Event::Disconnected).await;
}

#[tokio::test]
async fn fragmented_delivery_is_reassembled() {
    let server = MockServer::start(devices::default_set()).await.unwrap();
    server.set_fragmenting(true);
    let c = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .unwrap();
    assert_eq!(c.devices().len(), 6);
    let big = id_of(&c, "Mock Big 300");
    c.update_leds(big, &vec![Color::new(5, 5, 5); 300])
        .await
        .unwrap();
}

#[tokio::test]
async fn remote_client_cannot_save_profiles_but_can_control_devices() {
    let server = MockServer::start(devices::default_set()).await.unwrap();
    server.set_remote_mode(true);
    let c = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .unwrap();
    assert!(!c.is_local_client());
    assert!(matches!(
        c.save_profile("x").await,
        Err(SdkError::NotLocalClient)
    ));
    assert!(matches!(
        c.delete_profile("x").await,
        Err(SdkError::NotLocalClient)
    ));
    let strip = id_of(&c, "Mock Strip");
    c.update_leds(strip, &[Color::new(1, 2, 3); 60])
        .await
        .unwrap();
    assert!(c.list_profiles().await.unwrap().is_empty());
}

#[tokio::test]
async fn profile_save_load_delete_roundtrip_without_stalling() {
    let (s, c) = setup().await;
    let mut ev = c.events();
    let strip = id_of(&c, "Mock Strip");
    c.update_leds(strip, &[Color::new(10, 20, 30); 60])
        .await
        .unwrap();
    c.save_profile("night").await.unwrap();
    next_event(
        &mut ev,
        |e| matches!(e, Event::ProfileListUpdated(n) if n == &["night".to_string()]),
    )
    .await;
    assert_eq!(c.list_profiles().await.unwrap(), vec!["night"]);

    c.update_leds(strip, &[Color::new(200, 0, 0); 60])
        .await
        .unwrap();
    let t = Instant::now();
    c.load_profile("night").await.unwrap();
    // We echo PROFILE_ABOUT_TO_LOAD; the server must not have waited out its 1 s timeout.
    assert!(
        t.elapsed() < Duration::from_millis(700),
        "load took {:?}",
        t.elapsed()
    );
    assert_eq!(s.device(strip).unwrap().colors[0], Color::new(10, 20, 30));
    assert_eq!(c.active_profile().await.unwrap(), "night");
    next_event(
        &mut ev,
        |e| matches!(e, Event::ActiveProfileChanged(n) if n == "night"),
    )
    .await;

    c.delete_profile("night").await.unwrap();
    assert!(c.list_profiles().await.unwrap().is_empty());
    assert!(s.profile_names().is_empty());
}

#[tokio::test]
async fn streaming_path_is_flow_controlled_and_delivers_every_frame() {
    let server = MockServer::start(devices::default_set()).await.unwrap();
    let c = Client::connect(
        server.addr(),
        ConnectOptions {
            max_unacked: 8,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    server.set_latency(Duration::from_millis(2));
    let strip = id_of(&c, "Mock Strip");
    let n = c.device(strip).unwrap().leds.len();
    for frame in 0..300u32 {
        let v = (frame % 250) as u8;
        c.update_leds_nowait(strip, &vec![Color::new(v, v, v); n])
            .await
            .unwrap();
    }
    wait_for(|| server.device(strip).unwrap().colors[0] == Color::new(49, 49, 49)).await;
    let updates = server
        .received()
        .iter()
        .filter(|p| p.pkt_id == pkt::UPDATE_LEDS)
        .count();
    assert_eq!(updates, 300);
}

#[tokio::test]
async fn settings_and_schema_are_readable() {
    let (_s, c) = setup().await;
    assert_eq!(c.get_settings("General").await.unwrap(), "null");
    let schema = c.get_settings_schema("").await.unwrap();
    assert!(schema.contains("resume_profile") && schema.contains("suspend_profile"));
}

#[tokio::test]
async fn rescan_emits_detection_events() {
    let (_s, c) = setup().await;
    let mut ev = c.events();
    c.rescan().await.unwrap();
    next_event(&mut ev, |e| *e == Event::DetectionStarted).await;
    next_event(&mut ev, |e| {
        *e == Event::DetectionProgress {
            percent: 50,
            label: "Mock devices".into(),
        }
    })
    .await;
    next_event(&mut ev, |e| *e == Event::DetectionComplete).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_refreshes_never_cross_devices() {
    // Regression guard: replies are matched on (dev_id, pkt_id) and requests are queued in
    // registration order, so concurrent per-device refreshes cannot swap controller blocks.
    let (_s, c) = setup().await;
    let want: Vec<(u32, String)> = c
        .devices()
        .into_iter()
        .map(|(i, d)| (i, d.serial))
        .collect();
    for _ in 0..30 {
        let mut tasks = Vec::new();
        for (id, _) in &want {
            let c = c.clone();
            let id = *id;
            tasks.push(tokio::spawn(async move { c.set_custom_mode(id).await }));
        }
        for t in tasks {
            t.await.unwrap().unwrap();
        }
        let got: Vec<(u32, String)> = c
            .devices()
            .into_iter()
            .map(|(i, d)| (i, d.serial))
            .collect();
        assert_eq!(got, want, "a device block landed in another device's slot");
    }
}

#[tokio::test]
async fn load_profile_does_not_resend_modes_by_default() {
    let (s, c) = setup().await;
    c.save_profile("p").await.unwrap();
    let count = |s: &MockServer| {
        s.received()
            .iter()
            .filter(|p| p.pkt_id == pkt::UPDATE_MODE)
            .count()
    };
    let before = count(&s);
    c.load_profile("p").await.unwrap();
    assert_eq!(
        before,
        count(&s),
        "server applies profiles itself; the client must not double-write"
    );
}

// ---- settings writes ---------------------------------------------------------

fn modify_count(s: &MockServer) -> usize {
    s.received()
        .iter()
        .filter(|p| p.pkt_id == pkt::SETTINGS_MODIFY || p.pkt_id == pkt::SETTINGS_SET)
        .count()
}

/// Control for the guard test: prove the mock really aborts like OpenRGB 1.1 when a MODIFY
/// has no member that passes the schema filter, so "client never sent it" means something.
#[tokio::test]
async fn mock_aborts_on_modify_that_filters_to_null() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let server = MockServer::start(devices::default_set()).await.unwrap();
    let mut sock = tokio::net::TcpStream::connect(server.addr()).await.unwrap();
    for p in [
        Packet::new(0, pkt::REQUEST_PROTOCOL_VERSION, encode_protocol_version(6)),
        Packet::new(0, pkt::SET_CLIENT_NAME, encode_name("raw")),
        Packet::new(
            0,
            pkt::SET_CLIENT_FLAGS,
            encode_flags(client_flag::SUPPORTS_SETTINGSMANAGER | client_flag::REQUEST_LOCAL_CLIENT),
        ),
        Packet::new(
            0,
            pkt::SETTINGS_MODIFY,
            encode_cstring(r#"{"ProfileManager":{}}"#),
        ),
    ] {
        sock.write_all(&p.encode()).await.unwrap();
    }
    let mut sink = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), sock.read_to_end(&mut sink))
        .await
        .expect("mock did not drop the connection")
        .ok();
    assert_eq!(server.settings_aborts(), 1);
}

#[tokio::test]
async fn settings_write_merges_and_refuses_what_would_crash_the_server() {
    use serde_json::json;
    let (server, c) = setup().await;
    let on = json!({"enabled": true, "name": "Night"});
    c.modify_settings("ProfileManager", &json!({"resume_profile": on}))
        .await
        .unwrap();
    c.modify_settings(
        "ProfileManager",
        &json!({"suspend_profile": {"enabled": false, "name": ""}}),
    )
    .await
    .unwrap();
    let got = c.get_settings_json("ProfileManager").await.unwrap();
    assert_eq!(got["resume_profile"], on, "merge kept the first write");
    assert_eq!(got["suspend_profile"]["enabled"], false);
    let sent = modify_count(&server);

    for bad in [
        json!({}),
        json!({"resume_profile": "Night"}),
        json!({"unknown": 1}),
        json!("x"),
    ] {
        assert!(
            matches!(
                c.modify_settings("ProfileManager", &bad).await,
                Err(SdkError::InvalidSettings(_))
            ),
            "{bad}"
        );
        assert!(c.set_settings("ProfileManager", &bad).await.is_err());
    }
    assert_eq!(
        modify_count(&server),
        sent,
        "refused writes never reach the wire"
    );
    assert_eq!(server.settings_aborts(), 0);
    c.save_settings().await.unwrap();
    assert!(!c.devices().is_empty(), "still connected and synced");
}

#[tokio::test]
async fn settings_writes_need_local_status() {
    let server = MockServer::start(devices::default_set()).await.unwrap();
    server.set_remote_mode(true);
    let c = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .unwrap();
    let v = serde_json::json!({"resume_profile": {"enabled": true, "name": "x"}});
    assert!(matches!(
        c.modify_settings("ProfileManager", &v).await,
        Err(SdkError::NotLocalClient)
    ));
    assert!(matches!(
        c.save_settings().await,
        Err(SdkError::NotLocalClient)
    ));
    assert_eq!(modify_count(&server), 0);
    assert!(
        c.get_settings_json("ProfileManager").await.is_ok(),
        "reads are allowed"
    );
}

#[tokio::test]
async fn the_machine_lists_come_back_for_a_local_client_only() {
    let (_server, c) = setup().await;
    assert_eq!(c.i2c_buses().await.unwrap()[0].pci_vendor, 0x8086);
    assert_eq!(c.hid_devices().await.unwrap().len(), 2);
    assert_eq!(
        c.usb_devices().await.unwrap()[0].product,
        "iCUE H150i ELITE"
    );
    assert_eq!(c.serial_ports().await.unwrap(), ["/dev/ttyUSB0"]);
    assert_eq!(c.usb_serial_ports().await.unwrap()[0].usb_path, "1-4");

    let remote = MockServer::start(devices::default_set()).await.unwrap();
    remote.set_remote_mode(true);
    let r = Client::connect(remote.addr(), ConnectOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        r.hid_devices().await,
        Err(SdkError::NotLocalClient)
    ));
    assert!(matches!(
        r.request_log().await,
        Err(SdkError::NotLocalClient)
    ));
}

#[tokio::test]
async fn the_log_is_empty_until_the_setting_is_on_at_startup() {
    let (server, c) = setup().await;
    c.request_log().await.unwrap();
    server.log(1, "not kept");
    c.rescan().await.unwrap();
    assert!(c.log_since(0).is_empty(), "no log while the setting is off");

    server.enable_log();
    server.log(2, "fan curve missing");
    // A client that connects later sees the earlier lines through the buffer, once each.
    let c = Client::connect(server.addr(), ConnectOptions::default())
        .await
        .unwrap();
    let mut rx = c.events();
    c.request_log().await.unwrap();
    c.request_log().await.unwrap();
    let texts = |after| -> Vec<(u64, String)> {
        c.log_since(after)
            .into_iter()
            .map(|(n, l)| (n, l.text))
            .collect()
    };
    assert_eq!(
        texts(0),
        [
            (3, "[NetworkServer] Network server started".to_string()),
            (4, "fan curve missing".into())
        ]
    );
    // New lines are pushed live and numbered on.
    server.log(1, "live");
    next_event(&mut rx, |e| matches!(e, Event::LogUpdated)).await;
    wait_for(|| c.log_since(4).len() == 1).await;
    assert_eq!(texts(4), [(5, "live".to_string())]);
    c.clear_log().await.unwrap();
    c.request_log().await.unwrap();
    assert!(c.log_since(0).is_empty(), "cleared");
}
