// SPDX-License-Identifier: GPL-2.0-or-later
//! Opt-in tests against a REAL OpenRGB 1.1 server configured with the debug
//! devices described in crates/freergb-protocol/tests/fixtures/README.md.
//!
//!   FREERGB_REAL_SERVER=127.0.0.1:6743 cargo test -p freergb-sdk --test real_server -- --ignored --test-threads=1
//!
//! Skipped (ignored) in normal runs and CI.

use freergb_protocol::ids::*;
use freergb_sdk::*;
use std::time::{Duration, Instant};

async fn connect() -> Client {
    let addr = std::env::var("FREERGB_REAL_SERVER").expect("set FREERGB_REAL_SERVER=host:port");
    Client::connect(addr.parse().unwrap(), ConnectOptions::default())
        .await
        .unwrap()
}

fn id_of(c: &Client, name: &str) -> u32 {
    c.devices()
        .into_iter()
        .find(|(_, d)| d.name == name)
        .unwrap_or_else(|| panic!("no device {name}"))
        .0
}

#[tokio::test]
#[ignore]
async fn real_connect_lists_debug_devices_with_local_status() {
    let c = connect().await;
    assert!(c.is_local_client());
    assert!(
        c.server_name().starts_with("OpenRGB"),
        "{}",
        c.server_name()
    );
    let names: Vec<_> = c.devices().into_iter().map(|(_, d)| d.name).collect();
    for want in [
        "Debug Motherboard",
        "Debug Keyboard",
        "Debug ARGB",
        "Resizable Strip",
        "Big 300",
    ] {
        assert!(
            names.iter().any(|n| n == want),
            "{want} missing from {names:?}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn real_led_updates_and_error_statuses() {
    let c = connect().await;
    let big = id_of(&c, "Big 300");
    let cols: Vec<Color> = (0..300).map(|i| Color::new(i as u8, 1, 2)).collect();
    c.update_leds(big, &cols).await.unwrap();
    let mut seen = false;
    for _ in 0..100 {
        if c.device(big).unwrap().colors == cols {
            seen = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(seen, "SIGNAL_UPDATE colors never reached the local store");
    // Fewer colors than LEDs is a partial update and is accepted by the real server.
    c.update_leds(big, &[Color::new(1, 1, 1)]).await.unwrap();
    // More colors than LEDs is rejected with status 5.
    let err = c
        .update_leds(big, &vec![Color::new(1, 1, 1); 301])
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
#[ignore]
async fn real_flagged_resize_works_and_unflagged_is_refused() {
    let c = connect().await;
    let argb = id_of(&c, "Debug ARGB");
    let zone = c
        .device(argb)
        .unwrap()
        .zones
        .iter()
        .position(|z| z.flags & zone_flag::CONFIGURABLE_SIZE != 0)
        .unwrap();
    let before = c.device(argb).unwrap().leds.len();
    let old = c.device(argb).unwrap().zones[zone].leds_count;
    c.resize_zone(argb, zone as u32, 25).await.unwrap();
    let after = c.device(argb).unwrap();
    assert_eq!(after.zones[zone].leds_count, 25);
    assert_eq!(after.leds.len(), before - old as usize + 25);
    c.resize_zone(argb, zone as u32, old).await.unwrap();
    let strip = id_of(&c, "Resizable Strip");
    assert!(matches!(
        c.resize_zone(strip, 0, 90).await,
        Err(SdkError::ZoneNotResizable)
    ));
}

#[tokio::test]
#[ignore]
async fn real_modes_work() {
    let c = connect().await;
    let mb = id_of(&c, "Debug Motherboard");
    c.set_custom_mode(mb).await.unwrap();
    let d = c.device(mb).unwrap();
    let mut m = d.modes[d.active_mode as usize].clone();
    m.brightness = m.brightness_max;
    c.update_mode(mb, d.active_mode as u32, &m).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn real_profiles_save_load_delete_without_stalling() {
    let c = connect().await;
    let mut ev = c.events();
    let _ = c.delete_profile("freergb-it").await;
    c.save_profile("freergb-it").await.unwrap();
    assert!(c
        .list_profiles()
        .await
        .unwrap()
        .contains(&"freergb-it".to_string()));
    let t = Instant::now();
    c.load_profile("freergb-it").await.unwrap();
    // The real server stalls about 1 s if 159 is not echoed.
    assert!(
        t.elapsed() < Duration::from_millis(900),
        "load took {:?}",
        t.elapsed()
    );
    assert_eq!(c.active_profile().await.unwrap(), "freergb-it");
    let got = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(Event::ActiveProfileChanged(n)) = ev.recv().await {
                if n == "freergb-it" {
                    return;
                }
            }
        }
    })
    .await;
    assert!(got.is_ok(), "no ActiveProfileChanged event");
    c.delete_profile("freergb-it").await.unwrap();
    assert!(!c
        .list_profiles()
        .await
        .unwrap()
        .contains(&"freergb-it".to_string()));
}

#[tokio::test]
#[ignore]
async fn real_settings_schema_has_event_profile_keys() {
    let c = connect().await;
    let schema = c.get_settings_schema("").await.unwrap();
    for k in [
        "open_profile",
        "exit_profile",
        "suspend_profile",
        "resume_profile",
        "service_startup_profile",
    ] {
        assert!(schema.contains(k), "{k} missing");
    }
}

#[tokio::test]
#[ignore]
async fn real_streaming_frames_are_acked_and_land() {
    let c = Client::connect(
        std::env::var("FREERGB_REAL_SERVER")
            .unwrap()
            .parse()
            .unwrap(),
        ConnectOptions {
            max_unacked: 8,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let strip = id_of(&c, "Resizable Strip");
    let n = c.device(strip).unwrap().leds.len();
    for f in 0..120u32 {
        let v = (f % 250) as u8 + 1;
        c.update_leds_nowait(strip, &vec![Color::new(v, 0, 0); n])
            .await
            .unwrap();
    }
    // The last frame must arrive back through SIGNAL_UPDATE, proving the server processed the stream.
    let want = Color::new(120, 0, 0);
    let mut landed = false;
    for _ in 0..200 {
        if c.device(strip).unwrap().colors.iter().all(|&x| x == want) {
            landed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(landed, "last streamed frame never observed");
    // Every ACK came back, so all flow-control permits are free: a fresh burst of
    // max_unacked frames must not block.
    let t = Instant::now();
    for _ in 0..8 {
        c.update_leds_nowait(strip, &vec![Color::new(9, 9, 9); n])
            .await
            .unwrap();
    }
    assert!(
        t.elapsed() < Duration::from_secs(1),
        "permits not returned: {:?}",
        t.elapsed()
    );
}

/// Settings writes against a real server. A raw MODIFY whose members all fail the
/// schema filter (including `{}`) aborts OpenRGB 1.1, so the client must refuse those
/// and the server must stay up through the whole sequence.
#[tokio::test]
#[ignore]
async fn real_settings_write_merge_and_crash_guard() {
    use serde_json::json;
    let c = connect().await;
    let before = c.get_settings_json("ProfileManager").await.unwrap();
    let on = json!({"enabled": true, "name": "freergb-probe"});
    c.modify_settings("ProfileManager", &json!({"resume_profile": on}))
        .await
        .unwrap();
    let after = c.get_settings_json("ProfileManager").await.unwrap();
    assert_eq!(after["resume_profile"], on);
    for (k, v) in before.as_object().into_iter().flatten() {
        if k != "resume_profile" {
            assert_eq!(&after[k], v, "modify must not disturb {k}");
        }
    }
    for bad in [
        json!({"resume_profile": "oops"}),
        json!({}),
        json!({"nope": 1}),
    ] {
        assert!(matches!(
            c.modify_settings("ProfileManager", &bad).await,
            Err(SdkError::InvalidSettings(_))
        ));
    }
    // unchanged, and the server is still alive
    assert_eq!(
        c.get_settings_json("ProfileManager").await.unwrap()["resume_profile"],
        on
    );
    c.save_settings().await.unwrap();
    // restore the original (null means it was unset: put back the disabled default)
    let restore = if before.is_object() {
        before
    } else {
        json!({"resume_profile": {"enabled": false, "name": ""}})
    };
    c.set_settings("ProfileManager", &restore).await.unwrap();
    c.save_settings().await.unwrap();
}

/// Prints id -> identity for every device; run before and after a server restart and
/// diff to learn whether controller IDs are stable across server processes.
#[tokio::test]
#[ignore]
async fn real_print_device_identity() {
    let c = connect().await;
    let mut rows: Vec<_> = c.devices().into_iter().collect();
    rows.sort_by_key(|(id, _)| *id);
    for (id, d) in rows {
        println!("IDENT {id} | {} | {} | {}", d.name, d.location, d.serial);
    }
}
