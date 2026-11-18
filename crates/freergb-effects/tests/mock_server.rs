// SPDX-License-Identifier: GPL-2.0-or-later
//! The engine over a real TCP client against the mock server: the same path the app uses.

use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Scene, Targets};
use freergb_effects::{Engine, EngineConfig};
use freergb_mock::{devices, MockServer};
use freergb_sdk::{Client, ConnectOptions};
use std::sync::Arc;
use std::time::Duration;

fn rainbow() -> Scene {
    Scene {
        effect: EffectSpec {
            kind: EffectKind::Rainbow,
            colors: vec![],
            speed: 2.0,
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
async fn a_rainbow_reaches_the_server_and_the_old_mode_comes_back() {
    let mut strip = devices::big_300();
    strip.active_mode = 1;
    let server = MockServer::start(vec![strip]).await.unwrap();
    let client = Arc::new(
        Client::connect(server.addr(), ConnectOptions::default())
            .await
            .unwrap(),
    );
    let id = server.device_ids()[0];

    let engine = Engine::start(client.clone(), EngineConfig::default());
    engine.set_scenes(vec![rainbow()]).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;

    let first = server.device(id).unwrap();
    assert_eq!(first.active_mode, 0, "not switched to Direct");
    let distinct: std::collections::HashSet<_> =
        first.colors.iter().map(|c| (c.r, c.g, c.b)).collect();
    assert!(
        distinct.len() > 50,
        "a rainbow over 300 LEDs has many colours, got {}",
        distinct.len()
    );

    tokio::time::sleep(Duration::from_millis(300)).await;
    let later = server.device(id).unwrap();
    assert_ne!(first.colors, later.colors, "the effect is not animating");

    engine.stop().await;
    assert_eq!(
        server.device(id).unwrap().active_mode,
        1,
        "mode not restored"
    );
}

#[tokio::test]
async fn the_engine_ends_when_the_connection_drops_and_a_new_one_resumes() {
    let server = MockServer::start(vec![devices::big_300()]).await.unwrap();
    let client = Arc::new(
        Client::connect(server.addr(), ConnectOptions::default())
            .await
            .unwrap(),
    );
    let engine = Engine::start(client.clone(), EngineConfig::default());
    engine.set_scenes(vec![rainbow()]).unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(engine.is_running());

    server.crash();
    for _ in 0..50 {
        if !engine.is_running() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !engine.is_running(),
        "engine kept running against a dead connection"
    );

    server.restart().await.unwrap();
    let client2 = Arc::new(
        Client::connect(server.addr(), ConnectOptions::default())
            .await
            .unwrap(),
    );
    let id = server.device_ids()[0];
    let before = server.device(id).unwrap().colors;
    let engine2 = Engine::start(client2, EngineConfig::default());
    engine2.set_scenes(vec![rainbow()]).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_ne!(server.device(id).unwrap().colors, before);
    engine2.stop().await;
}

#[tokio::test]
async fn each_apply_and_clear_cycle_restores_the_users_mode_through_a_real_client() {
    // Run twice: the second run captures its "original" from the client's cached state, which
    // is only right if the client refreshed the device after the first restore.
    let mut strip = devices::big_300();
    strip.active_mode = 1;
    let server = MockServer::start(vec![strip]).await.unwrap();
    let client = Arc::new(
        Client::connect(server.addr(), ConnectOptions::default())
            .await
            .unwrap(),
    );
    let id = server.device_ids()[0];
    let engine = Engine::start(client.clone(), EngineConfig::default());
    for round in 1..=3 {
        engine.set_scenes(vec![rainbow()]).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            server.device(id).unwrap().active_mode,
            0,
            "round {round}: not in Direct"
        );
        engine.clear();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            server.device(id).unwrap().active_mode,
            1,
            "round {round}: not restored"
        );
        assert_eq!(
            client.device(id).unwrap().active_mode,
            1,
            "round {round}: the client's cached mode is stale"
        );
    }
    engine.stop().await;
}

#[tokio::test]
async fn a_device_mode_reaches_the_server_with_the_mapped_values_and_the_old_mode_comes_back() {
    let mut ram = devices::with_rich_modes(devices::ram_stick(0));
    ram.active_mode = 1; // Static
    let server = MockServer::start(vec![ram]).await.unwrap();
    let client = Arc::new(
        Client::connect(server.addr(), ConnectOptions::default())
            .await
            .unwrap(),
    );
    let id = server.device_ids()[0];
    let key = client.device_keys()[0].1.clone();

    let engine = Engine::start(client.clone(), EngineConfig::default());
    let mut scene = Scene {
        effect: EffectSpec {
            kind: EffectKind::DeviceMode,
            colors: vec![
                freergb_effects::Rgb { r: 1, g: 2, b: 3 },
                freergb_effects::Rgb { r: 4, g: 5, b: 6 },
            ],
            speed: 0.0, // slowest: the inverted range's `min`, 100
            direction: Default::default(),
            brightness: 0.5,
            density: 1.0,
            mode: Some("Breathing".into()),
            mode_direction: Some(freergb_effects::ModeDirection::Down),
            angle_deg: None,
            zones: vec![],
            react: None,
        },
        targets: Targets::Devices(vec![key]),
        canvas: Canvas::Shared,
        group: None,
    };
    engine.set_scenes(vec![scene.clone()]).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;

    let d = server.device(id).unwrap();
    assert_eq!(d.active_mode, 3, "the built-in mode is active");
    let m = &d.modes[3];
    assert_eq!((m.speed, m.brightness), (100, 50));
    assert_eq!(m.direction, freergb_sdk::ids::direction::DOWN);
    assert_eq!(
        m.colors.iter().map(|c| (c.r, c.g, c.b)).collect::<Vec<_>>(),
        [(1, 2, 3), (4, 5, 6)]
    );

    scene.effect.speed = 1.0; // fastest: `max`, 10
    engine.set_scenes(vec![scene]).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(server.device(id).unwrap().modes[3].speed, 10);

    engine.stop().await;
    assert_eq!(
        server.device(id).unwrap().active_mode,
        1,
        "mode not restored"
    );
}
