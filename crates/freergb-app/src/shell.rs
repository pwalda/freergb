// SPDX-License-Identifier: GPL-2.0-or-later
//! Tauri shell: owns the Supervisor, bridges its state and the device list to the UI, runs the
//! tray, and makes quitting stop an owned OpenRGB gracefully (dropping the supervisor would
//! SIGKILL it).

use crate::diag::{Hardware, LogLine, LogState, LogView};
use crate::dto::{devices_of, Detection, DeviceView, ServerState};
use crate::lights::{Lights, SegmentRun};
use crate::schedule::{Action, Schedule};
use crate::store::{
    effect_file_name, effect_from_file, effect_to_file, regroup, DeviceNote, Group, SavedEffect,
    Store, MAX_EFFECT_FILE,
};
use freergb_effects::layout::Placed;
use freergb_effects::react::Readings;
use freergb_effects::spec::{EffectSpec, Scene};
use freergb_power::Power;
use freergb_sdk::{Client, ConnectOptions, DeviceKey, Event};
use freergb_sources::Hub;
use freergb_supervisor::{Config, ServerCommand, Supervisor};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, Wry};

/// Env override for development and tests; otherwise the bundled binary (`bundled_openrgb`)
/// on Windows, or the distro's `openrgb` on Linux (`system_openrgb`).
const ENV_BIN: &str = "FREERGB_OPENRGB_BIN";
const ENV_CONFIG: &str = "FREERGB_OPENRGB_CONFIG";
const ENV_PORT: &str = "FREERGB_OPENRGB_PORT";
/// Passed by the OS autostart entry: start with the tray only, no window.
const TRAY_ARG: &str = "--tray";

struct App {
    /// `None` once shutdown has taken it.
    supervisor: Mutex<Option<Supervisor>>,
    lights: Lights,
    store: Mutex<Store>,
    /// Why saved profiles could not be loaded, shown once by the window.
    store_notice: Mutex<Option<String>>,
    quitting: Mutex<bool>,
    detection: Mutex<Detection>,
    /// Bumped by every scan start, so a stale watchdog leaves a newer scan alone.
    scans: AtomicU64,
    /// The readings looks can follow (temperatures, sound, screen), run on demand.
    hub: Arc<Hub>,
    /// The look before the PC went to sleep, for waking to go back to.
    before_sleep: Mutex<Option<Vec<Scene>>>,
    /// Whether OpenRGB keeps a log, and the epoch of the lines (see `LogView`).
    log: Mutex<(LogState, u64)>,
    /// A "server-log" nudge to the window is on its way.
    log_nudge: AtomicBool,
}

impl App {
    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(serde::Serialize, Clone)]
struct ProfilesView {
    names: Vec<String>,
    /// The profile whose look is running now.
    active: Option<String>,
}

fn profiles_view(app: &App) -> ProfilesView {
    let store = app.store();
    ProfilesView {
        names: store.profiles().iter().map(|p| p.name.clone()).collect(),
        active: store.active(&app.lights.scenes()).map(str::to_string),
    }
}

fn config_from_env() -> Config {
    let port = std::env::var(ENV_PORT)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6742);
    let command = std::env::var_os(ENV_BIN)
        .map(PathBuf::from)
        .or_else(|| bundled_openrgb(&std::env::current_exe().ok()?))
        .or_else(|| system_openrgb(&std::env::var_os("PATH")?))
        .map(|bin| ServerCommand::openrgb(bin, std::env::var_os(ENV_CONFIG).map(PathBuf::from)));
    Config {
        port,
        command,
        connect: ConnectOptions::default(),
        ..Config::default()
    }
}

/// The OpenRGB the Windows installer puts beside the app. Usually its service already
/// answers on the port; this is the fallback when the service is stopped or missing.
pub(crate) fn bundled_openrgb(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?.join("openrgb").join("OpenRGB.exe");
    bin.is_file().then_some(bin)
}

/// Linux has no bundled OpenRGB: the distro package's, found on PATH, is started headless when
/// nothing answers (its systemd service, when enabled, usually does).
fn system_openrgb(path: &std::ffi::OsStr) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    std::env::split_paths(path)
        .map(|d| d.join("openrgb"))
        .find(|bin| bin.is_file())
}

#[tauri::command]
fn server_state(app: State<'_, App>) -> Option<ServerState> {
    let g = app.supervisor.lock().ok()?;
    g.as_ref().map(|s| ServerState::from(&s.state()))
}

#[tauri::command]
fn devices(app: State<'_, App>) -> Vec<DeviceView> {
    let Ok(g) = app.supervisor.lock() else {
        return vec![];
    };
    devices_of(g.as_ref().and_then(|s| s.client()).as_ref())
}

#[tauri::command]
fn retry_server(app: State<'_, App>) {
    if let Ok(g) = app.supervisor.lock() {
        if let Some(s) = g.as_ref() {
            s.retry();
        }
    }
}

/// After the lights changed: remember the look for the next start, and tell the window what is
/// running (so a panel never shows a replaced scene) and which profile that is.
fn scenes_changed(handle: &AppHandle, app: &App) {
    let scenes = app.lights.scenes();
    if let Err(e) = app.store().set_last(&scenes) {
        eprintln!("freergb: {e}");
    }
    let _ = handle.emit("scenes", scenes);
    profiles_changed(handle, app);
}

fn profiles_changed(handle: &AppHandle, app: &App) {
    let view = profiles_view(app);
    refresh_tray(handle, &view);
    let _ = handle.emit("profiles", view);
}

const TRAY_ID: &str = "main";
/// Menu IDs of profile entries; the rest of the ID is the profile's name.
const PROFILE_ITEM: &str = "profile:";

/// Show, a ready update, the profiles (the running one checked), Lights off, About, Quit.
fn tray_menu(handle: &AppHandle, view: &ProfilesView) -> tauri::Result<Menu<Wry>> {
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = vec![Box::new(MenuItem::with_id(
        handle,
        "show",
        "Show freeRGB",
        true,
        None::<&str>,
    )?)];
    if let Some((label, enabled)) = crate::update::tray_item(handle) {
        items.push(Box::new(MenuItem::with_id(
            handle,
            "update",
            label,
            enabled,
            None::<&str>,
        )?));
    }
    items.push(Box::new(PredefinedMenuItem::separator(handle)?));
    if view.names.is_empty() {
        items.push(Box::new(MenuItem::with_id(
            handle,
            "no-profiles",
            "No profiles yet",
            false,
            None::<&str>,
        )?));
    }
    for name in &view.names {
        let active = view.active.as_deref() == Some(name.as_str());
        items.push(Box::new(CheckMenuItem::with_id(
            handle,
            format!("{PROFILE_ITEM}{name}"),
            name,
            true,
            active,
            None::<&str>,
        )?));
    }
    items.push(Box::new(PredefinedMenuItem::separator(handle)?));
    items.push(Box::new(MenuItem::with_id(
        handle,
        "off",
        "Lights off",
        true,
        None::<&str>,
    )?));
    items.push(Box::new(PredefinedMenuItem::separator(handle)?));
    items.push(Box::new(MenuItem::with_id(
        handle,
        "about",
        "About freeRGB",
        true,
        None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(
        handle,
        "quit",
        "Quit",
        true,
        None::<&str>,
    )?));
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Menu::with_items(handle, &refs)
}

/// Rebuilds the tray menu, for changes outside the profiles (a ready update).
pub(crate) fn tray_changed(handle: &AppHandle) {
    refresh_tray(handle, &profiles_view(&handle.state::<App>()));
}

fn refresh_tray(handle: &AppHandle, view: &ProfilesView) {
    let (Some(tray), Ok(menu)) = (handle.tray_by_id(TRAY_ID), tray_menu(handle, view)) else {
        return;
    };
    let _ = tray.set_menu(Some(menu));
}

fn on_tray_menu(handle: &AppHandle, id: &str) {
    let app = handle.state::<App>();
    let result = match id {
        "show" => {
            show_main(handle);
            Ok(())
        }
        "quit" => {
            quit(handle, false);
            Ok(())
        }
        "about" => {
            show_main(handle);
            crate::about::request(handle);
            Ok(())
        }
        "update" => {
            crate::update::install_from_tray(handle);
            Ok(())
        }
        "off" => lights_off(handle, &app),
        _ => match id.strip_prefix(PROFILE_ITEM) {
            Some(name) => apply(handle, &app, name),
            None => Ok(()),
        },
    };
    if let Err(e) = result {
        eprintln!("freergb: {e}");
        let _ = handle.emit("notice", e);
    }
    // A check item toggles itself on click; put the marks back to what is really running.
    profiles_changed(handle, &app);
}

/// Install these scenes (an empty list stops all effects). Rejected scenes change nothing.
#[tauri::command]
fn set_scenes(handle: AppHandle, app: State<'_, App>, scenes: Vec<Scene>) -> Result<(), String> {
    app.lights.set_scenes(scenes)?;
    scenes_changed(&handle, &app);
    Ok(())
}

#[tauri::command]
fn scenes(app: State<'_, App>) -> Vec<Scene> {
    app.lights.scenes()
}

/// Paint one device a single colour (replaces running effects; see `Lights::paint`).
#[tauri::command]
fn set_device_color(
    handle: AppHandle,
    app: State<'_, App>,
    device: u32,
    r: u8,
    g: u8,
    b: u8,
) -> Result<(), String> {
    app.lights.paint(device, r, g, b)?;
    scenes_changed(&handle, &app);
    Ok(())
}

#[tauri::command]
fn profiles(app: State<'_, App>) -> ProfilesView {
    profiles_view(&app)
}

/// Why saved profiles could not be loaded; returned once.
#[tauri::command]
fn store_notice(app: State<'_, App>) -> Option<String> {
    app.store_notice.lock().ok()?.take()
}

/// Save what is running now under this name (replacing a profile of that name).
#[tauri::command]
fn save_profile(handle: AppHandle, app: State<'_, App>, name: String) -> Result<String, String> {
    let scenes = app.lights.scenes();
    let saved = app.store().save_profile(&name, &scenes)?;
    profiles_changed(&handle, &app);
    Ok(saved)
}

fn apply(handle: &AppHandle, app: &App, name: &str) -> Result<(), String> {
    let scenes = app
        .store()
        .profile(name)
        .ok_or_else(|| format!("No profile called “{name}”"))?
        .scenes
        .clone();
    app.lights.set_scenes(scenes)?;
    scenes_changed(handle, app);
    Ok(())
}

#[tauri::command]
fn apply_profile(handle: AppHandle, app: State<'_, App>, name: String) -> Result<(), String> {
    apply(&handle, &app, &name)
}

#[tauri::command]
fn rename_profile(
    handle: AppHandle,
    app: State<'_, App>,
    from: String,
    to: String,
) -> Result<String, String> {
    let renamed = app.store().rename_profile(&from, &to)?;
    profiles_changed(&handle, &app);
    schedule_changed(&handle, &app);
    Ok(renamed)
}

#[tauri::command]
fn delete_profile(handle: AppHandle, app: State<'_, App>, name: String) -> Result<(), String> {
    app.store().delete_profile(&name)?;
    profiles_changed(&handle, &app);
    schedule_changed(&handle, &app);
    Ok(())
}

#[tauri::command]
fn schedule(app: State<'_, App>) -> Schedule {
    app.store().schedule().clone()
}

#[tauri::command]
fn set_schedule(handle: AppHandle, app: State<'_, App>, schedule: Schedule) -> Result<(), String> {
    app.store().set_schedule(schedule)?;
    schedule_changed(&handle, &app);
    Ok(())
}

fn schedule_changed(handle: &AppHandle, app: &App) {
    let _ = handle.emit("schedule", app.store().schedule().clone());
}

/// Do a scheduled action. It changes what runs even while OpenRGB is away (the look is applied
/// once it is back), so a rule or a wake that lands mid-reconnect is not lost.
fn act(app: &App, action: &Action) -> Result<bool, String> {
    let scenes = match action {
        Action::Nothing => return Ok(false),
        Action::Off => Lights::off_scenes(),
        Action::Back => match app
            .before_sleep
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            Some(s) => s,
            None => return Ok(false),
        },
        Action::Profile { name } => app
            .store()
            .profile(name)
            .ok_or_else(|| format!("No profile called “{name}”"))?
            .scenes
            .clone(),
    };
    app.lights.rewrite(|_| scenes)?;
    Ok(true)
}

fn report(handle: &AppHandle, result: Result<bool, String>, keep: bool) {
    let app = handle.state::<App>();
    match result {
        // A sleep look is not kept as the last look: a PC switched off while asleep should
        // start with the look from before.
        Ok(true) if keep => scenes_changed(handle, &app),
        Ok(true) => {
            let _ = handle.emit("scenes", app.lights.scenes());
            profiles_changed(handle, &app);
        }
        Ok(false) => {}
        Err(e) => {
            eprintln!("freergb: schedule: {e}");
            let _ = handle.emit("notice", e);
        }
    }
}

/// Runs on the OS's notifying thread; sleep waits for it.
fn on_power(handle: &AppHandle, p: Power) {
    let app = handle.state::<App>();
    let schedule = app.store().schedule().clone();
    match p {
        Power::Sleep => {
            if schedule.sleep == Action::Nothing {
                return;
            }
            *app.before_sleep.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.lights.scenes());
            let result = act(&app, &schedule.sleep);
            let changed = matches!(result, Ok(true));
            report(handle, result, false);
            if changed {
                // Let the engine's next frames reach the devices before the system stops.
                std::thread::sleep(SLEEP_FLUSH);
            }
        }
        Power::Wake => {
            let result = act(&app, &schedule.wake);
            app.before_sleep
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            report(handle, result, true);
        }
    }
}

/// Long enough for a few frames at 30 fps, well inside the time the OS gives (about 2 s).
const SLEEP_FLUSH: Duration = Duration::from_millis(300);
/// Most the end of a session waits for the lights, inside the time Windows gives (5 s).
const SESSION_END_LIMIT: Duration = Duration::from_millis(2500);
/// How often the schedule looks at the clock.
const SCHEDULE_TICK: Duration = Duration::from_secs(15);

/// Fires the time rules while the app runs.
fn spawn_schedule(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut prev = chrono::Local::now().naive_local();
        let mut tick = tokio::time::interval(SCHEDULE_TICK);
        loop {
            tick.tick().await;
            let now = chrono::Local::now().naive_local();
            let app = handle.state::<App>();
            let due = app
                .store()
                .schedule()
                .due(prev, now)
                .map(|r| r.action.clone());
            prev = now;
            if let Some(action) = due {
                let result = act(&app, &action);
                report(&handle, result, true);
            }
        }
    });
}

#[tauri::command]
fn groups(app: State<'_, App>) -> Vec<Group> {
    app.store().groups().to_vec()
}

/// Create a group (`old` = None) or change one; a look it has follows the change.
#[tauri::command]
fn save_group(
    handle: AppHandle,
    app: State<'_, App>,
    old: Option<String>,
    name: String,
    devices: Vec<DeviceKey>,
) -> Result<Group, String> {
    let g = app.store().save_group(old.as_deref(), &name, devices)?;
    let from = old.unwrap_or_else(|| g.name.clone());
    app.lights.rewrite(|s| regroup(s, &from, Some(&g)))?;
    groups_changed(&handle, &app);
    scenes_changed(&handle, &app);
    Ok(g)
}

/// Delete a group; its devices follow the all-devices look again.
#[tauri::command]
fn delete_group(handle: AppHandle, app: State<'_, App>, name: String) -> Result<(), String> {
    app.store().delete_group(&name)?;
    app.lights.rewrite(|s| regroup(s, &name, None))?;
    groups_changed(&handle, &app);
    scenes_changed(&handle, &app);
    Ok(())
}

/// The user's names for devices and the devices freeRGB leaves alone.
#[tauri::command]
fn device_notes(app: State<'_, App>) -> Vec<DeviceNote> {
    app.store().device_notes().to_vec()
}

/// Rename a device (`None` or blank: its own name again) and hide or show it. A hidden device
/// is driven by no effect; one that was goes back to its own mode.
#[tauri::command]
fn set_device_note(
    handle: AppHandle,
    app: State<'_, App>,
    device: DeviceKey,
    name: Option<String>,
    hidden: bool,
) -> Result<(), String> {
    let hidden_now = {
        let mut store = app.store();
        store.set_device_note(device, name.as_deref(), hidden)?;
        store.hidden()
    };
    app.lights.set_hidden(hidden_now);
    let _ = handle.emit("device-notes", app.store().device_notes().to_vec());
    Ok(())
}

fn groups_changed(handle: &AppHandle, app: &App) {
    let _ = handle.emit("groups", app.store().groups().to_vec());
}

/// "My effects": looks kept under a name.
#[tauri::command]
fn my_effects(app: State<'_, App>) -> Vec<SavedEffect> {
    app.store().effects().to_vec()
}

fn effects_changed(handle: &AppHandle, app: &App) {
    let _ = handle.emit("effects", app.store().effects().to_vec());
}

/// Keep an effect under a name (replacing one of that name), or rename/replace `old`.
#[tauri::command]
fn save_effect(
    handle: AppHandle,
    app: State<'_, App>,
    old: Option<String>,
    name: String,
    effect: EffectSpec,
) -> Result<SavedEffect, String> {
    let saved = app.store().save_effect(old.as_deref(), &name, effect)?;
    effects_changed(&handle, &app);
    Ok(saved)
}

#[tauri::command]
fn delete_effect(handle: AppHandle, app: State<'_, App>, name: String) -> Result<(), String> {
    app.store().delete_effect(&name)?;
    effects_changed(&handle, &app);
    Ok(())
}

/// Save a kept effect to a file the user picks. `None` when the dialog was cancelled.
#[tauri::command]
async fn export_effect(handle: AppHandle, name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let effect = {
        let app = handle.state::<App>();
        let store = app.store();
        let e = store.effects().iter().find(|e| e.name == name).cloned();
        e.ok_or_else(|| format!("No effect called “{name}”"))?
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    handle
        .dialog()
        .file()
        .set_title("Export effect")
        .add_filter("freeRGB effect", &["json"])
        .set_file_name(effect_file_name(&name))
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(path) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, effect_to_file(&effect))
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// Add an effect from a file the user picks to "My effects". `None` when cancelled.
#[tauri::command]
async fn import_effect(handle: AppHandle) -> Result<Option<SavedEffect>, String> {
    use std::io::Read;
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    handle
        .dialog()
        .file()
        .set_title("Import effect")
        .add_filter("freeRGB effect", &["json"])
        .pick_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(path) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let mut raw = String::new();
    std::fs::File::open(&path)
        .and_then(|f| f.take(MAX_EFFECT_FILE + 1).read_to_string(&mut raw))
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    if raw.len() as u64 > MAX_EFFECT_FILE {
        return Err("This file is too large to be a freeRGB effect".into());
    }
    let app = handle.state::<App>();
    let saved = app.store().import_effect(effect_from_file(&raw)?)?;
    effects_changed(&handle, &app);
    Ok(Some(saved))
}

fn lights_off(handle: &AppHandle, app: &App) -> Result<(), String> {
    app.lights.off()?;
    scenes_changed(handle, app);
    Ok(())
}

#[tauri::command]
fn turn_lights_off(handle: AppHandle, app: State<'_, App>) -> Result<(), String> {
    lights_off(&handle, &app)
}

/// Keep a built-in mode look in the device's own memory.
#[tauri::command]
async fn save_mode(
    handle: AppHandle,
    app: State<'_, App>,
    device: u32,
    effect: EffectSpec,
) -> Result<(), String> {
    app.lights.save_mode(device, &effect).await?;
    let _ = handle.emit("devices", devices_of(app.lights.client().as_ref()));
    Ok(())
}

/// Split a zone into named runs (OpenRGB segments); each becomes its own piece on the desk map.
#[tauri::command]
async fn set_segments(
    handle: AppHandle,
    app: State<'_, App>,
    device: u32,
    zone: u32,
    runs: Vec<SegmentRun>,
) -> Result<(), String> {
    app.lights.set_segments(device, zone, &runs).await?;
    let _ = handle.emit("devices", devices_of(app.lights.client().as_ref()));
    Ok(())
}

/// Change how many LEDs a zone has (strips and similar). Refused for zones the device does not
/// let us resize, and outside the zone's range.
#[tauri::command]
async fn resize_zone(
    handle: AppHandle,
    app: State<'_, App>,
    device: u32,
    zone: u32,
    size: u32,
) -> Result<(), String> {
    app.lights.resize_zone(device, zone, size).await?;
    // Do not wait for the server to announce the change: show the new size now.
    let _ = handle.emit("devices", devices_of(app.lights.client().as_ref()));
    Ok(())
}

/// A scan that never reports its end (a server below protocol 6 sends no detection packets)
/// stops showing as running after this long.
const SCAN_LIMIT: Duration = Duration::from_secs(120);

#[tauri::command]
fn detection(app: State<'_, App>) -> Detection {
    app.detection.lock().map(|d| d.clone()).unwrap_or_default()
}

fn set_detection(handle: &AppHandle, app: &App, next: Detection) {
    let Ok(mut d) = app.detection.lock() else {
        return;
    };
    if next.scanning && !d.scanning {
        app.scans.fetch_add(1, Ordering::Relaxed);
    }
    if *d != next {
        *d = next.clone();
        drop(d);
        let _ = handle.emit("detection", next);
    }
}

fn follow_detection(handle: &AppHandle, app: &App, ev: &Event) {
    let mut next = app.detection.lock().map(|d| d.clone()).unwrap_or_default();
    if next.follow(ev) {
        set_detection(handle, app, next);
    }
}

/// Ask OpenRGB to look for devices again. Shows as scanning at once; the server's detection
/// packets then report progress and the end, and new devices arrive as a device list change.
#[tauri::command]
async fn rescan_devices(handle: AppHandle, app: State<'_, App>) -> Result<(), String> {
    let client = app.lights.client().ok_or("OpenRGB is not connected")?;
    set_detection(&handle, &app, Detection::started());
    let scan = app.scans.load(Ordering::Relaxed);
    if let Err(e) = client.rescan().await {
        set_detection(&handle, &app, Detection::default());
        return Err(e.to_string());
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SCAN_LIMIT).await;
        let app = handle.state::<App>();
        if app.scans.load(Ordering::Relaxed) == scan {
            set_detection(&handle, &app, Detection::default());
        }
    });
    Ok(())
}

// ---- diagnostics ------------------------------------------------------------

/// The window hears about new log lines at most this often.
const LOG_NUDGE: Duration = Duration::from_millis(200);

fn set_log(handle: &AppHandle, app: &App, state: LogState, new_epoch: bool) {
    if let Ok(mut l) = app.log.lock() {
        l.0 = state;
        if new_epoch {
            l.1 += 1;
        }
    }
    let _ = handle.emit("server-log", ());
}

/// A new connection starts its log over: find out whether OpenRGB keeps one, and fetch it.
fn start_log(handle: &AppHandle, client: Option<Client>) {
    let app = handle.state::<App>();
    set_log(handle, &app, LogState::Unavailable, true);
    let Some(c) = client.filter(|c| c.is_local_client()) else {
        return;
    };
    let epoch = app.log.lock().map(|l| l.1).unwrap_or_default();
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let on = c
            .get_settings_json("LogManager")
            .await
            .ok()
            .and_then(|v| v["log_console"].as_bool());
        let state = LogState::at_connect(true, on);
        if state == LogState::On {
            let _ = c.request_log().await;
        }
        let app = handle.state::<App>();
        // A newer connection has started its own log meanwhile.
        if app.log.lock().map(|l| l.1).unwrap_or_default() == epoch {
            set_log(&handle, &app, state, false);
        }
    });
}

/// Tell an open window there are new log lines, at most every `LOG_NUDGE`.
fn nudge_log(handle: &AppHandle) {
    if handle.get_webview_window("main").is_none()
        || handle
            .state::<App>()
            .log_nudge
            .swap(true, Ordering::Relaxed)
    {
        return;
    }
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(LOG_NUDGE).await;
        handle
            .state::<App>()
            .log_nudge
            .store(false, Ordering::Relaxed);
        let _ = handle.emit("server-log", ());
    });
}

fn owns_server(app: &App) -> bool {
    app.supervisor
        .lock()
        .ok()
        .and_then(|s| s.as_ref().map(|s| s.owned_pid().is_some()))
        .unwrap_or(false)
}

/// OpenRGB's log: lines after `after` when the window is on the current `epoch`, else all
/// (a window that has no lines yet sends none).
#[tauri::command]
fn server_log(app: State<'_, App>, epoch: Option<u64>, after: u64) -> LogView {
    let (state, now) = app
        .log
        .lock()
        .map(|l| *l)
        .unwrap_or((LogState::Unavailable, 0));
    let after = if epoch == Some(now) { after } else { 0 };
    let lines = app
        .lights
        .client()
        .map(|c| c.log_since(after))
        .unwrap_or_default()
        .into_iter()
        .map(|(seq, e)| LogLine::new(seq, e))
        .collect();
    LogView {
        epoch: now,
        state,
        owned: owns_server(&app),
        lines,
    }
}

/// Turn on OpenRGB's "Enable Log Console" setting. OpenRGB reads it only when it starts, so
/// one freeRGB started is restarted; any other waits for the user to restart it.
#[tauri::command]
async fn turn_on_server_log(handle: AppHandle, app: State<'_, App>) -> Result<(), String> {
    let c = app.lights.client().ok_or("OpenRGB is not connected")?;
    let on = serde_json::json!({ "log_console": true });
    c.modify_settings("LogManager", &on)
        .await
        .map_err(|e| format!("OpenRGB did not take the setting: {e}"))?;
    c.save_settings()
        .await
        .map_err(|e| format!("OpenRGB did not save the setting: {e}"))?;
    let restarted = app
        .supervisor
        .lock()
        .ok()
        .and_then(|s| s.as_ref().map(|s| s.restart_owned()))
        .unwrap_or(false);
    if !restarted {
        set_log(&handle, &app, LogState::Waiting, false);
    }
    Ok(())
}

#[tauri::command]
async fn clear_server_log(handle: AppHandle, app: State<'_, App>) -> Result<(), String> {
    let c = app.lights.client().ok_or("OpenRGB is not connected")?;
    c.clear_log().await.map_err(|e| e.to_string())?;
    let state = app.log.lock().map(|l| l.0).unwrap_or(LogState::Unavailable);
    set_log(&handle, &app, state, true);
    Ok(())
}

/// What OpenRGB sees on this machine.
#[tauri::command]
async fn hardware(app: State<'_, App>) -> Result<Hardware, String> {
    let c = app.lights.client().ok_or("OpenRGB is not connected")?;
    if !c.is_local_client() {
        return Err("OpenRGB runs on another computer, so it does not share this.".into());
    }
    let (buses, hid, usb, ports, usb_ports) = tokio::try_join!(
        c.i2c_buses(),
        c.hid_devices(),
        c.usb_devices(),
        c.serial_ports(),
        c.usb_serial_ports()
    )
    .map_err(|e| format!("OpenRGB did not answer: {e}"))?;
    Ok(Hardware::new(buses, hid, usb, ports, usb_ports))
}

/// What a look can follow on this machine.
#[tauri::command]
fn sources(app: State<'_, App>) -> Vec<freergb_sources::Offer> {
    app.hub.available()
}

/// The readings now, for a window that just opened (later ones arrive as "readings" events).
#[tauri::command]
fn readings(app: State<'_, App>) -> Readings {
    app.hub.readings().borrow().clone()
}

/// The window's preview of the readings: at most this often.
const READINGS_EVERY: Duration = Duration::from_millis(66);

/// Hand every new reading to the engine, and to the window while it is open (at most
/// `READINGS_EVERY`). Nothing wakes while no source runs.
fn spawn_readings(handle: AppHandle) {
    let mut to_engine = handle.state::<App>().hub.readings();
    let mut to_window = to_engine.clone();
    let h = handle.clone();
    tauri::async_runtime::spawn(async move {
        while to_engine.changed().await.is_ok() {
            let r = to_engine.borrow_and_update().clone();
            h.state::<App>().lights.set_readings(r);
        }
    });
    tauri::async_runtime::spawn(async move {
        while to_window.changed().await.is_ok() {
            let r = to_window.borrow_and_update().clone();
            if handle.get_webview_window("main").is_some() {
                let _ = handle.emit("readings", r);
            }
            tokio::time::sleep(READINGS_EVERY).await;
        }
    });
}

/// Forward supervisor state and device snapshots to the window until the supervisor is gone.
fn spawn_bridges(
    handle: AppHandle,
    mut state: tokio::sync::watch::Receiver<freergb_supervisor::State>,
    mut client: tokio::sync::watch::Receiver<Option<Client>>,
) {
    let h = handle.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let s = ServerState::from(&*state.borrow_and_update());
            let _ = h.emit("server-state", s);
            if state.changed().await.is_err() {
                break;
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        loop {
            let current = client.borrow_and_update().clone();
            handle.state::<App>().lights.attach(current.as_ref());
            // A scan of the previous connection is over either way.
            set_detection(&handle, &handle.state::<App>(), Detection::default());
            start_log(&handle, current.clone());
            let _ = handle.emit("devices", devices_of(current.as_ref()));
            let Some(c) = current else {
                if client.changed().await.is_err() {
                    break;
                }
                continue;
            };
            let mut events = c.events();
            loop {
                tokio::select! {
                    changed = client.changed() => {
                        if changed.is_err() { return; }
                        break;
                    }
                    ev = events.recv() => {
                        if matches!(ev, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                            // Connection is gone; wait for the supervisor to publish the next client.
                            if client.changed().await.is_err() { return; }
                            break;
                        }
                        let app = handle.state::<App>();
                        if let Ok(e) = &ev {
                            follow_detection(&handle, &app, e);
                            if matches!(e, Event::LogUpdated) {
                                nudge_log(&handle);
                            }
                            if matches!(
                                e,
                                Event::DetectionStarted | Event::DetectionProgress { .. } | Event::LogUpdated
                            ) {
                                continue;
                            }
                        }
                        app.lights.devices_changed();
                        // Coalesce bursts (a profile load fires many updates) into one snapshot.
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        while let Ok(e) = events.try_recv() {
                            follow_detection(&handle, &app, &e);
                            if matches!(e, Event::LogUpdated) {
                                nudge_log(&handle);
                            }
                        }
                        let _ = handle.emit("devices", devices_of(Some(&c)));
                    }
                }
            }
        }
    });
}

/// Bring the window up, building it again if it was closed: closing destroys the webview so
/// the tray costs no browser memory.
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == "main") else {
        return;
    };
    // Windows: the UI draws the title bar (WindowFrame.svelte), so it blends with the window.
    let built = tauri::WebviewWindowBuilder::from_config(app, cfg)
        .and_then(|b| b.decorations(!cfg!(windows)).build());
    match built {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => eprintln!("freergb: could not open the window: {e}"),
    }
}

/// Whether the app starts when the user logs in.
#[tauri::command]
fn autostart(handle: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    handle.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(handle: AppHandle, on: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let l = handle.autolaunch();
    if on { l.enable() } else { l.disable() }
        .map_err(|e| format!("Could not change starting at login: {e}"))?;
    l.is_enabled().map_err(|e| e.to_string())
}

/// Start over: everything freeRGB keeps goes, effects stop (devices go back to their own
/// modes) and it no longer starts at login. OpenRGB's own settings are left alone.
#[tauri::command]
fn reset_all(handle: AppHandle, app: State<'_, App>) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.store().reset()?;
    app.lights.set_scenes(vec![])?;
    app.lights.set_fps(app.store().fps());
    app.lights.set_map(vec![]);
    app.lights.set_hidden(vec![]);
    *app.before_sleep.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if let Ok(dir) = handle.path().app_config_dir() {
        let _ = std::fs::remove_file(dir.join("screen-token"));
    }
    let l = handle.autolaunch();
    if l.is_enabled().unwrap_or(false) {
        l.disable()
            .map_err(|e| format!("Could not stop starting at login: {e}"))?;
    }
    scenes_changed(&handle, &app);
    groups_changed(&handle, &app);
    effects_changed(&handle, &app);
    schedule_changed(&handle, &app);
    let _ = handle.emit("device-notes", app.store().device_notes().to_vec());
    Ok(())
}

/// Frames per second of software effects.
#[tauri::command]
fn fps(app: State<'_, App>) -> u32 {
    app.store().fps()
}

#[tauri::command]
fn set_fps(app: State<'_, App>, fps: u32) -> Result<u32, String> {
    app.store().set_fps(fps)?;
    app.lights.set_fps(fps);
    Ok(fps)
}

/// Where each device sits on the desk.
#[tauri::command]
fn desk_map(app: State<'_, App>) -> Vec<Placed> {
    app.store().map().to_vec()
}

#[tauri::command]
fn set_desk_map(app: State<'_, App>, map: Vec<Placed>) -> Result<(), String> {
    app.store().set_map(map.clone())?;
    app.lights.set_map(map);
    Ok(())
}

/// Stop an owned OpenRGB gracefully, then exit. Runs once; later exit requests pass through.
/// `session_end`: the PC is shutting down, restarting or signing out.
fn quit(app: &AppHandle, session_end: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if stop_all(&app, session_end).await {
            app.exit(0);
        }
    });
}

/// Hands devices back and stops an owned OpenRGB, once; false if that is already under way.
pub(crate) async fn shut_down(app: &AppHandle) -> bool {
    stop_all(app, false).await
}

/// Like `shut_down`, but at the end of the session the schedule may turn the lights off and
/// leave them off: devices handed back to their own modes would stay lit with the PC off.
async fn stop_all(app: &AppHandle, session_end: bool) -> bool {
    let state = app.state::<App>();
    {
        let Ok(mut q) = state.quitting.lock() else {
            return false;
        };
        if *q {
            return false;
        }
        *q = true;
    }
    let sup = state.supervisor.lock().ok().and_then(|mut g| g.take());
    let off = session_end && state.store().schedule().shutdown == Action::Off;
    // While the server is still up.
    if off {
        state.lights.shut_off().await;
    } else {
        state.lights.shutdown().await;
    }
    if let Some(s) = sup {
        s.shutdown().await;
    }
    true
}

/// SIGTERM/SIGINT (and Ctrl-C on Windows) take the same graceful path as the tray's Quit.
/// Without this a terminated app leaves its OpenRGB running, because Drop cannot run on a
/// signal death.
fn spawn_signal_handler(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int), Ok(mut hup)) = (
                signal(SignalKind::terminate()),
                signal(SignalKind::interrupt()),
                signal(SignalKind::hangup()),
            ) else {
                return;
            };
            // Shutting down or signing out sends TERM to the session's processes.
            let session_end = tokio::select! {
                _ = term.recv() => true,
                _ = int.recv() => false,
                _ = hup.recv() => false,
            };
            quit(&handle, session_end);
        }
        #[cfg(not(unix))]
        {
            if tokio::signal::ctrl_c().await.is_err() {
                return;
            }
            quit(&handle, false);
        }
    });
}

pub fn run() {
    let app = tauri::Builder::default()
        // First, so a second launch hands over before building anything: it opens our window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::Builder::new().arg(TRAY_ARG).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        // Windows 11 shows its snap layouts when hovering the app-drawn maximise button.
        .plugin(
            tauri_plugin_snap_layout::init()
                .button_id("frame-max")
                .build(),
        )
        .manage(crate::update::Updates::default())
        .manage(crate::about::AboutRequest::default())
        .invoke_handler(tauri::generate_handler![
            server_state,
            schedule,
            set_schedule,
            devices,
            retry_server,
            set_scenes,
            scenes,
            set_device_color,
            resize_zone,
            save_mode,
            set_segments,
            sources,
            readings,
            detection,
            rescan_devices,
            server_log,
            turn_on_server_log,
            clear_server_log,
            hardware,
            profiles,
            store_notice,
            save_profile,
            apply_profile,
            rename_profile,
            delete_profile,
            turn_lights_off,
            autostart,
            set_autostart,
            reset_all,
            fps,
            set_fps,
            desk_map,
            set_desk_map,
            device_notes,
            set_device_note,
            groups,
            save_group,
            delete_group,
            my_effects,
            save_effect,
            delete_effect,
            export_effect,
            import_effect,
            crate::update::update_status,
            crate::update::install_update,
            crate::udev::udev_rules,
            crate::udev::install_udev_rules,
            crate::about::about,
            crate::about::take_about_request,
            crate::about::open_link
        ])
        .setup(|app| {
            // `Supervisor::start` spawns onto the current tokio runtime.
            let sup =
                tauri::async_runtime::block_on(async { Supervisor::start(config_from_env()) });
            // State must exist before the bridges run: they look it up on every client change.
            let (state_rx, client_rx) = (sup.watch_state(), sup.watch_client());
            let (store, store_notice) =
                Store::open(app.path().app_config_dir()?.join("state.json"));
            if let Some(n) = &store_notice {
                eprintln!("freergb: {n}");
            }
            // Bring back the last look; it is applied as soon as OpenRGB connects.
            let lights = Lights::default();
            lights.remember(store.last().to_vec());
            lights.set_fps(store.fps());
            lights.set_map(store.map().to_vec());
            lights.set_hidden(store.hidden());
            let hub = Arc::new(Hub::new(
                freergb_sources::machine::local(Some(
                    app.path().app_config_dir()?.join("screen-token"),
                )),
                tauri::async_runtime::handle().inner().clone(),
            ));
            lights.set_hub(hub.clone());
            app.manage(App {
                supervisor: Mutex::new(Some(sup)),
                lights,
                store: Mutex::new(store),
                store_notice: Mutex::new(store_notice),
                quitting: Mutex::new(false),
                detection: Mutex::new(Detection::default()),
                scans: AtomicU64::new(0),
                hub,
                before_sleep: Mutex::new(None),
                log: Mutex::new((LogState::Unavailable, 0)),
                log_nudge: AtomicBool::new(false),
            });
            spawn_bridges(app.handle().clone(), state_rx, client_rx);
            spawn_readings(app.handle().clone());
            spawn_signal_handler(app.handle().clone());
            spawn_schedule(app.handle().clone());
            crate::update::spawn(app.handle().clone());
            let power = app.handle().clone();
            if !freergb_power::watch(move |p| on_power(&power, p)) {
                eprintln!("freergb: this system does not report sleep and wake");
            }

            let menu = tray_menu(app.handle(), &profiles_view(&app.state::<App>()))?;
            let mut tray = TrayIconBuilder::with_id(TRAY_ID)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| on_tray_menu(app, ev.id().as_ref()))
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if !std::env::args().any(|a| a == TRAY_ARG) {
                show_main(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the freergb app");

    app.run(|handle, ev| match ev {
        RunEvent::ExitRequested { api, code, .. } => {
            // `code` is None when the last window closed: keep running in the tray. Quitting
            // goes through `quit`, whose `app.exit(0)` carries Some.
            if code.is_none() {
                api.prevent_exit();
            }
        }
        // Without a quit first, this is Windows ending the session (WM_ENDSESSION): the
        // process ends as soon as this returns, and Windows waits only a few seconds.
        RunEvent::Exit => {
            let handle = handle.clone();
            tauri::async_runtime::block_on(async move {
                let _ = tokio::time::timeout(SESSION_END_LIMIT, stop_all(&handle, true)).await;
            });
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_openrgb_is_found_beside_the_app_only_when_installed() {
        let dir = std::env::temp_dir().join(format!("freergb-bundled-{}", std::process::id()));
        let exe = dir.join("freeRGB.exe");
        assert_eq!(bundled_openrgb(&exe), None);
        std::fs::create_dir_all(dir.join("openrgb")).unwrap();
        std::fs::write(dir.join("openrgb").join("OpenRGB.exe"), b"").unwrap();
        assert_eq!(
            bundled_openrgb(&exe),
            Some(dir.join("openrgb").join("OpenRGB.exe"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_distro_openrgb_is_found_on_the_path() {
        let dir = std::env::temp_dir().join(format!("freergb-path-{}", std::process::id()));
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(b.join("openrgb")).unwrap();
        let path = std::env::join_paths([&b, &a]).unwrap();
        assert_eq!(
            system_openrgb(&path),
            None,
            "a directory is not the program"
        );
        std::fs::write(a.join("openrgb"), b"").unwrap();
        assert_eq!(system_openrgb(&path), Some(a.join("openrgb")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
