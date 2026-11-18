// SPDX-License-Identifier: GPL-2.0-or-later
//! Opt-in: the engine against a REAL OpenRGB 1.1 server with debug devices. This is the check
//! the mock cannot give: real ACK pacing, real flow control, real mode switching.
//!
//!   FREERGB_REAL_SERVER=127.0.0.1:6743 cargo test -p freergb-effects --test real_server -- --ignored --test-threads=1

use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Scene, Targets};
use freergb_effects::{Engine, EngineConfig};
use freergb_sdk::{Client, ConnectOptions, RgbBackend};
use std::sync::Arc;
use std::time::{Duration, Instant};

async fn connect() -> Client {
    let addr = std::env::var("FREERGB_REAL_SERVER").expect("set FREERGB_REAL_SERVER=host:port");
    Client::connect(addr.parse().unwrap(), ConnectOptions::default())
        .await
        .unwrap()
}

fn rainbow() -> Scene {
    Scene {
        effect: EffectSpec {
            kind: EffectKind::Rainbow,
            colors: vec![],
            speed: 1.0,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: None,
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        },
        targets: Targets::All,
        canvas: Canvas::Shared,
        group: None,
    }
}

#[tokio::test]
#[ignore]
async fn real_rainbow_at_30_fps_is_acknowledged_and_the_modes_come_back() {
    let client = Arc::new(connect().await);

    // The debug devices have a single mode ("Direct"), so against them the mode restore below
    // is NOT exercised (restoring Direct to Direct proves nothing). Any device that has a
    // second mode is first put in it, so on real hardware, or a server with multi-mode
    // devices, the restore assertions are meaningful.
    for (id, d) in client.devices() {
        if d.modes.len() > 1 {
            client.update_mode(id, 1, &d.modes[1]).await.unwrap();
        }
    }
    let before: Vec<_> = client
        .devices()
        .into_iter()
        .map(|(id, d)| (id, d.active_mode))
        .collect();
    println!("modes before: {before:?}");
    assert!(!before.is_empty(), "the server has no devices");
    let restore_exercised = before.iter().any(|(_, m)| *m != 0);
    println!(
        "mode restore {}",
        if restore_exercised {
            "IS exercised (some device started outside Direct)"
        } else {
            "is NOT exercised: every device has only the Direct mode"
        }
    );

    let engine = Engine::start(client.clone(), EngineConfig::default());
    let started = Instant::now();
    // Apply and clear twice: the second run must capture the restored mode, not Direct.
    for round in 1..=2 {
        engine.set_scenes(vec![rainbow()]).unwrap();
        tokio::time::sleep(Duration::from_secs(if round == 1 { 5 } else { 1 })).await;
        assert!(engine.is_running(), "engine died against the real server");
        assert!(client.is_connected());

        if round == 1 {
            // A second client reads what the server actually holds.
            let reader = connect().await;
            for (id, d) in reader.devices() {
                let distinct: std::collections::HashSet<_> =
                    d.colors.iter().map(|c| (c.r, c.g, c.b)).collect();
                println!(
                    "{id} {:<22} {:>3} LEDs, {:>3} distinct colours, mode {}",
                    d.name,
                    d.colors.len(),
                    distinct.len(),
                    d.active_mode
                );
                assert_eq!(
                    d.active_mode, 0,
                    "{}: not in Direct while streaming",
                    d.name
                );
                if d.colors.len() > 8 {
                    assert!(distinct.len() > 3, "{}: not showing a rainbow", d.name);
                }
            }
            // The request/reply path must stay responsive while frames stream.
            let t = Instant::now();
            client.get_settings_json("General").await.unwrap();
            println!("settings round trip under load: {:?}", t.elapsed());
            let stats = engine.stats();
            println!("ran {:?}: {stats:?}", started.elapsed());
            assert!(
                stats.dropped * 100 <= stats.sent.max(1),
                "more than 1% of frames dropped: {stats:?}"
            );
        }

        engine.clear();
        tokio::time::sleep(Duration::from_millis(800)).await;
        let reader = connect().await;
        for (id, mode) in &before {
            let now = reader.device(*id).map(|d| d.active_mode);
            assert_eq!(
                now,
                Some(*mode),
                "round {round}: device {id} mode not restored"
            );
        }
    }
    engine.stop().await;
}
