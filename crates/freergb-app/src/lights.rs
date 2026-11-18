// SPDX-License-Identifier: GPL-2.0-or-later
//! What the user wants the lights to do, and the engine that does it. Independent of Tauri so
//! it can be tested against the mock server.
//!
//! The engine is bound to one connection. When the supervisor publishes a new client (after an
//! OpenRGB restart) [`Lights::attach`] starts a fresh engine and re-applies the remembered
//! scenes, so effects survive a server restart without the UI doing anything.

use freergb_effects::layout::Placed;
use freergb_effects::react::Readings;
use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Rgb, Scene, Targets};
use freergb_effects::{hwmode, Engine, EngineConfig};
use freergb_sdk::ids::{mode_flag, zone_type};
use freergb_sdk::{keys_of, Client, DeviceKey, RgbBackend, Segment};
use freergb_sources::{demand, Hub};
use serde::Deserialize;
use std::sync::{Arc, Mutex, MutexGuard};

const MAX_SEGMENTS: usize = 32;
const MAX_SEGMENT_NAME: usize = 40;

/// One run of a split zone, as the Arrange view sends it.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct SegmentRun {
    pub name: String,
    pub leds: u32,
}

#[derive(Default)]
struct Inner {
    engine: Option<Engine>,
    client: Option<Client>,
    /// What was last asked for; kept while disconnected.
    scenes: Vec<Scene>,
    /// Frames per second; 0 until set, meaning the engine default.
    fps: u32,
    map: Vec<Placed>,
    /// Devices no effect may drive.
    hidden: Vec<DeviceKey>,
    /// Where looks that follow a reading get it; `None` in tests that need none.
    hub: Option<Arc<Hub>>,
}

impl Inner {
    /// Run the sources the playing looks follow, and only while there is an engine to play
    /// them.
    fn follow(&self) {
        if let Some(h) = &self.hub {
            let want = match &self.engine {
                Some(_) => demand(self.scenes.iter().map(|s| &s.effect)),
                None => Default::default(),
            };
            h.want(&want);
        }
    }
}

#[derive(Default)]
pub struct Lights(Mutex<Inner>);

impl Lights {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Bind to a new connection (or none). The previous engine is dropped, which aborts its
    /// loop; its connection is gone, so there is nothing to restore.
    pub fn attach(&self, client: Option<&Client>) {
        let mut g = self.inner();
        let mut cfg = EngineConfig::default();
        if g.fps > 0 {
            cfg.fps = g.fps;
        }
        g.engine = client.map(|c| {
            let e = Engine::start(Arc::new(c.clone()), cfg);
            e.set_map(g.map.clone());
            e.set_hidden(g.hidden.clone());
            if !g.scenes.is_empty() {
                let _ = e.set_scenes(g.scenes.clone());
            }
            if let Some(h) = &g.hub {
                e.set_readings(h.readings().borrow().clone());
            }
            e
        });
        g.client = client.cloned();
        g.follow();
    }

    /// Use `hub` for the readings looks follow, now and for every later connection.
    pub fn set_hub(&self, hub: Arc<Hub>) {
        let mut g = self.inner();
        g.hub = Some(hub);
        g.follow();
    }

    /// Hand the latest readings to the engine.
    pub fn set_readings(&self, r: Readings) {
        if let Some(e) = &self.inner().engine {
            e.set_readings(r);
        }
    }

    /// Change the frame rate now and for every later connection.
    pub fn set_fps(&self, fps: u32) {
        let mut g = self.inner();
        g.fps = fps;
        if let Some(e) = &g.engine {
            e.set_fps(fps);
        }
    }

    /// Change the desk map now and for every later connection.
    pub fn set_map(&self, map: Vec<Placed>) {
        let mut g = self.inner();
        if let Some(e) = &g.engine {
            e.set_map(map.clone());
        }
        g.map = map;
    }

    /// Change the devices left alone, now and for every later connection.
    pub fn set_hidden(&self, hidden: Vec<DeviceKey>) {
        let mut g = self.inner();
        if let Some(e) = &g.engine {
            e.set_hidden(hidden.clone());
        }
        g.hidden = hidden;
    }

    /// The connection, if any.
    pub fn client(&self) -> Option<Client> {
        self.inner().client.clone()
    }

    /// Tell the engine the device list changed (hotplug, resize).
    pub fn devices_changed(&self) {
        if let Some(e) = &self.inner().engine {
            e.devices_changed();
        }
    }

    /// Install these scenes; an empty list stops all effects. Invalid scenes change nothing.
    /// Remembered even while disconnected only when empty (stopping needs no server).
    pub fn set_scenes(&self, scenes: Vec<Scene>) -> Result<(), String> {
        for s in &scenes {
            s.validate().map_err(|e| e.to_string())?;
        }
        let mut g = self.inner();
        match &g.engine {
            Some(e) => e.set_scenes(scenes.clone()).map_err(|e| e.to_string())?,
            None if scenes.is_empty() => {}
            None => return Err("OpenRGB is not connected".into()),
        }
        g.scenes = scenes;
        g.follow();
        Ok(())
    }

    /// Change what is running (or remembered while disconnected) by `f`, e.g. after a group
    /// changed. Unlike `set_scenes` this needs no connection: the result is applied on connect.
    pub fn rewrite(&self, f: impl FnOnce(&[Scene]) -> Vec<Scene>) -> Result<(), String> {
        let mut g = self.inner();
        let next = f(&g.scenes);
        for s in &next {
            s.validate().map_err(|e| e.to_string())?;
        }
        if let Some(e) = &g.engine {
            e.set_scenes(next.clone()).map_err(|e| e.to_string())?;
        }
        g.scenes = next;
        g.follow();
        Ok(())
    }

    /// Remember scenes to apply once connected (the look restored at startup). Ignored when
    /// any scene is invalid, or when something has already been asked for.
    pub fn remember(&self, scenes: Vec<Scene>) {
        if scenes.iter().any(|s| s.validate().is_err()) {
            return;
        }
        let mut g = self.inner();
        if g.scenes.is_empty() {
            if let Some(e) = &g.engine {
                if e.set_scenes(scenes.clone()).is_err() {
                    return;
                }
            }
            g.scenes = scenes;
        }
    }

    pub fn scenes(&self) -> Vec<Scene> {
        self.inner().scenes.clone()
    }

    /// Paint one device a single colour. This replaces all running effects with a static scene
    /// on that device only, so the engine does the mode switch and restores the other devices
    /// (doing it separately would race with the engine's own restore).
    pub fn paint(&self, device: u32, r: u8, g: u8, b: u8) -> Result<(), String> {
        let client = self
            .inner()
            .client
            .clone()
            .ok_or("OpenRGB is not connected")?;
        let devices = client.devices();
        let key = keys_of(devices.iter().map(|(id, d)| (*id, d)))
            .into_iter()
            .find(|(id, _)| *id == device)
            .map(|(_, k)| k)
            .ok_or_else(|| format!("no such device ({device})"))?;
        self.set_scenes(vec![Scene {
            effect: EffectSpec {
                kind: EffectKind::Static,
                colors: vec![Rgb { r, g, b }],
                speed: 0.0,
                direction: Default::default(),
                brightness: 1.0,
                density: 1.0,
                mode: None,
                mode_direction: None,
                angle_deg: None,
                zones: vec![],
                react: None,
            },
            targets: Targets::Devices(vec![key]),
            canvas: Canvas::PerDevice,
            group: None,
        }])
    }

    /// Every device black. A static scene, not "stop effects": that would hand devices back
    /// to their own modes, which are usually lit.
    pub fn off(&self) -> Result<(), String> {
        self.set_scenes(Self::off_scenes())
    }

    /// Every device black (what `off` runs).
    pub fn off_scenes() -> Vec<Scene> {
        vec![Scene {
            effect: EffectSpec {
                kind: EffectKind::Static,
                colors: vec![Rgb { r: 0, g: 0, b: 0 }],
                speed: 0.0,
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
        }]
    }

    /// Change a zone's LED count. The client refuses zones the server would silently ignore and
    /// sizes outside the zone's range; the engine is told so its next frame has the new layout.
    pub async fn resize_zone(&self, device: u32, zone: u32, size: u32) -> Result<(), String> {
        let client = self
            .inner()
            .client
            .clone()
            .ok_or("OpenRGB is not connected")?;
        client
            .resize_zone(device, zone, size)
            .await
            .map_err(|e| e.to_string())?;
        self.devices_changed();
        Ok(())
    }

    /// Save a `device_mode` look into the device's own memory, so it stays after freeRGB quits
    /// or the PC is off. Refused for modes that do not offer saving.
    pub async fn save_mode(&self, device: u32, effect: &EffectSpec) -> Result<(), String> {
        let client = self.client().ok_or("OpenRGB is not connected")?;
        let data = client.device(device).ok_or("That device is gone")?;
        let name = effect.mode.as_deref().ok_or("Not a built-in mode")?;
        let idx = data
            .modes
            .iter()
            .position(|m| m.name == name)
            .ok_or_else(|| format!("The device has no mode called “{name}”"))?;
        let base = &data.modes[idx];
        if base.flags & mode_flag::MANUAL_SAVE == 0 {
            return Err(format!("“{name}” cannot be saved to the device"));
        }
        let mode = hwmode::apply(base, effect);
        client
            .save_mode(device, idx as u32, &mode)
            .await
            .map_err(|e| e.to_string())
    }

    /// Split a zone into runs, one after another from its first LED; the LEDs after the last
    /// run stay with the device. No runs puts the zone back in one piece. OpenRGB keeps them as
    /// the zone's segments (saved in its own configuration).
    pub async fn set_segments(
        &self,
        device: u32,
        zone: u32,
        runs: &[SegmentRun],
    ) -> Result<(), String> {
        let client = self.client().ok_or("OpenRGB is not connected")?;
        if runs.len() > MAX_SEGMENTS {
            return Err(format!("A zone holds at most {MAX_SEGMENTS} segments"));
        }
        let mut start = 0u32;
        let mut segments = vec![];
        for r in runs {
            let name = r.name.trim();
            if name.chars().count() > MAX_SEGMENT_NAME {
                return Err(format!(
                    "A segment name has at most {MAX_SEGMENT_NAME} characters"
                ));
            }
            segments.push(Segment {
                name: name.to_string(),
                segment_type: zone_type::LINEAR,
                start_idx: start,
                leds_count: r.leds,
                ..Default::default()
            });
            start = start.saturating_add(r.leds);
        }
        client
            .set_segments(device, zone, &segments)
            .await
            .map_err(|e| e.to_string())
    }

    /// Stop the engine and let it put devices back in their previous modes. Call before the
    /// server is stopped.
    pub async fn shutdown(&self) {
        let engine = {
            let mut g = self.inner();
            g.client = None;
            let e = g.engine.take();
            g.follow();
            e
        };
        if let Some(e) = engine {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(4), e.stop()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freergb_mock::{devices, MockServer};
    use freergb_sdk::ConnectOptions;
    use std::time::Duration;

    async fn connect(server: &MockServer) -> Client {
        Client::connect(server.addr(), ConnectOptions::default())
            .await
            .unwrap()
    }

    fn rainbow_all() -> Scene {
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

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    fn rgb_set(server: &MockServer, id: u32) -> std::collections::HashSet<(u8, u8, u8)> {
        server
            .device(id)
            .unwrap()
            .colors
            .iter()
            .map(|c| (c.r, c.g, c.b))
            .collect()
    }

    /// The panel's scenes (ui/src/lib/scene-fixtures.json, checked against the TS builder by
    /// `npm test`) must parse into `Scene` and validate: `Scene` rejects unknown fields, so any
    /// drift between the two sides shows up here instead of as a silent UI failure.
    #[test]
    fn every_scene_the_ui_builds_is_accepted() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/lib/scene-fixtures.json"
        );
        let raw = std::fs::read_to_string(path).unwrap();
        let fixtures: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(fixtures.len(), 20);
        let mut kinds = std::collections::HashSet::new();
        let mut named_devices = vec![];
        for f in fixtures {
            let scene: Scene = serde_json::from_value(f["scene"].clone())
                .unwrap_or_else(|e| panic!("{e}: {}", f["scene"]));
            scene.validate().unwrap();
            kinds.insert(format!("{:?}", scene.effect.kind));
            if let freergb_effects::Targets::Devices(keys) = &scene.targets {
                named_devices.push(keys.len());
            }
        }
        named_devices.sort();
        assert_eq!(
            named_devices,
            [1, 1, 1, 1, 2, 2],
            "four single-device scenes (device modes, one with zone looks, a painting) and two on several devices"
        );
        assert_eq!(
            kinds.len(),
            14,
            "every effect kind must be covered: {kinds:?}"
        );
    }

    #[test]
    fn the_scene_format_rejects_a_misspelt_field() {
        let bad =
            r##"{"effect":{"type":"rainbow","colours":[]},"targets":"all","canvas":"shared"}"##;
        assert!(serde_json::from_str::<Scene>(bad).is_err());
    }

    #[tokio::test]
    async fn scenes_need_a_connection_but_stopping_does_not() {
        let l = Lights::default();
        assert!(l.set_scenes(vec![rainbow_all()]).is_err());
        assert!(l.scenes().is_empty());
        assert!(l.set_scenes(vec![]).is_ok());
        assert!(l.paint(1, 0, 0, 0).is_err());
    }

    #[tokio::test]
    async fn a_remembered_look_is_applied_on_connect() {
        let server = MockServer::start(vec![devices::big_300()]).await.unwrap();
        let id = server.device_ids()[0];
        let l = Lights::default();
        l.remember(vec![rainbow_all()]);
        assert_eq!(l.scenes(), vec![rainbow_all()]);
        let before = server.device(id).unwrap().colors;
        l.attach(Some(&connect(&server).await));
        settle().await;
        assert_ne!(server.device(id).unwrap().colors, before);
        l.shutdown().await;
    }

    #[test]
    fn rewrite_works_while_disconnected_and_refuses_an_invalid_result() {
        let l = Lights::default();
        l.remember(vec![rainbow_all()]);
        let grouped = Scene {
            targets: Targets::Devices(vec![]),
            ..rainbow_all()
        };
        assert!(
            l.rewrite(|_| vec![grouped]).is_err(),
            "no devices is invalid"
        );
        assert_eq!(l.scenes(), vec![rainbow_all()]);
        l.rewrite(|s| s.iter().chain(s).cloned().collect()).unwrap();
        assert_eq!(l.scenes().len(), 2);
    }

    #[test]
    fn remember_never_replaces_a_newer_choice_or_takes_invalid_scenes() {
        let l = Lights::default();
        let mut bad = rainbow_all();
        bad.effect.speed = -1.0;
        l.remember(vec![bad]);
        assert!(l.scenes().is_empty());
        l.remember(vec![rainbow_all()]);
        let mut other = rainbow_all();
        other.effect.speed = 2.0;
        l.remember(vec![other]);
        assert_eq!(l.scenes(), vec![rainbow_all()]);
    }

    #[tokio::test]
    async fn a_source_runs_while_a_connected_look_follows_it() {
        use freergb_effects::react::{Drive, React, SourceId};
        use freergb_sources::Polled;
        let server = MockServer::start(vec![devices::ram_stick(0)])
            .await
            .unwrap();
        let c = connect(&server).await;
        let hub = Arc::new(Hub::new(
            vec![Box::new(Polled {
                id: SourceId::CpuLoad,
                period: std::time::Duration::from_millis(50),
                read: Arc::new(|| Some(50.0)),
            })],
            tokio::runtime::Handle::current(),
        ));
        let l = Lights::default();
        l.set_hub(hub.clone());
        let mut look = rainbow_all();
        look.effect.react = Some(React {
            source: SourceId::CpuLoad,
            drives: Drive::Amount,
            low: None,
            high: None,
            sensitivity: None,
            strength: None,
        });
        // Remembered while disconnected: nothing to light, so nothing reads.
        l.remember(vec![look.clone()]);
        assert!(hub.running().is_empty());
        l.attach(Some(&c));
        assert_eq!(hub.running(), [SourceId::CpuLoad]);
        l.set_scenes(vec![rainbow_all()]).unwrap();
        assert!(hub.running().is_empty(), "no look follows it any more");
        l.set_scenes(vec![look]).unwrap();
        assert_eq!(hub.running(), [SourceId::CpuLoad]);
        l.shutdown().await;
        assert!(hub.running().is_empty(), "quitting stops every source");
    }

    #[tokio::test]
    async fn a_zone_is_split_into_runs_one_after_another() {
        let server = MockServer::start(vec![devices::big_300()]).await.unwrap();
        let id = server.device_ids()[0];
        let c = connect(&server).await;
        let l = Lights::default();
        l.attach(Some(&c));
        let run = |name: &str, leds| SegmentRun {
            name: name.into(),
            leds,
        };
        l.set_segments(id, 0, &[run(" Desk ", 100), run("Shelf", 60)])
            .await
            .unwrap();
        let got: Vec<_> = server.device(id).unwrap().zones[0]
            .segments
            .iter()
            .map(|s| (s.name.clone(), s.start_idx, s.leds_count))
            .collect();
        assert_eq!(got, [("Desk".into(), 0, 100), ("Shelf".into(), 100, 60)]);
        // Too many LEDs for the zone, or a blank name: refused, the old runs stay.
        assert!(l.set_segments(id, 0, &[run("A", 301)]).await.is_err());
        assert!(l.set_segments(id, 0, &[run("", 3)]).await.is_err());
        assert_eq!(server.device(id).unwrap().zones[0].segments.len(), 2);
        l.set_segments(id, 0, &[]).await.unwrap();
        assert!(server.device(id).unwrap().zones[0].segments.is_empty());
        l.shutdown().await;
    }

    #[tokio::test]
    async fn only_a_mode_the_device_can_keep_is_saved_to_it() {
        let server = MockServer::start(vec![devices::with_rich_modes(devices::ram_stick(0))])
            .await
            .unwrap();
        let id = server.device_ids()[0];
        let c = connect(&server).await;
        let l = Lights::default();
        l.attach(Some(&c));
        server.set_recording(true);
        let mut look = rainbow_all().effect;
        look.kind = EffectKind::DeviceMode;
        look.mode = Some("Breathing".into());
        look.speed = 0.0;
        l.save_mode(id, &look).await.unwrap();
        let saves = |s: &MockServer| {
            s.received()
                .iter()
                .filter(|p| p.pkt_id == freergb_sdk::ids::pkt::SAVE_MODE)
                .count()
        };
        assert_eq!(saves(&server), 1);
        let d = server.device(id).unwrap();
        let breathing = &d.modes[d.active_mode as usize];
        // Speed position 0 is the start of the inverted 100..10 range.
        assert_eq!(
            (breathing.name.as_str(), breathing.speed),
            ("Breathing", 100)
        );

        look.mode = Some("Static".into());
        assert!(
            l.save_mode(id, &look).await.is_err(),
            "Static has no MANUAL_SAVE"
        );
        look.mode = Some("Nope".into());
        assert!(l.save_mode(id, &look).await.is_err());
        assert_eq!(saves(&server), 1);
        l.shutdown().await;
    }

    #[tokio::test]
    async fn an_invalid_scene_changes_nothing() {
        let server = MockServer::start(vec![devices::big_300()]).await.unwrap();
        let c = connect(&server).await;
        let l = Lights::default();
        l.attach(Some(&c));
        l.set_scenes(vec![rainbow_all()]).unwrap();
        let mut bad = rainbow_all();
        bad.effect.speed = f64::NAN;
        assert!(l.set_scenes(vec![bad]).is_err());
        assert_eq!(l.scenes(), vec![rainbow_all()]);
        l.shutdown().await;
    }

    #[tokio::test]
    async fn effects_survive_a_server_restart() {
        let server = MockServer::start(vec![devices::big_300()]).await.unwrap();
        let id = server.device_ids()[0];
        let l = Lights::default();
        let first = connect(&server).await;
        l.attach(Some(&first));
        l.set_scenes(vec![rainbow_all()]).unwrap();
        settle().await;
        assert!(rgb_set(&server, id).len() > 50);

        // The connection dies; the supervisor publishes "no client", then a new one.
        server.crash();
        for _ in 0..100 {
            if !first.is_connected() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(!first.is_connected());
        l.attach(None);
        assert_eq!(l.scenes().len(), 1, "scenes forgotten while disconnected");
        server.restart().await.unwrap();
        let id = server.device_ids()[0];
        let before = server.device(id).unwrap().colors;
        l.attach(Some(&connect(&server).await));
        settle().await;
        assert_ne!(
            server.device(id).unwrap().colors,
            before,
            "scene not re-applied"
        );
        l.shutdown().await;
    }

    #[tokio::test]
    async fn paint_replaces_effects_and_restores_the_other_devices() {
        let mut a = devices::ram_stick(0);
        let mut b = devices::ram_stick(1);
        a.active_mode = 1;
        b.active_mode = 1;
        let server = MockServer::start(vec![a, b]).await.unwrap();
        let ids = server.device_ids();
        let l = Lights::default();
        l.attach(Some(&connect(&server).await));
        l.set_scenes(vec![rainbow_all()]).unwrap();
        settle().await;
        assert_eq!(server.device(ids[0]).unwrap().active_mode, 0);
        assert_eq!(server.device(ids[1]).unwrap().active_mode, 0);

        l.paint(ids[0], 255, 0, 0).unwrap();
        settle().await;
        let painted = server.device(ids[0]).unwrap();
        assert!(painted
            .colors
            .iter()
            .all(|c| (c.r, c.g, c.b) == (255, 0, 0)));
        assert_eq!(painted.active_mode, 0, "painted device left Direct mode");
        assert_eq!(
            server.device(ids[1]).unwrap().active_mode,
            1,
            "the other device was not restored"
        );
        assert_eq!(l.scenes().len(), 1);
        l.shutdown().await;
    }

    #[tokio::test]
    async fn resizing_a_zone_updates_the_device_and_the_running_effect_follows() {
        let server = MockServer::start(vec![devices::resizable_strip()])
            .await
            .unwrap();
        let id = server.device_ids()[0];
        let l = Lights::default();
        l.attach(Some(&connect(&server).await));
        l.set_scenes(vec![rainbow_all()]).unwrap();
        settle().await;
        assert_eq!(server.device(id).unwrap().colors.len(), 60);

        l.resize_zone(id, 0, 120).await.unwrap();
        settle().await;
        let d = server.device(id).unwrap();
        assert_eq!(d.zones[0].leds_count, 120);
        assert_eq!(d.colors.len(), 120);
        // The rainbow covers the new length: the last LED is not left at its old colour.
        let distinct: std::collections::HashSet<_> =
            d.colors.iter().map(|c| (c.r, c.g, c.b)).collect();
        assert!(distinct.len() > 60, "{} distinct colours", distinct.len());
        l.shutdown().await;
    }

    #[tokio::test]
    async fn resizing_refuses_fixed_zones_bad_sizes_and_a_missing_connection() {
        let server = MockServer::start(vec![devices::ram_stick(0), devices::resizable_strip()])
            .await
            .unwrap();
        let ids = server.device_ids();
        let l = Lights::default();
        assert!(l
            .resize_zone(ids[0], 0, 5)
            .await
            .unwrap_err()
            .contains("not connected"));
        l.attach(Some(&connect(&server).await));
        assert!(
            l.resize_zone(ids[0], 0, 5).await.is_err(),
            "RAM zone is fixed"
        );
        assert!(
            l.resize_zone(ids[1], 0, 301).await.is_err(),
            "above the zone's maximum"
        );
        assert!(
            l.resize_zone(ids[1], 0, 0).await.is_err(),
            "below the zone's minimum"
        );
        assert!(l.resize_zone(ids[1], 9, 10).await.is_err(), "no such zone");
        assert_eq!(server.device(ids[1]).unwrap().zones[0].leds_count, 60);
        l.shutdown().await;
    }

    #[tokio::test]
    async fn off_blacks_out_every_device() {
        let server = MockServer::start(vec![devices::ram_stick(0), devices::big_300()])
            .await
            .unwrap();
        let l = Lights::default();
        l.attach(Some(&connect(&server).await));
        l.set_scenes(vec![rainbow_all()]).unwrap();
        settle().await;
        l.off().unwrap();
        settle().await;
        for id in server.device_ids() {
            assert_eq!(rgb_set(&server, id), [(0, 0, 0)].into_iter().collect());
        }
        l.shutdown().await;
    }

    #[tokio::test]
    async fn paint_rejects_an_unknown_device() {
        let server = MockServer::start(vec![devices::ram_stick(0)])
            .await
            .unwrap();
        let l = Lights::default();
        l.attach(Some(&connect(&server).await));
        assert!(l.paint(99, 1, 2, 3).unwrap_err().contains("no such device"));
        l.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_restores_modes_while_the_server_is_still_up() {
        let mut d = devices::ram_stick(0);
        d.active_mode = 2;
        let server = MockServer::start(vec![d]).await.unwrap();
        let id = server.device_ids()[0];
        let l = Lights::default();
        l.attach(Some(&connect(&server).await));
        l.set_scenes(vec![rainbow_all()]).unwrap();
        settle().await;
        assert_eq!(server.device(id).unwrap().active_mode, 0);
        l.shutdown().await;
        assert_eq!(server.device(id).unwrap().active_mode, 2);
        assert!(
            l.set_scenes(vec![rainbow_all()]).is_err(),
            "usable after shutdown"
        );
    }
}
