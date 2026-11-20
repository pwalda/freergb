// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-loop behaviour against an in-memory backend, on a paused clock so frame counts and
//! timings are exact rather than approximate.
// The trait declares `-> impl Future`, so the fake's impls mirror that signature.
#![allow(clippy::manual_async_fn)]

use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Scene, Targets};
use freergb_effects::{Engine, EngineConfig, StopBehavior};
use freergb_mock::devices;
use freergb_sdk::{Color, ControllerData, DeviceKey, Mode, Result, RgbBackend, SdkError};
use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone)]
enum Call {
    Custom(u32),
    Leds(u32, Vec<Color>, Instant),
    Mode(u32, u32, String),
    /// The full mode written, for checking values.
    ModeValues(Mode),
    /// device, zone, mode index (-1 = follow the device), the mode's name and speed if any.
    ZoneMode(u32, u32, i32, Option<(String, u32)>),
}

#[derive(Default)]
struct State {
    devices: BTreeMap<u32, ControllerData>,
    log: Vec<Call>,
    stalled: HashSet<u32>,
    gone: HashSet<u32>,
    disconnected: bool,
    /// How often the engine read the device list.
    reads: usize,
}

#[derive(Default)]
struct Fake(Mutex<State>);

impl Fake {
    fn with(devs: Vec<(u32, ControllerData)>) -> Arc<Self> {
        let f = Fake::default();
        f.0.lock().unwrap().devices = devs.into_iter().collect();
        Arc::new(f)
    }
    fn log(&self) -> Vec<Call> {
        self.0.lock().unwrap().log.clone()
    }
    fn frames(&self, dev: u32) -> Vec<(Vec<Color>, Instant)> {
        self.log()
            .into_iter()
            .filter_map(|c| match c {
                Call::Leds(d, f, t) if d == dev => Some((f, t)),
                _ => None,
            })
            .collect()
    }
    fn count_leds(&self, dev: u32) -> usize {
        self.frames(dev).len()
    }
    fn customs(&self, dev: u32) -> usize {
        self.log()
            .iter()
            .filter(|c| matches!(c, Call::Custom(d) if *d == dev))
            .count()
    }
    fn mode_calls(&self) -> Vec<(u32, u32, String)> {
        self.log()
            .into_iter()
            .filter_map(|c| match c {
                Call::Mode(d, i, n) => Some((d, i, n)),
                _ => None,
            })
            .collect()
    }
    fn written_modes(&self) -> Vec<Mode> {
        self.log()
            .into_iter()
            .filter_map(|c| match c {
                Call::ModeValues(m) => Some(m),
                _ => None,
            })
            .collect()
    }
    fn set(&self, f: impl FnOnce(&mut State)) {
        f(&mut self.0.lock().unwrap());
    }
}

impl RgbBackend for Fake {
    fn devices(&self) -> Vec<(u32, ControllerData)> {
        let mut s = self.0.lock().unwrap();
        s.reads += 1;
        s.devices
            .iter()
            .filter(|(id, _)| !s.gone.contains(id))
            .map(|(i, d)| (*i, d.clone()))
            .collect()
    }
    fn update_leds_nowait(
        &self,
        device: u32,
        colors: &[Color],
    ) -> impl Future<Output = Result<()>> + Send {
        let verdict = {
            let mut s = self.0.lock().unwrap();
            if s.disconnected {
                Some(Err(SdkError::Disconnected))
            } else if s.gone.contains(&device) {
                Some(Err(SdkError::UnknownDevice(device)))
            } else if s.stalled.contains(&device) {
                None
            } else {
                s.log
                    .push(Call::Leds(device, colors.to_vec(), Instant::now()));
                Some(Ok(()))
            }
        };
        async move {
            match verdict {
                Some(r) => r,
                None => std::future::pending().await,
            }
        }
    }
    fn update_leds(&self, d: u32, c: &[Color]) -> impl Future<Output = Result<()>> + Send {
        self.update_leds_nowait(d, c)
    }
    fn set_custom_mode(&self, device: u32) -> impl Future<Output = Result<()>> + Send {
        let r = {
            let mut s = self.0.lock().unwrap();
            if s.disconnected {
                Err(SdkError::Disconnected)
            } else if s.gone.contains(&device) {
                Err(SdkError::UnknownDevice(device))
            } else {
                s.log.push(Call::Custom(device));
                if let Some(d) = s.devices.get_mut(&device) {
                    d.active_mode = 0; // "Direct"
                }
                Ok(())
            }
        };
        async move { r }
    }
    fn update_mode(
        &self,
        device: u32,
        mode_idx: u32,
        mode: &Mode,
    ) -> impl Future<Output = Result<()>> + Send {
        let mut s = self.0.lock().unwrap();
        s.log.push(Call::Mode(device, mode_idx, mode.name.clone()));
        s.log.push(Call::ModeValues(mode.clone()));
        if let Some(d) = s.devices.get_mut(&device) {
            d.active_mode = mode_idx as i32;
        }
        async { Ok(()) }
    }
    fn update_zone_mode(
        &self,
        device: u32,
        zone: u32,
        mode_idx: i32,
        mode: Option<&Mode>,
    ) -> impl Future<Output = Result<()>> + Send {
        let mut s = self.0.lock().unwrap();
        s.log.push(Call::ZoneMode(
            device,
            zone,
            mode_idx,
            mode.map(|m| (m.name.clone(), m.speed)),
        ));
        if let Some(z) = s
            .devices
            .get_mut(&device)
            .and_then(|d| d.zones.get_mut(zone as usize))
        {
            z.active_mode = mode_idx;
            if let (Some(m), Ok(i)) = (mode, usize::try_from(mode_idx)) {
                z.modes[i] = m.clone();
            }
        }
        async { Ok(()) }
    }
    fn resize_zone(&self, _: u32, _: u32, _: u32) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
    fn list_profiles(&self) -> impl Future<Output = Result<Vec<String>>> + Send {
        async { Ok(vec![]) }
    }
    fn save_profile(&self, _: &str) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
    fn load_profile(&self, _: &str) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
    fn delete_profile(&self, _: &str) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}

/// A device that is in its "Static" mode (index 1), so a restore has something to restore.
fn dev(slot: u32) -> ControllerData {
    let mut d = devices::ram_stick(slot);
    d.active_mode = 1;
    d
}

fn rainbow(targets: Targets) -> Scene {
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
        targets,
        canvas: Canvas::Shared,
        group: None,
    }
}

fn static_red() -> Scene {
    Scene {
        effect: EffectSpec {
            kind: EffectKind::Static,
            colors: vec![freergb_effects::Rgb { r: 255, g: 0, b: 0 }],
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
    }
}

fn static_red_on(k: DeviceKey) -> Scene {
    Scene {
        targets: Targets::Devices(vec![k]),
        ..static_red()
    }
}

fn cfg(fps: u32) -> EngineConfig {
    EngineConfig {
        fps,
        ..Default::default()
    }
}

async fn run_for(secs: f64) {
    tokio::time::sleep(Duration::from_secs_f64(secs)).await;
}

fn key(f: &Fake, id: u32) -> DeviceKey {
    let devices = f.devices();
    freergb_sdk::keys_of(devices.iter().map(|(i, d)| (*i, d)))
        .into_iter()
        .find(|(i, _)| *i == id)
        .unwrap()
        .1
}

#[tokio::test(start_paused = true)]
async fn an_idle_engine_sleeps_and_wakes_for_the_next_scene() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    run_for(5.0).await;
    assert_eq!(f.0.lock().unwrap().reads, 0, "no work while nothing plays");
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    let n = f.frames(1).len();
    assert!((29..=31).contains(&n), "{n} frames in the first second");
    e.clear();
    run_for(0.5).await;
    let reads = |f: &Fake| f.0.lock().unwrap().reads;
    let before = (reads(&f), f.frames(1).len());
    run_for(5.0).await;
    let after = (reads(&f), f.frames(1).len());
    assert_eq!(after, before, "asleep again after stopping");
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn the_frame_rate_changes_live_and_the_animation_keeps_its_clock() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    let at30 = f.frames(1).len();
    e.set_fps(60);
    run_for(2.0).await;
    let frames = f.frames(1);
    let at60 = frames.len() - at30;
    assert!(
        (118..=121).contains(&at60),
        "{at60} frames in 2 s at 60 fps"
    );
    let min_gap = frames.windows(2).map(|w| w[1].1 - w[0].1).min().unwrap();
    assert!(
        min_gap >= Duration::from_secs_f64(1.0 / 60.0 - 0.001),
        "{min_gap:?}"
    );
    // No mode switch for the change: the device went into custom mode once.
    assert_eq!(f.customs(1), 1);
    e.set_fps(30);
    run_for(1.0).await;
    let back = f.frames(1).len() - frames.len();
    assert!(
        (29..=31).contains(&back),
        "{back} frames in 1 s back at 30 fps"
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn frame_rate_is_capped_and_frames_are_evenly_spaced() {
    for fps in [10u32, 30, 60] {
        let f = Fake::with(vec![(1, dev(0))]);
        let e = Engine::start(f.clone(), cfg(fps));
        e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
        run_for(2.0).await;
        let frames = f.frames(1);
        let n = frames.len() as u32;
        assert!(
            (fps * 2 - 2..=fps * 2 + 1).contains(&n),
            "fps {fps}: {n} frames in 2 s"
        );
        let min_gap = frames.windows(2).map(|w| w[1].1 - w[0].1).min().unwrap();
        assert!(
            min_gap >= Duration::from_secs_f64(1.0 / fps as f64 - 0.001),
            "fps {fps}: frames only {min_gap:?} apart"
        );
        e.stop().await;
    }
}

#[tokio::test(start_paused = true)]
async fn a_static_effect_is_sent_once() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![static_red()]).unwrap();
    run_for(3.0).await;
    assert_eq!(f.count_leds(1), 1);
    assert_eq!(f.count_leds(2), 1);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn custom_mode_is_set_once_per_device_before_its_first_frame() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    for id in [1, 2] {
        assert_eq!(f.customs(id), 1, "device {id}");
        let log = f.log();
        let custom = log
            .iter()
            .position(|c| matches!(c, Call::Custom(d) if *d == id))
            .unwrap();
        let first = log
            .iter()
            .position(|c| matches!(c, Call::Leds(d, ..) if *d == id))
            .unwrap();
        assert!(custom < first);
    }
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_stalled_device_does_not_stop_the_others() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    f.set(|s| {
        s.stalled.insert(1);
    });
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(2.0).await;
    assert_eq!(f.count_leds(1), 0);
    assert!(f.count_leds(2) >= 58, "{}", f.count_leds(2));
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn stats_count_sent_and_dropped_frames() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    f.set(|s| {
        s.stalled.insert(1);
    });
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(2.0).await;
    let st = e.stats();
    assert_eq!(st.sent as usize, f.count_leds(2));
    assert!(st.sent >= 58, "{st:?}");
    assert!(
        st.dropped >= 58,
        "the stalled device's frames are not counted: {st:?}"
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_recovered_device_gets_fresh_frames_not_a_backlog() {
    let f = Fake::with(vec![(1, dev(0))]);
    f.set(|s| {
        s.stalled.insert(1);
    });
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(2.0).await;
    f.set(|s| {
        s.stalled.clear();
    });
    run_for(1.0).await;
    let frames = f.frames(1);
    assert!(
        (28..=31).contains(&frames.len()),
        "{} frames after recovery",
        frames.len()
    );
    let gaps: Vec<_> = frames.windows(2).map(|w| w[1].1 - w[0].1).collect();
    let min_gap = *gaps.iter().min().unwrap();
    assert!(
        min_gap >= Duration::from_millis(32),
        "burst: {min_gap:?} gaps {gaps:?}"
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn an_unplugged_device_is_dropped_and_the_rest_carry_on() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    f.set(|s| {
        s.gone.insert(1);
    });
    let before = f.count_leds(2);
    run_for(2.0).await;
    assert!(f.count_leds(2) >= before + 58);
    let frozen = f.count_leds(1);
    run_for(1.0).await;
    assert_eq!(f.count_leds(1), frozen);
    assert!(e.is_running());
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_replugged_device_is_picked_up_again_and_set_up_again() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    f.set(|s| {
        s.gone.insert(1);
    });
    run_for(2.0).await;
    let frozen = f.count_leds(1);
    f.set(|s| {
        s.gone.clear();
    });
    run_for(3.0).await;
    assert!(f.count_leds(1) > frozen + 30);
    assert_eq!(f.customs(1), 2);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn all_targets_follow_hotplug_when_told() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    f.set(|s| {
        s.devices.insert(2, dev(1));
    });
    e.devices_changed();
    run_for(0.2).await;
    assert!(f.count_leds(2) > 0);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn the_device_list_is_re_read_without_being_told() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    f.set(|s| {
        s.devices.insert(2, dev(1));
    });
    run_for(2.5).await;
    assert!(f.count_leds(2) > 0);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn clear_restores_each_devices_original_mode_once() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    assert!(f.mode_calls().is_empty(), "restored while still running");
    e.clear();
    run_for(0.5).await;
    let mut calls = f.mode_calls();
    calls.sort();
    assert_eq!(
        calls,
        vec![(1, 1, "Static".to_string()), (2, 1, "Static".to_string())]
    );
    run_for(1.0).await;
    assert_eq!(f.mode_calls().len(), 2);
    e.stop().await;
    assert_eq!(f.mode_calls().len(), 2, "stop restored a second time");
}

#[tokio::test(start_paused = true)]
async fn each_run_restores_the_mode_the_device_had_before_that_run() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    for _ in 0..2 {
        e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
        run_for(1.0).await;
        e.clear();
        run_for(0.5).await;
    }
    e.stop().await;
    assert_eq!(
        f.mode_calls(),
        vec![(1, 1, "Static".to_string()), (1, 1, "Static".to_string())]
    );
}

#[tokio::test(start_paused = true)]
async fn stop_restores_and_leave_does_not() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    e.stop().await;
    assert_eq!(f.mode_calls().len(), 1);

    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(
        f.clone(),
        EngineConfig {
            on_stop: StopBehavior::Leave,
            ..cfg(30)
        },
    );
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    e.clear();
    run_for(0.5).await;
    e.stop().await;
    assert!(f.mode_calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn stop_with_leave_keeps_the_last_frame_on_a_restoring_engine() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    let frames = f.frames(1).len();
    e.stop_with(StopBehavior::Leave).await;
    assert!(f.mode_calls().is_empty(), "a mode was restored");
    assert_eq!(f.frames(1).len(), frames, "a frame was sent after stopping");
}

#[tokio::test(start_paused = true)]
async fn narrowing_the_targets_releases_only_the_dropped_devices() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(1.0).await;
    let k2 = key(&f, 2);
    e.set_scenes(vec![rainbow(Targets::Devices(vec![k2]))])
        .unwrap();
    run_for(0.5).await;
    assert_eq!(f.mode_calls(), vec![(1, 1, "Static".to_string())]);
    let n1 = f.count_leds(1);
    run_for(1.0).await;
    assert_eq!(f.count_leds(1), n1, "released device still being driven");
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn replacing_a_scene_does_not_re_run_custom_mode_setup() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    e.set_scenes(vec![static_red()]).unwrap();
    run_for(0.5).await;
    assert_eq!(f.customs(1), 1);
    assert!(f.mode_calls().is_empty());
    // The mode it had before the *first* scene comes back, once.
    e.clear();
    run_for(0.5).await;
    assert_eq!(f.mode_calls(), vec![(1, 1, "Static".to_string())]);
    assert_eq!(f.customs(1), 1);
    e.stop().await;
}

/// Adding or editing one device's scene must not make the others' animations jump back to
/// the start. Without carried clocks, device 1 would replay its first frames after the change.
#[tokio::test(start_paused = true)]
async fn editing_one_scene_does_not_restart_the_others_animation() {
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let (k1, k2) = (key(&f, 1), key(&f, 2));
    let e = Engine::start(f.clone(), cfg(30));
    let on_1 = rainbow(Targets::Devices(vec![k1]));
    e.set_scenes(vec![on_1.clone()]).unwrap();
    run_for(0.5).await;
    let before: Vec<_> = f.frames(1).into_iter().map(|(c, _)| c).collect();
    assert!(before.len() >= 10);

    e.set_scenes(vec![on_1, static_red_on(k2)]).unwrap();
    run_for(0.4).await;
    let after: Vec<_> = f
        .frames(1)
        .into_iter()
        .skip(before.len())
        .map(|(c, _)| c)
        .collect();
    assert!(after.len() >= 8, "device 1 stopped animating");
    let replayed = after.iter().filter(|c| before.contains(c)).count();
    assert_eq!(
        replayed, 0,
        "device 1 restarted its animation: {replayed} replayed frames"
    );
    // The new scene did start from the beginning on its own device.
    assert!(f.count_leds(2) >= 1);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn an_invalid_scene_is_rejected_and_the_old_one_keeps_running() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    let mut bad = static_red();
    bad.effect.colors.clear();
    assert!(e.set_scenes(vec![bad]).is_err());
    let before = f.count_leds(1);
    run_for(1.0).await;
    assert!(f.count_leds(1) > before + 25);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_disconnect_ends_the_loop() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    assert!(e.is_running());
    f.set(|s| s.disconnected = true);
    run_for(0.5).await;
    assert!(!e.is_running());
}

#[tokio::test(start_paused = true)]
async fn a_shared_canvas_sweeps_across_devices_and_per_device_does_not() {
    // Same wave on two identical sticks: shared, the two show different phases; per-device,
    // identical pictures.
    let make = |canvas| {
        let mut s = rainbow(Targets::All);
        s.canvas = canvas;
        s.effect.speed = 0.0;
        s
    };
    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![make(Canvas::Shared)]).unwrap();
    run_for(0.2).await;
    assert_ne!(f.frames(1)[0].0, f.frames(2)[0].0);
    e.stop().await;

    let f = Fake::with(vec![(1, dev(0)), (2, dev(1))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![make(Canvas::PerDevice)]).unwrap();
    run_for(0.2).await;
    assert_eq!(f.frames(1)[0].0, f.frames(2)[0].0);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn dropping_the_engine_ends_the_loop() {
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    drop(e);
    run_for(0.1).await;
    let n = f.count_leds(1);
    run_for(1.0).await;
    assert_eq!(f.count_leds(1), n);
}

// ---- device modes: the device's own built-in modes, run by the hardware ----

const WAVE: &str = "Rainbow Wave"; // index 2 on `dev`: speed 0..100, left/right, brightness

fn device_mode(k: DeviceKey, mode: &str, speed: f64) -> Scene {
    Scene {
        effect: EffectSpec {
            kind: EffectKind::DeviceMode,
            colors: vec![],
            speed,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: Some(mode.into()),
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        },
        targets: Targets::Devices(vec![k]),
        canvas: Canvas::Shared,
        group: None,
    }
}

/// (zone, mode index, the mode's name and speed) as written to device 1.
type ZoneCall = (u32, i32, Option<(String, u32)>);

fn zone_calls(f: &Fake) -> Vec<ZoneCall> {
    f.log()
        .into_iter()
        .filter_map(|c| match c {
            Call::ZoneMode(1, z, i, m) => Some((z, i, m)),
            _ => None,
        })
        .collect()
}

fn pulse(zone: u32, speed: f64) -> freergb_effects::ZoneLook {
    freergb_effects::ZoneLook {
        zone,
        mode: "Pulse".into(),
        colors: vec![],
        speed,
        brightness: 1.0,
        mode_direction: None,
    }
}

#[tokio::test(start_paused = true)]
async fn zones_get_their_own_modes_and_only_what_changed_is_sent() {
    let f = Fake::with(vec![(1, devices::fan_hub())]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    let mut sc = device_mode(k.clone(), "Static", 0.5);
    sc.effect.zones = vec![pulse(1, 0.25)];
    e.set_scenes(vec![sc.clone()]).unwrap();
    run_for(1.0).await;
    assert_eq!(f.mode_calls(), vec![(1, 1, "Static".to_string())]);
    // Fan 2 gets Pulse (index 1 in its own list) at speed 25; fans 1 and 3 already follow.
    assert_eq!(zone_calls(&f), vec![(1, 1, Some(("Pulse".into(), 25)))]);

    // Moving fan 2's slider resends fan 2 only; a new zone look on fan 1 sends fan 1 only.
    sc.effect.zones = vec![pulse(1, 0.75), pulse(0, 0.0)];
    e.set_scenes(vec![sc.clone()]).unwrap();
    run_for(1.0).await;
    assert_eq!(
        f.mode_calls().len(),
        1,
        "the device's own mode is not resent"
    );
    assert_eq!(
        zone_calls(&f)[1..],
        [
            (0, 1, Some(("Pulse".into(), 0))),
            (1, 1, Some(("Pulse".into(), 75)))
        ]
    );

    // Dropping fan 1's look puts it back on the device's mode.
    sc.effect.zones = vec![pulse(1, 0.75)];
    e.set_scenes(vec![sc.clone()]).unwrap();
    run_for(1.0).await;
    assert_eq!(zone_calls(&f).last(), Some(&(0, -1, None)));

    // Stopping puts every zone back as it was: all three followed the device.
    let before = zone_calls(&f).len();
    e.clear();
    run_for(0.5).await;
    assert_eq!(
        zone_calls(&f)[before..],
        [(0, -1, None), (1, -1, None), (2, -1, None)]
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_mode_that_drives_the_whole_device_leaves_zones_alone() {
    let f = Fake::with(vec![(1, devices::fan_hub())]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    let mut sc = device_mode(k, "Super Rainbow", 0.5);
    sc.effect.zones = vec![pulse(0, 0.5)];
    e.set_scenes(vec![sc]).unwrap();
    run_for(1.0).await;
    assert_eq!(f.mode_calls(), vec![(1, 3, "Super Rainbow".to_string())]);
    assert!(zone_calls(&f).is_empty());
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_device_mode_is_one_mapped_write_with_no_frames_and_no_custom_mode() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    let mut sc = device_mode(k, WAVE, 0.25);
    sc.effect.brightness = 0.5;
    sc.effect.mode_direction = Some(freergb_effects::ModeDirection::Right);
    e.set_scenes(vec![sc]).unwrap();
    run_for(2.0).await;
    // However long it runs, the mode is written once; the hardware does the animating.
    assert_eq!(f.mode_calls(), vec![(1, 2, WAVE.to_string())]);
    let m = &f.written_modes()[0];
    assert_eq!((m.speed, m.brightness), (25, 50));
    assert_eq!(m.direction, freergb_sdk::ids::direction::RIGHT);
    assert_eq!(f.count_leds(1), 0);
    assert_eq!(f.customs(1), 0);
    e.clear();
    run_for(0.5).await;
    assert_eq!(f.mode_calls().last(), Some(&(1, 1, "Static".to_string())));
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn editing_a_device_mode_writes_again_and_restores_the_original_only() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![device_mode(k.clone(), WAVE, 0.2)])
        .unwrap();
    run_for(0.5).await;
    e.set_scenes(vec![device_mode(k, WAVE, 0.8)]).unwrap();
    run_for(0.5).await;
    e.clear();
    run_for(0.5).await;
    // Two writes of the built-in mode, then exactly one restore, to the mode the device had
    // before the app took over (Static), not to the first built-in mode.
    assert_eq!(
        f.mode_calls(),
        vec![
            (1, 2, WAVE.to_string()),
            (1, 2, WAVE.to_string()),
            (1, 1, "Static".to_string())
        ]
    );
    let speeds: Vec<u32> = f.written_modes().iter().map(|m| m.speed).collect();
    assert_eq!(speeds[..2], [20, 80]);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn software_to_device_mode_and_back_keeps_the_original_and_never_restores_in_between() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    assert_eq!(f.customs(1), 1);
    e.set_scenes(vec![device_mode(k, WAVE, 0.5)]).unwrap();
    run_for(0.5).await;
    // The first mode write is the built-in mode itself: no restore of the old mode slipped in.
    assert_eq!(f.mode_calls(), vec![(1, 2, WAVE.to_string())]);
    let frames_before = f.count_leds(1);
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    assert_eq!(f.customs(1), 2, "back to custom mode for the frames");
    assert!(f.count_leds(1) > frames_before, "frames resumed");
    e.clear();
    run_for(0.5).await;
    assert_eq!(
        f.mode_calls(),
        vec![(1, 2, WAVE.to_string()), (1, 1, "Static".to_string())]
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn an_unknown_mode_name_leaves_the_device_alone() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![device_mode(k, "No Such Mode", 0.5)])
        .unwrap();
    run_for(1.0).await;
    assert!(f.log().is_empty());
    e.clear();
    run_for(0.5).await;
    assert!(
        f.log().is_empty(),
        "nothing was taken, so nothing is restored"
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_device_that_disappears_and_returns_gets_its_mode_again() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![device_mode(k, WAVE, 0.5)]).unwrap();
    run_for(1.5).await;
    f.set(|s| {
        s.gone.insert(1);
    });
    run_for(1.5).await;
    f.set(|s| {
        s.gone.remove(&1);
    });
    run_for(1.5).await;
    let writes = f.mode_calls().iter().filter(|c| c.2 == WAVE).count();
    assert_eq!(
        writes,
        2,
        "applied again after coming back: {:?}",
        f.mode_calls()
    );
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn an_inverted_speed_range_is_respected_when_writing() {
    let mut d = dev(0);
    d.modes[2].speed_min = 100;
    d.modes[2].speed_max = 10;
    let f = Fake::with(vec![(1, d)]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![device_mode(k, WAVE, 0.0)]).unwrap();
    run_for(0.5).await;
    assert_eq!(f.written_modes()[0].speed, 100);
    e.stop().await;
}

// ---- painted looks: colours set by hand, by LED index ----

fn painted(k: DeviceKey, n: u8) -> Scene {
    Scene {
        effect: EffectSpec {
            kind: EffectKind::Painted,
            colors: (0..n)
                .map(|i| freergb_effects::Rgb {
                    r: i + 1,
                    g: 0,
                    b: 0,
                })
                .collect(),
            speed: 0.2,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: None,
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        },
        targets: Targets::Devices(vec![k]),
        canvas: Canvas::Shared,
        group: None,
    }
}

#[tokio::test(start_paused = true)]
async fn a_painted_look_is_one_frame_by_led_index_and_the_old_mode_comes_back() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![painted(k, 5)]).unwrap();
    run_for(2.0).await;
    let frames = f.frames(1);
    assert_eq!(frames.len(), 1, "a still look is sent once");
    let reds: Vec<u8> = frames[0].0.iter().map(|c| c.r).collect();
    assert_eq!(reds, [1, 2, 3, 4, 5]);
    assert_eq!(f.customs(1), 1);
    e.clear();
    run_for(0.5).await;
    assert_eq!(f.mode_calls(), vec![(1, 1, "Static".to_string())]);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn after_a_zone_resize_the_painted_frame_has_the_new_length_and_keeps_the_old_paint() {
    let f = Fake::with(vec![(1, devices::resizable_strip())]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![painted(k, 60)]).unwrap();
    run_for(0.5).await;
    assert_eq!(f.frames(1).last().unwrap().0.len(), 60);

    f.set(|s| {
        let d = s.devices.get_mut(&1).unwrap();
        d.zones[0].leds_count = 120;
        devices::rebuild_leds(d);
    });
    e.devices_changed();
    run_for(0.5).await;
    let frames = f.frames(1);
    let last = &frames.last().unwrap().0;
    assert_eq!(last.len(), 120);
    assert_eq!(last[59].r, 60, "the first 60 LEDs keep their paint");
    assert!(
        last[60..].iter().all(|c| (c.r, c.g, c.b) == (0, 0, 0)),
        "new LEDs start black"
    );

    // and a shrink cuts the paint instead of failing
    f.set(|s| {
        let d = s.devices.get_mut(&1).unwrap();
        d.zones[0].leds_count = 10;
        devices::rebuild_leds(d);
    });
    e.devices_changed();
    run_for(0.5).await;
    assert_eq!(f.frames(1).last().unwrap().0.len(), 10);
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn painted_and_software_looks_swap_without_extra_mode_changes() {
    let f = Fake::with(vec![(1, dev(0))]);
    let k = key(&f, 1);
    let e = Engine::start(f.clone(), cfg(30));
    e.set_scenes(vec![painted(k, 5)]).unwrap();
    run_for(0.5).await;
    e.set_scenes(vec![rainbow(Targets::All)]).unwrap();
    run_for(0.5).await;
    assert_eq!(f.customs(1), 1, "still the same custom-mode session");
    assert!(f.count_leds(1) > 5, "the rainbow animates after the paint");
    e.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_look_that_follows_the_sound_changes_with_the_readings_only() {
    use freergb_effects::react::{Drive, React, Readings, SourceId};
    let f = Fake::with(vec![(1, dev(0))]);
    let e = Engine::start(f.clone(), cfg(30));
    let mut sc = static_red();
    sc.effect.react = Some(React {
        source: SourceId::Sound,
        drives: Drive::Brightness,
        low: None,
        high: None,
        sensitivity: None,
        strength: None,
    });
    e.set_scenes(vec![sc]).unwrap();
    run_for(0.5).await;
    let last = |f: &Fake| f.frames(1).last().map(|(c, _)| c[0]);
    // No reading yet: the look as it is.
    assert_eq!(last(&f).map(|c| c.r), Some(255));
    let sent = f.count_leds(1);
    e.set_readings(Readings {
        sound: Some(0.5),
        ..Default::default()
    });
    run_for(0.5).await;
    assert_eq!(last(&f).map(|c| c.r), Some(128));
    // A still reading sends nothing new: a static look stays one frame per change.
    assert_eq!(f.count_leds(1), sent + 1);
    e.stop().await;
}
