// SPDX-License-Identifier: GPL-2.0-or-later
//! The frame loop.
//!
//! Every tick the engine renders each targeted device and sends the frames that changed.
//! Guarantees (each pinned by a test in `tests/engine.rs`):
//!
//! * never more than `fps` frames per second per device, and no backlog: a tick that cannot
//!   send in time drops that frame and the next tick renders fresh, so a slow server shows
//!   old colours late rather than very old colours later;
//! * a frame identical to the last one sent is not sent (a static effect costs nothing after
//!   its first frame);
//! * a device that stalls or vanishes does not stop the others;
//! * with no scenes (and no device left to restore) the loop sleeps instead of ticking;
//! * each device is switched to its custom/direct mode once before its first frame, and, by
//!   default, put back in its previous mode when no scene targets it any more;
//! * a `painted` scene is a fixed frame by LED index (padded or cut to the device's current
//!   LED count, so a zone resize just produces the next frame);
//! * a `device_mode` scene hands the device to one of its own built-in modes instead: one mode
//!   write (again when the scene changes), no custom mode and no frames. The mode the device
//!   had before the engine first touched it is remembered across switches between the two.
//!
//! Scenes name devices by `DeviceKey`, so the engine re-resolves them against the live device
//! list; `Targets::All` follows hotplug. LEDs sit on the desk map given by
//! [`Engine::set_map`] (see `layout`); devices missing from it take slots in target order. The device list is re-read when the scenes change,
//! when [`Engine::devices_changed`] is called, and at least every `refresh`.

use crate::hwmode;
use crate::layout::{self, Placed, Placement, Point};
use crate::react::{self, Readings};
use crate::render::{painted_frame, render_at, render_frame};
use crate::spec::{Canvas, EffectKind, EffectSpec, Rgb, Scene, SpecError, Targets};
use freergb_sdk::ids::mode_flag;
use freergb_sdk::{keys_of, resolve, Color, ControllerData, DeviceKey, Mode, RgbBackend, SdkError};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{interval_at, timeout, timeout_at, Instant, MissedTickBehavior};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBehavior {
    /// Leave the last frame on the hardware, in custom mode.
    Leave,
    /// Put each device back in the mode it had before the engine touched it.
    RestoreMode,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Frames per second, clamped to 1..=120.
    pub fps: u32,
    /// Longest the device list goes without being re-read while scenes are active.
    pub refresh: Duration,
    pub on_stop: StopBehavior,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            refresh: Duration::from_secs(1),
            on_stop: StopBehavior::RestoreMode,
        }
    }
}

struct SceneSet {
    scenes: Vec<Scene>,
    /// When each scene's clock started, parallel to `scenes`.
    started: Vec<Instant>,
}

/// Start times for `new`: a scene that is identical to one already running keeps that clock,
/// so editing one device's effect does not restart the animation on every other device.
fn carry_starts(old: &SceneSet, new: &[Scene], now: Instant) -> Vec<Instant> {
    new.iter()
        .map(|s| {
            old.scenes
                .iter()
                .position(|o| o == s)
                .map_or(now, |i| old.started[i])
        })
        .collect()
}

/// Counters since the engine started, for measuring a real setup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Frames handed to the backend and accepted.
    pub sent: u64,
    /// Frames abandoned because the backend was not ready within one frame period.
    pub dropped: u64,
}

#[derive(Default)]
struct Counters {
    sent: AtomicU64,
    dropped: AtomicU64,
}

/// Handle to a running frame loop. Dropping it aborts the loop without restoring modes;
/// use [`Engine::stop`] for a tidy exit.
pub struct Engine {
    scenes: watch::Sender<Arc<SceneSet>>,
    map: watch::Sender<Arc<Vec<Placed>>>,
    hidden: watch::Sender<Arc<Vec<DeviceKey>>>,
    readings: watch::Sender<Arc<Readings>>,
    fps: watch::Sender<u32>,
    devices_dirty: Arc<AtomicBool>,
    counters: Arc<Counters>,
    /// How the loop ends, once asked to.
    stop: watch::Sender<Option<StopBehavior>>,
    on_stop: StopBehavior,
    task: Option<JoinHandle<()>>,
}

impl Engine {
    /// Spawns onto the current tokio runtime.
    pub fn start<B: RgbBackend + 'static>(backend: Arc<B>, cfg: EngineConfig) -> Self {
        let (scenes, scenes_rx) = watch::channel(Arc::new(SceneSet {
            scenes: vec![],
            started: vec![],
        }));
        let (stop, stop_rx) = watch::channel(None);
        let on_stop = cfg.on_stop;
        let (fps, fps_rx) = watch::channel(cfg.fps);
        let (map, map_rx) = watch::channel(Arc::new(vec![]));
        let (hidden, hidden_rx) = watch::channel(Arc::new(vec![]));
        let (readings, readings_rx) = watch::channel(Arc::new(Readings::default()));
        let devices_dirty = Arc::new(AtomicBool::new(true));
        let counters = Arc::new(Counters::default());
        let task = tokio::spawn(run(
            backend,
            cfg,
            Inputs {
                scenes_rx,
                fps_rx,
                map_rx,
                hidden_rx,
                readings_rx,
                stop_rx,
            },
            devices_dirty.clone(),
            counters.clone(),
        ));
        Self {
            scenes,
            map,
            hidden,
            readings,
            fps,
            devices_dirty,
            counters,
            stop,
            on_stop,
            task: Some(task),
        }
    }

    /// Replace the active scenes. New or changed scenes start at time zero; a scene identical
    /// to one already running keeps its clock. When several scenes target one device, the
    /// later scene wins. Nothing changes if any scene is invalid.
    pub fn set_scenes(&self, scenes: Vec<Scene>) -> Result<(), SpecError> {
        for s in &scenes {
            s.validate()?;
        }
        // `send_modify` holds the channel lock, so two callers cannot both carry clocks over
        // from the same old set.
        self.scenes.send_modify(|cur| {
            let started = carry_starts(cur, &scenes, Instant::now());
            *cur = Arc::new(SceneSet { scenes, started });
        });
        Ok(())
    }

    /// Change the frame rate (clamped to 1..=120) without restarting: running scenes keep
    /// their clocks and devices keep the modes remembered for restoring.
    pub fn set_fps(&self, fps: u32) {
        self.fps.send_if_modified(|cur| {
            let changed = *cur != fps;
            *cur = fps;
            changed
        });
    }

    /// Replace the desk map. Running scenes keep their clocks; the next frame uses the new
    /// layout.
    pub fn set_map(&self, map: Vec<Placed>) {
        self.map.send_if_modified(|cur| {
            let changed = **cur != map;
            *cur = Arc::new(map);
            changed
        });
    }

    /// Devices no scene may drive, whatever it targets. One that was driven goes back to its
    /// previous mode, as when no scene targets it any more.
    pub fn set_hidden(&self, hidden: Vec<DeviceKey>) {
        self.hidden.send_if_modified(|cur| {
            let changed = **cur != hidden;
            *cur = Arc::new(hidden);
            changed
        });
    }

    /// The latest readings for looks that follow one; the next frame uses them.
    pub fn set_readings(&self, r: Readings) {
        self.readings.send_replace(Arc::new(r));
    }

    /// Stop animating; devices go back to their previous mode (per `StopBehavior`).
    pub fn clear(&self) {
        let _ = self.set_scenes(vec![]);
    }

    /// Call after a hotplug or zone resize so the next frame uses the new layout.
    pub fn devices_changed(&self) {
        self.devices_dirty.store(true, Ordering::Relaxed);
    }

    pub fn stats(&self) -> Stats {
        Stats {
            sent: self.counters.sent.load(Ordering::Relaxed),
            dropped: self.counters.dropped.load(Ordering::Relaxed),
        }
    }

    /// False once the loop has ended on its own (the backend disconnected).
    pub fn is_running(&self) -> bool {
        self.task.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Stop the loop and restore device modes if configured.
    pub async fn stop(self) {
        let how = self.on_stop;
        self.stop_with(how).await;
    }

    /// Stop the loop, ending as `how` says rather than as configured.
    pub async fn stop_with(mut self, how: StopBehavior) {
        let _ = self.stop.send(Some(how));
        if let Some(t) = self.task.take() {
            let _ = t.await;
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}

/// One device's place in the plan.
#[derive(Debug, Clone, PartialEq)]
struct Assignment {
    scene: usize,
    /// Canvas position `(u, v)` of each LED.
    positions: Vec<Point>,
    /// Each LED's place on the scene's desk area, 0..1 both ways (y down): where a look that
    /// follows the screen samples it.
    desk: Vec<Point>,
}

/// Desk points to 0..1 over their bounding box, each axis on its own (a flat row sits at 0.5).
fn desk_area(desk: &[Vec<Point>]) -> impl Fn(Point) -> Point {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in desk.iter().flatten() {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    move |p: Point| {
        let f = |k: usize| {
            let span = max[k] - min[k];
            if span > 0.0 {
                (p[k] - min[k]) / span
            } else {
                0.5
            }
        };
        [f(0), f(1)]
    }
}

/// Decide which scene drives which device and where each LED sits on that scene's canvas.
fn plan(
    devices: &[(u32, ControllerData)],
    scenes: &[Scene],
    map: &[Placed],
    hidden: &[DeviceKey],
) -> BTreeMap<u32, Assignment> {
    let keys = keys_of(devices.iter().map(|(id, d)| (*id, d)));
    let shown = |id: &u32| !keys.iter().any(|(i, k)| i == id && hidden.contains(k));
    let entries = |id: u32| -> Vec<&Placed> {
        match keys.iter().find(|(i, _)| *i == id) {
            Some((_, key)) => map.iter().filter(|p| &p.device == key).collect(),
            None => vec![],
        }
    };
    let mut out = BTreeMap::new();
    for (si, scene) in scenes.iter().enumerate() {
        let ids: Vec<u32> = match &scene.targets {
            Targets::All => devices.iter().map(|(id, _)| *id).collect(),
            Targets::Devices(saved) => resolve(saved, &keys).into_iter().flatten().collect(),
        };
        let ids: Vec<u32> = ids.into_iter().filter(shown).collect();
        let found: Vec<(u32, &ControllerData)> = ids
            .into_iter()
            .filter_map(|id| devices.iter().find(|(i, _)| *i == id).map(|(_, d)| (id, d)))
            .collect();
        // Every piece of every device (a device split into segments has several), in order.
        let mut owner: Vec<(usize, Vec<usize>)> = vec![];
        let mut pieces: Vec<(Vec<Point>, Option<Placement>)> = vec![];
        for (k, (id, d)) in found.iter().enumerate() {
            for (leds, local, at) in layout::placed_pieces(d, &entries(*id)) {
                owner.push((k, leds));
                pieces.push((local, at));
            }
        }
        let desk = layout::arrange(&pieces);
        let area = desk_area(&desk);
        let angle = scene.effect.angle();
        let mut positions: Vec<Vec<Point>> = found
            .iter()
            .map(|(_, d)| vec![[0.0, 0.0]; d.colors.len()])
            .collect();
        let mut desks = positions.clone();
        for ((dev, leds), pts) in owner.iter().zip(&desk) {
            for (&led, &p) in leds.iter().zip(pts) {
                desks[*dev][led] = area(p);
            }
        }
        // Hand device `k`'s canvas points (its pieces' LEDs in piece order) back to its LEDs.
        let mut scatter = |k: usize, uv: &mut std::vec::IntoIter<Point>| {
            for (dev, leds) in owner.iter().filter(|(d, _)| *d == k) {
                for (&led, p) in leds.iter().zip(uv.by_ref()) {
                    positions[*dev][led] = p;
                }
            }
        };
        match scene.canvas {
            Canvas::PerDevice => {
                for k in 0..found.len() {
                    let mine: Vec<Point> = owner
                        .iter()
                        .zip(&desk)
                        .filter(|((d, _), _)| *d == k)
                        .flat_map(|(_, pts)| pts.iter().copied())
                        .collect();
                    scatter(k, &mut layout::canvas(&mine, angle).into_iter());
                }
            }
            Canvas::Shared => {
                let mut all = layout::canvas(&desk.concat(), angle).into_iter();
                // Pieces of one device sit next to each other in `desk`, devices in order.
                for k in 0..found.len() {
                    scatter(k, &mut all);
                }
            }
        }
        for (((id, _), positions), desk) in found.into_iter().zip(positions).zip(desks) {
            out.insert(
                id,
                Assignment {
                    scene: si,
                    positions,
                    desk,
                },
            );
        }
    }
    out
}

struct DevState {
    /// Mode to put back, captured before the engine first changed the device's mode.
    original: Option<(u32, Mode)>,
    /// Zones with modes of their own to put back: (zone, its active mode, that mode).
    original_zones: Vec<(u32, i32, Option<Mode>)>,
    last: Option<Vec<Color>>,
    /// The device is in custom mode, ready for frames.
    custom: bool,
    /// The `device_mode` effect last written to the device, if it is in one of its own modes.
    applied: Option<EffectSpec>,
}

const SETUP_TIMEOUT: Duration = Duration::from_secs(2);

/// What the [`Engine`] handle changes while the loop runs.
struct Inputs {
    scenes_rx: watch::Receiver<Arc<SceneSet>>,
    fps_rx: watch::Receiver<u32>,
    map_rx: watch::Receiver<Arc<Vec<Placed>>>,
    hidden_rx: watch::Receiver<Arc<Vec<DeviceKey>>>,
    readings_rx: watch::Receiver<Arc<Readings>>,
    stop_rx: watch::Receiver<Option<StopBehavior>>,
}

async fn run<B: RgbBackend + 'static>(
    backend: Arc<B>,
    cfg: EngineConfig,
    inputs: Inputs,
    devices_dirty: Arc<AtomicBool>,
    counters: Arc<Counters>,
) {
    let Inputs {
        mut scenes_rx,
        mut fps_rx,
        mut map_rx,
        mut hidden_rx,
        readings_rx,
        mut stop_rx,
    } = inputs;
    let ticker = |fps: u32, first: Instant| {
        let period = Duration::from_secs_f64(1.0 / fps.clamp(1, 120) as f64);
        let mut t = interval_at(first, period);
        // Delay, not Skip: after a tick that overran, Skip fires the next one early to get back
        // on the grid, which would briefly exceed the frame-rate cap.
        t.set_missed_tick_behavior(MissedTickBehavior::Delay);
        (t, period)
    };
    let (mut tick, mut period) = ticker(*fps_rx.borrow_and_update(), Instant::now());

    let mut set = scenes_rx.borrow_and_update().clone();
    let mut devices: Vec<(u32, ControllerData)> = vec![];
    let mut assignments: BTreeMap<u32, Assignment> = BTreeMap::new();
    let mut states: HashMap<u32, DevState> = HashMap::new();
    // Devices whose setup failed, so they are not retried every frame.
    let mut cooling: HashMap<u32, Instant> = HashMap::new();
    let mut last_refresh = Instant::now();
    let mut buf: Vec<Rgb> = vec![];

    'frames: loop {
        if set.scenes.is_empty() && states.is_empty() && !scenes_rx.has_changed().unwrap_or(false) {
            // Nothing to play and nothing to put back: sleep without ticking until a scene
            // arrives, so an idle tray costs no wake-ups.
            tokio::select! {
                r = scenes_rx.changed() => if r.is_err() { break },
                _ = stop_rx.changed() => break,
            }
            // `changed` marked the new set as seen, so take it here; the first frame is now.
            set = scenes_rx.borrow_and_update().clone();
            devices_dirty.store(true, Ordering::Relaxed);
            (tick, period) = ticker(*fps_rx.borrow_and_update(), Instant::now());
        }
        tokio::select! {
            _ = tick.tick() => {}
            _ = stop_rx.changed() => break,
        }
        if stop_rx.borrow().is_some() {
            break;
        }
        if fps_rx.has_changed().unwrap_or(false) {
            // The next frame is one new period out, so a rate change never squeezes two frames
            // closer than the new period.
            let fps = *fps_rx.borrow_and_update();
            let next = Duration::from_secs_f64(1.0 / fps.clamp(1, 120) as f64);
            (tick, period) = ticker(fps, Instant::now() + next);
        }

        let mut dirty = devices_dirty.swap(false, Ordering::Relaxed);
        if scenes_rx.has_changed().unwrap_or(false) {
            set = scenes_rx.borrow_and_update().clone();
            dirty = true;
        }
        if map_rx.has_changed().unwrap_or(false) {
            map_rx.mark_unchanged();
            dirty = true;
        }
        if hidden_rx.has_changed().unwrap_or(false) {
            hidden_rx.mark_unchanged();
            dirty = true;
        }
        if !set.scenes.is_empty() && last_refresh.elapsed() >= cfg.refresh {
            dirty = true;
        }
        if dirty {
            last_refresh = Instant::now();
            devices = backend.devices();
            assignments = plan(&devices, &set.scenes, &map_rx.borrow(), &hidden_rx.borrow());
            let released: Vec<u32> = states
                .keys()
                .filter(|id| !assignments.contains_key(id))
                .copied()
                .collect();
            for id in released {
                if let Some(st) = states.remove(&id) {
                    restore(&*backend, cfg.on_stop, id, st).await;
                }
            }
            cooling.retain(|_, until| *until > Instant::now());
        }
        if assignments.is_empty() {
            continue;
        }

        let mut sends: JoinSet<(u32, Result<(), SdkError>, Vec<Color>)> = JoinSet::new();

        for (&id, asg) in &assignments {
            if cooling.contains_key(&id) {
                continue;
            }
            let (Some(scene), Some(started)) =
                (set.scenes.get(asg.scene), set.started.get(asg.scene))
            else {
                continue;
            };
            let device = devices.iter().find(|(i, _)| *i == id).map(|(_, d)| d);
            let original = || device.and_then(mode_of);

            if scene.effect.kind == EffectKind::DeviceMode {
                let Some(data) = device else { continue };
                let prev = states.get(&id).and_then(|s| s.applied.clone());
                if prev.as_ref() == Some(&scene.effect) {
                    continue;
                }
                match apply_device_mode(&*backend, id, data, &scene.effect, prev.as_ref()).await {
                    Ok(true) => {
                        let st = states.entry(id).or_insert_with(|| DevState {
                            original: original(),
                            original_zones: zone_modes_of(data),
                            last: None,
                            custom: false,
                            applied: None,
                        });
                        st.custom = false;
                        st.last = None;
                        st.applied = Some(scene.effect.clone());
                    }
                    // A name this device does not have leaves it alone rather than guessing.
                    Ok(false) => {}
                    Err(SdkError::Disconnected) => break 'frames,
                    Err(_) => {
                        cooling.insert(id, Instant::now() + cfg.refresh);
                    }
                }
                continue;
            }

            if !states.get(&id).is_some_and(|s| s.custom) {
                match timeout(SETUP_TIMEOUT, backend.set_custom_mode(id)).await {
                    Ok(Ok(())) => {
                        let st = states.entry(id).or_insert_with(|| DevState {
                            original: original(),
                            original_zones: device.map(zone_modes_of).unwrap_or_default(),
                            last: None,
                            custom: false,
                            applied: None,
                        });
                        st.custom = true;
                        st.last = None;
                        st.applied = None;
                    }
                    Ok(Err(SdkError::Disconnected)) => break 'frames,
                    Ok(Err(_)) | Err(_) => {
                        cooling.insert(id, Instant::now() + cfg.refresh);
                        continue;
                    }
                }
            }
            let t = started.elapsed().as_secs_f64();
            if scene.effect.kind == EffectKind::Painted {
                // By LED index, whatever the canvas: `positions` has one entry per LED.
                painted_frame(&scene.effect, asg.positions.len(), &mut buf);
            } else if scene.effect.react.is_some() {
                let r = readings_rx.borrow().clone();
                let spec = react::frame_spec(&scene.effect, &r);
                buf.clear();
                buf.extend(asg.positions.iter().zip(&asg.desk).map(|(&[x, y], &d)| {
                    react::per_led(&scene.effect, &r, render_at(&spec, t, x, y), x, d)
                }));
            } else {
                render_frame(&scene.effect, t, &asg.positions, &mut buf);
            }
            let frame: Vec<Color> = buf
                .iter()
                .map(|c| Color {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                })
                .collect();
            if states.get(&id).and_then(|s| s.last.as_ref()) == Some(&frame) {
                continue;
            }
            let backend = backend.clone();
            sends.spawn(async move {
                let r = backend.update_leds_nowait(id, &frame).await;
                (id, r, frame)
            });
        }

        // Collect results, but never wait past this frame's budget: unfinished sends are
        // aborted, `last` stays as it was, and the next tick renders a fresh frame.
        let deadline = Instant::now() + period;
        while let Ok(Some(joined)) = timeout_at(deadline, sends.join_next()).await {
            let Ok((id, result, frame)) = joined else {
                continue;
            };
            match result {
                Ok(()) => {
                    counters.sent.fetch_add(1, Ordering::Relaxed);
                    if let Some(st) = states.get_mut(&id) {
                        st.last = Some(frame);
                    }
                }
                Err(SdkError::Disconnected) => break 'frames,
                Err(SdkError::UnknownDevice(_)) => {
                    // Unplugged: forget it; the next refresh decides whether it is back.
                    states.remove(&id);
                    cooling.insert(id, Instant::now() + cfg.refresh);
                }
                Err(_) => {}
            }
        }
        counters
            .dropped
            .fetch_add(sends.len() as u64, Ordering::Relaxed);
        sends.abort_all();
    }

    let how = stop_rx.borrow().unwrap_or(cfg.on_stop);
    for (id, st) in states.drain() {
        restore(&*backend, how, id, st).await;
    }
}

/// Put a device in one of its own modes, and each zone with modes of its own in the zone's
/// mode from the look or back on the device's mode. Only what differs from `prev` (the look
/// last written) is sent, so moving one zone's slider does not resend the rest. `Ok(false)`:
/// the device has no mode of that name, and nothing was sent.
async fn apply_device_mode<B: RgbBackend>(
    backend: &B,
    id: u32,
    data: &ControllerData,
    look: &EffectSpec,
    prev: Option<&EffectSpec>,
) -> Result<bool, SdkError> {
    let Some(idx) = data
        .modes
        .iter()
        .position(|m| Some(m.name.as_str()) == look.mode.as_deref())
    else {
        return Ok(false);
    };
    let whole = |e: &EffectSpec| EffectSpec {
        zones: vec![],
        react: None,
        ..e.clone()
    };
    if prev.map(whole) != Some(whole(look)) {
        let mode = hwmode::apply(&data.modes[idx], look);
        timed(backend.update_mode(id, idx as u32, &mode)).await?;
    }
    // A mode that drives the whole device leaves its zones alone (OpenRGB's own page does too).
    if data.modes[idx].flags & mode_flag::REQUIRES_ENTIRE_DEVICE != 0 {
        return Ok(true);
    }
    for (zi, zone) in data.zones.iter().enumerate() {
        if zone.modes.is_empty() {
            continue;
        }
        let zi = zi as u32;
        let want = look.zones.iter().find(|z| z.zone == zi);
        let had = prev.and_then(|p| p.zones.iter().find(|z| z.zone == zi));
        if prev.is_some() && want == had {
            continue;
        }
        let target = want.and_then(|z| {
            let i = zone.modes.iter().position(|m| m.name == z.mode)?;
            Some((i, hwmode::apply(&zone.modes[i], &z.effect())))
        });
        match target {
            Some((i, mode)) => {
                timed(backend.update_zone_mode(id, zi, i as i32, Some(&mode))).await?;
            }
            None if zone.active_mode != -1 => {
                timed(backend.update_zone_mode(id, zi, -1, None)).await?;
            }
            None => {}
        }
    }
    Ok(true)
}

async fn timed<F: std::future::Future<Output = Result<(), SdkError>>>(
    f: F,
) -> Result<(), SdkError> {
    timeout(SETUP_TIMEOUT, f)
        .await
        .unwrap_or(Err(SdkError::BadReply("no reply in time")))
}

fn zone_modes_of(d: &ControllerData) -> Vec<(u32, i32, Option<Mode>)> {
    d.zones
        .iter()
        .enumerate()
        .filter(|(_, z)| !z.modes.is_empty())
        .map(|(i, z)| {
            let mode = usize::try_from(z.active_mode)
                .ok()
                .and_then(|m| z.modes.get(m).cloned());
            (
                i as u32,
                if mode.is_some() { z.active_mode } else { -1 },
                mode,
            )
        })
        .collect()
}

fn mode_of(d: &ControllerData) -> Option<(u32, Mode)> {
    let idx = usize::try_from(d.active_mode).ok()?;
    d.modes.get(idx).map(|m| (idx as u32, m.clone()))
}

async fn restore<B: RgbBackend>(backend: &B, how: StopBehavior, id: u32, st: DevState) {
    if how != StopBehavior::RestoreMode {
        return;
    }
    if let Some((idx, mode)) = st.original {
        let _ = timeout(SETUP_TIMEOUT, backend.update_mode(id, idx, &mode)).await;
    }
    for (zone, idx, mode) in st.original_zones {
        let _ = timeout(
            SETUP_TIMEOUT,
            backend.update_zone_mode(id, zone, idx, mode.as_ref()),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{EffectKind, EffectSpec};
    use freergb_mock::devices;
    use freergb_sdk::DeviceKey;

    fn scene(targets: Targets, canvas: Canvas) -> Scene {
        Scene {
            effect: EffectSpec {
                kind: EffectKind::Rainbow,
                colors: vec![],
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
            targets,
            canvas,
            group: None,
        }
    }

    fn two_devices() -> Vec<(u32, ControllerData)> {
        vec![(3, devices::ram_stick(0)), (7, devices::ram_stick(1))]
    }

    fn key_of(devs: &[(u32, ControllerData)], id: u32) -> DeviceKey {
        keys_of(devs.iter().map(|(i, d)| (*i, d)))
            .into_iter()
            .find(|(i, _)| *i == id)
            .unwrap()
            .1
    }

    #[test]
    fn all_targets_every_device_in_id_order_on_a_shared_canvas() {
        let devs = two_devices();
        let p = plan(&devs, &[scene(Targets::All, Canvas::Shared)], &[], &[]);
        assert_eq!(p.keys().copied().collect::<Vec<_>>(), vec![3, 7]);
        let a = &p[&3].positions;
        let b = &p[&7].positions;
        assert!(a.iter().all(|x| x[0] <= 0.5) && b.iter().all(|x| x[0] >= 0.5));
    }

    #[test]
    fn per_device_canvas_gives_each_device_the_full_range() {
        let devs = two_devices();
        let p = plan(&devs, &[scene(Targets::All, Canvas::PerDevice)], &[], &[]);
        assert_eq!(p[&3].positions.first().map(|p| p[0]), Some(0.0));
        assert_eq!(p[&7].positions.last().map(|p| p[0]), Some(1.0));
    }

    #[test]
    fn named_targets_follow_the_key_not_the_id_and_keep_their_order() {
        let devs = two_devices();
        let (k3, k7) = (key_of(&devs, 3), key_of(&devs, 7));
        // Same devices after a "restart" that renumbered them.
        let renumbered: Vec<_> = devs.iter().cloned().map(|(id, d)| (id + 10, d)).collect();
        let sc = scene(Targets::Devices(vec![k7, k3]), Canvas::Shared);
        let p = plan(&renumbered, &[sc], &[], &[]);
        // k7 is first in the target list, so it takes the left slot.
        assert!(p[&17].positions.iter().all(|x| x[0] <= 0.5));
        assert!(p[&13].positions.iter().all(|x| x[0] >= 0.5));
    }

    #[test]
    fn absent_devices_are_skipped_and_slots_close_up() {
        let devs = two_devices();
        let (k3, k7) = (key_of(&devs, 3), key_of(&devs, 7));
        let only_7 = vec![devs[1].clone()];
        let sc = scene(Targets::Devices(vec![k3, k7]), Canvas::Shared);
        let p = plan(&only_7, &[sc], &[], &[]);
        assert_eq!(p.keys().copied().collect::<Vec<_>>(), vec![7]);
        assert_eq!(p[&7].positions.first().map(|p| p[0]), Some(0.0));
        assert_eq!(p[&7].positions.last().map(|p| p[0]), Some(1.0));
    }

    #[test]
    fn the_desk_map_decides_where_devices_sit() {
        let devs = two_devices();
        let (k3, k7) = (key_of(&devs, 3), key_of(&devs, 7));
        let at = |x: f64| Placement {
            x,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            rot: 0.0,
        };
        // 3 sits right of 7 on the desk, whatever the id order.
        let map = vec![
            Placed {
                device: k3,
                at: Some(at(5.0)),
                points: None,
                shape: None,
                length: None,
                segment: None,
            },
            Placed {
                device: k7,
                at: Some(at(0.0)),
                points: None,
                shape: None,
                length: None,
                segment: None,
            },
        ];
        let p = plan(&devs, &[scene(Targets::All, Canvas::Shared)], &map, &[]);
        assert!(p[&7].positions.iter().all(|x| x[0] <= 0.5));
        assert!(p[&3].positions.iter().all(|x| x[0] >= 0.5));
        // Reversed by the angle.
        let mut back = scene(Targets::All, Canvas::Shared);
        back.effect.angle_deg = Some(180.0);
        let p = plan(&devs, &[back], &map, &[]);
        assert!(p[&3].positions.iter().all(|x| x[0] <= 0.5));
    }

    #[test]
    fn a_segment_sits_where_its_own_entry_puts_it() {
        let mut strip = devices::big_300();
        strip.zones[0].segments = vec![freergb_sdk::Segment {
            name: "Shelf".into(),
            start_idx: 200,
            leds_count: 100,
            ..Default::default()
        }];
        let devs = vec![(1, strip)];
        let key = key_of(&devs, 1);
        let at = |x: f64| Placement {
            x,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            rot: 0.0,
        };
        // The shelf run sits left of the rest of the strip, so a sweep reaches it first.
        let entry = |segment, x| Placed {
            device: key.clone(),
            at: Some(at(x)),
            points: None,
            shape: None,
            length: None,
            segment,
        };
        let shelf = Some(layout::SegmentKey {
            zone: 0,
            start: 200,
        });
        let map = vec![entry(None, 5.0), entry(shelf, 0.0)];
        let p = plan(&devs, &[scene(Targets::All, Canvas::PerDevice)], &map, &[]);
        let pos = &p[&1].positions;
        assert_eq!(pos.len(), 300);
        assert!(pos[200..].iter().all(|x| x[0] <= 0.5));
        assert!(pos[..200].iter().all(|x| x[0] >= 0.5));
        assert_eq!(pos[200][0], 0.0);
        assert_eq!(pos[199][0], 1.0);
    }

    #[test]
    fn an_unchanged_scene_keeps_its_clock_and_a_changed_one_restarts() {
        let t0 = Instant::now();
        let a = scene(Targets::All, Canvas::Shared);
        let mut b = scene(Targets::All, Canvas::PerDevice);
        b.effect.speed = 0.7;
        let old = SceneSet {
            scenes: vec![a.clone()],
            started: vec![t0],
        };
        let later = t0 + Duration::from_secs(5);
        assert_eq!(
            carry_starts(&old, std::slice::from_ref(&a), later),
            vec![t0]
        );
        assert_eq!(
            carry_starts(&old, &[b.clone(), a.clone()], later),
            vec![later, t0]
        );
        assert_eq!(carry_starts(&old, &[], later), Vec::<Instant>::new());
    }

    #[test]
    fn a_later_scene_wins_a_shared_device() {
        let devs = two_devices();
        let k3 = key_of(&devs, 3);
        let p = plan(
            &devs,
            &[
                scene(Targets::All, Canvas::Shared),
                scene(Targets::Devices(vec![k3]), Canvas::PerDevice),
            ],
            &[],
            &[],
        );
        assert_eq!(p[&3].scene, 1);
        assert_eq!(p[&7].scene, 0);
    }

    #[test]
    fn a_hidden_device_is_left_out_of_every_scene() {
        let devs = two_devices();
        let k3 = key_of(&devs, 3);
        let scenes = [
            scene(Targets::All, Canvas::Shared),
            scene(Targets::Devices(vec![k3.clone()]), Canvas::PerDevice),
        ];
        let p = plan(&devs, &scenes, &[], std::slice::from_ref(&k3));
        assert!(!p.contains_key(&3));
        // The rest of the shared canvas spans the whole width without it.
        let xs: Vec<f64> = p[&7].positions.iter().map(|x| x[0]).collect();
        assert!(xs.iter().cloned().fold(f64::MAX, f64::min) < 0.01);
        assert!(xs.iter().cloned().fold(f64::MIN, f64::max) > 0.99);
    }
}
