// SPDX-License-Identifier: GPL-2.0-or-later
//! Saved looks: the user's profiles and the last look, in one JSON file in the app's config
//! folder. Our own format rather than OpenRGB's profiles, because those cannot hold software
//! effects or painted LEDs. Scenes name devices by `DeviceKey`, so a file stays valid across
//! OpenRGB restarts and reboots.
//!
//! Every change is written at once (temp file + rename, so a crash never leaves half a file).
//! A file we cannot read is moved aside, never overwritten, so a newer version's file or a
//! hand edit gone wrong is not lost.

use crate::schedule::Schedule;
use freergb_effects::layout::Placed;
use freergb_effects::spec::{EffectKind, EffectSpec, Scene, Targets};
use freergb_sdk::DeviceKey;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;
pub const MAX_NAME: usize = 40;
const MAX_PLACED: usize = 256;
const MAX_POINTS: usize = 4096;
const MAX_LENGTH: u32 = 300;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub scenes: Vec<Scene>,
}

/// Frame rates the app offers; the spec asks for 30 by default and 60 as an option.
pub const FPS_CHOICES: [u32; 2] = [30, 60];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_fps")]
    fps: u32,
}

fn default_fps() -> u32 {
    FPS_CHOICES[0]
}

impl Default for Settings {
    fn default() -> Self {
        Settings { fps: default_fps() }
    }
}

/// A look the user kept under a name ("My effects"): a software effect without devices, so it
/// can be put on any target and shared as a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedEffect {
    pub name: String,
    pub effect: EffectSpec,
}

/// Most saved effects kept; the shelf shows them all.
pub const MAX_EFFECTS: usize = 48;

/// A saved effect, if it can be one: valid, and not tied to one device (a painting or a
/// device's own mode).
fn check_effect(e: &EffectSpec) -> Result<(), String> {
    if matches!(e.kind, EffectKind::Painted | EffectKind::DeviceMode) {
        return Err(
            "Paintings and built-in modes belong to one device and cannot be saved here".into(),
        );
    }
    e.validate().map_err(|e| e.to_string())
}

/// The shared file: `{"freergb_effect": 1, "name": .., "effect": {..}}`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectFile {
    freergb_effect: u32,
    name: String,
    effect: EffectSpec,
}

/// Larger than any valid effect file; a guard against opening the wrong file.
pub const MAX_EFFECT_FILE: u64 = 64 * 1024;

pub fn effect_to_file(e: &SavedEffect) -> String {
    let f = EffectFile {
        freergb_effect: 1,
        name: e.name.clone(),
        effect: e.effect.clone(),
    };
    serde_json::to_string_pretty(&f).unwrap_or_default() + "\n"
}

/// A file name for `name` that every OS accepts.
pub fn effect_file_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim();
    format!("{}.json", if safe.is_empty() { "effect" } else { safe })
}

pub fn effect_from_file(raw: &str) -> Result<SavedEffect, String> {
    let f: EffectFile = serde_json::from_str(raw)
        .map_err(|e| format!("This is not a freeRGB effect file ({e})"))?;
    if f.freergb_effect != 1 {
        return Err("This effect file is from a newer version of freeRGB".into());
    }
    check_effect(&f.effect)?;
    Ok(SavedEffect {
        name: clean_name_of("effect", &f.name)?,
        effect: f.effect,
    })
}

/// Devices that share one canvas, left to right in this order. A group may have a look of its
/// own (a scene labelled with its name); without one its devices follow the all-devices look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub devices: Vec<DeviceKey>,
}

/// The user's own name for a device, and whether freeRGB leaves it alone (out of the window and
/// of every effect). Kept here rather than in OpenRGB: OpenRGB 1.1 renames only its debug
/// devices and does not save the hidden flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceNote {
    pub device: DeviceKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}

/// `scenes` after group `old` became `new` (renamed and/or with other devices), or was deleted
/// (`None`): its look follows the new name and members, or is dropped.
pub fn regroup(scenes: &[Scene], old: &str, new: Option<&Group>) -> Vec<Scene> {
    scenes
        .iter()
        .filter_map(|s| {
            if s.group.as_deref() != Some(old) {
                return Some(s.clone());
            }
            let g = new?;
            Some(Scene {
                targets: Targets::Devices(g.devices.clone()),
                group: Some(g.name.clone()),
                ..s.clone()
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Data {
    version: u32,
    /// What was running when the app last changed the lights; restored at startup.
    #[serde(default)]
    last: Vec<Scene>,
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(default)]
    groups: Vec<Group>,
    #[serde(default)]
    effects: Vec<SavedEffect>,
    /// Where each device sits on the desk. Physical, so profiles do not change it.
    #[serde(default)]
    map: Vec<Placed>,
    /// Names and hidden flags; only devices with one of them have an entry.
    #[serde(default)]
    devices: Vec<DeviceNote>,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    schedule: Schedule,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            version: VERSION,
            last: vec![],
            profiles: vec![],
            groups: vec![],
            effects: vec![],
            map: vec![],
            devices: vec![],
            settings: Settings::default(),
            schedule: Schedule::default(),
        }
    }
}

pub struct Store {
    path: PathBuf,
    data: Data,
    /// The file exists but could not be read (permissions, a lock): writing would replace it.
    read_only: bool,
}

fn clean_name(name: &str) -> Result<String, String> {
    clean_name_of("profile", name)
}

fn clean_name_of(what: &str, name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(format!("A {what} needs a name"));
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("Keep the name under {MAX_NAME} characters"));
    }
    if name.chars().any(char::is_control) {
        return Err("The name has characters that cannot be shown".into());
    }
    Ok(name.to_string())
}

impl Store {
    /// Load the file, or start empty. The message says what went wrong with an unreadable file
    /// and where it was moved.
    pub fn open(path: impl Into<PathBuf>) -> (Store, Option<String>) {
        let path = path.into();
        let mut read_only = false;
        let (data, warning) = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Data::default(), None),
            Err(e) => {
                read_only = true;
                let msg = format!(
                    "Could not read saved profiles ({e}); changes will not be saved this session"
                );
                (Data::default(), Some(msg))
            }
            Ok(raw) => match parse(&raw) {
                Ok(d) => (d, None),
                Err(e) => {
                    let aside = path.with_extension("json.unreadable");
                    let moved = std::fs::rename(&path, &aside).is_ok();
                    let msg = if moved {
                        format!(
                            "Saved profiles could not be read ({e}); the file was kept as {}",
                            aside.display()
                        )
                    } else {
                        format!("Saved profiles could not be read ({e})")
                    };
                    (Data::default(), Some(msg))
                }
            },
        };
        (
            Store {
                path,
                data,
                read_only,
            },
            warning,
        )
    }

    pub fn last(&self) -> &[Scene] {
        &self.data.last
    }

    /// Remember the current look. Unchanged looks are not rewritten (sliders send many edits).
    pub fn set_last(&mut self, scenes: &[Scene]) -> Result<(), String> {
        if self.data.last == scenes {
            return Ok(());
        }
        self.change(|d| {
            d.last = scenes.to_vec();
            Ok(())
        })
    }

    pub fn fps(&self) -> u32 {
        self.data.settings.fps
    }

    pub fn set_fps(&mut self, fps: u32) -> Result<(), String> {
        if !FPS_CHOICES.contains(&fps) {
            return Err(format!("{fps} frames per second is not offered"));
        }
        self.change(|d| {
            d.settings.fps = fps;
            Ok(())
        })
    }

    pub fn map(&self) -> &[Placed] {
        &self.data.map
    }

    pub fn device_notes(&self) -> &[DeviceNote] {
        &self.data.devices
    }

    /// The devices freeRGB leaves alone.
    pub fn hidden(&self) -> Vec<DeviceKey> {
        self.data
            .devices
            .iter()
            .filter(|n| n.hidden)
            .map(|n| n.device.clone())
            .collect()
    }

    /// Give a device a name of the user's (`None` or blank: its own again) and hide or show
    /// it. A device back to its defaults loses its entry.
    pub fn set_device_note(
        &mut self,
        device: DeviceKey,
        name: Option<&str>,
        hidden: bool,
    ) -> Result<(), String> {
        let name = match name.map(str::trim) {
            None | Some("") => None,
            Some(n) => Some(clean_name_of("device", n)?),
        };
        let mut notes: Vec<DeviceNote> = self
            .data
            .devices
            .iter()
            .filter(|n| n.device != device)
            .cloned()
            .collect();
        if name.is_some() || hidden {
            if notes.len() >= MAX_PLACED {
                return Err(format!(
                    "At most {MAX_PLACED} devices can be renamed or hidden"
                ));
            }
            notes.push(DeviceNote {
                device,
                name,
                hidden,
            });
        }
        if notes == self.data.devices {
            return Ok(());
        }
        self.change(|d| {
            d.devices = notes;
            Ok(())
        })
    }

    /// Replace the desk map: one entry per device, sizes above zero.
    pub fn set_map(&mut self, map: Vec<Placed>) -> Result<(), String> {
        if map.len() > MAX_PLACED {
            return Err(format!("The map holds at most {MAX_PLACED} devices"));
        }
        if map.iter().any(|p| p.at.is_some_and(|a| !a.is_valid())) {
            return Err("A device on the map needs a finite position and a size above zero".into());
        }
        let bad_points = |p: &Placed| {
            p.points.as_ref().is_some_and(|pts| {
                pts.len() > MAX_POINTS
                    || pts
                        .iter()
                        .flatten()
                        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            })
        };
        if map.iter().any(bad_points) {
            return Err("A device's LED spots must lie in its own square".into());
        }
        if map.iter().any(|p| {
            p.shape.as_ref().is_some_and(|s| s.len() > MAX_NAME)
                || p.length.is_some_and(|n| n == 0 || n > MAX_LENGTH)
        }) {
            return Err("A device's drawing on the map is not one the app makes".into());
        }
        if map.iter().enumerate().any(|(i, p)| {
            map[..i]
                .iter()
                .any(|q| q.device == p.device && q.segment == p.segment)
        }) {
            return Err("A device can only sit in one place on the map".into());
        }
        if self.data.map == map {
            return Ok(());
        }
        self.change(|d| {
            d.map = map;
            Ok(())
        })
    }

    pub fn effects(&self) -> &[SavedEffect] {
        &self.data.effects
    }

    /// Keep `effect` under `name`, replacing a saved effect of that name in place. With `old`,
    /// that saved effect is the one being renamed or replaced.
    pub fn save_effect(
        &mut self,
        old: Option<&str>,
        name: &str,
        effect: EffectSpec,
    ) -> Result<SavedEffect, String> {
        check_effect(&effect)?;
        let saved = SavedEffect {
            name: clean_name_of("effect", name)?,
            effect,
        };
        let e = saved.clone();
        self.change(move |d| {
            if let Some(o) = old {
                if o != e.name && d.effects.iter().any(|x| x.name == e.name) {
                    return Err(format!("There is already an effect called “{}”", e.name));
                }
                let x = d
                    .effects
                    .iter_mut()
                    .find(|x| x.name == o)
                    .ok_or_else(|| format!("No effect called “{o}”"))?;
                *x = e;
                return Ok(());
            }
            let full = d.effects.len() >= MAX_EFFECTS;
            match d.effects.iter_mut().find(|x| x.name == e.name) {
                Some(x) => *x = e,
                None if full => {
                    return Err(format!("My effects holds {MAX_EFFECTS}; delete one first"))
                }
                None => d.effects.push(e),
            }
            Ok(())
        })?;
        Ok(saved)
    }

    /// Add an imported effect, numbering its name ("Fire 2") if that name is taken.
    pub fn import_effect(&mut self, e: SavedEffect) -> Result<SavedEffect, String> {
        let mut name = e.name.clone();
        let mut n = 2;
        while self.data.effects.iter().any(|x| x.name == name) {
            let suffix = format!(" {n}");
            let keep: String = e
                .name
                .chars()
                .take(MAX_NAME - suffix.chars().count())
                .collect();
            name = format!("{keep}{suffix}");
            n += 1;
        }
        self.save_effect(None, &name, e.effect)
    }

    pub fn delete_effect(&mut self, name: &str) -> Result<(), String> {
        self.change(|d| {
            let before = d.effects.len();
            d.effects.retain(|e| e.name != name);
            if d.effects.len() == before {
                return Err(format!("No effect called “{name}”"));
            }
            Ok(())
        })
    }

    pub fn groups(&self) -> &[Group] {
        &self.data.groups
    }

    /// Create a group, or change group `old` (its name, devices or their order). Its look, in
    /// the last look and in every profile, follows. A device belongs to one group at most.
    pub fn save_group(
        &mut self,
        old: Option<&str>,
        name: &str,
        devices: Vec<DeviceKey>,
    ) -> Result<Group, String> {
        let name = clean_name_of("group", name)?;
        if devices.len() < 2 {
            return Err("A group needs at least two devices".into());
        }
        if devices
            .iter()
            .enumerate()
            .any(|(i, k)| devices[..i].contains(k))
        {
            return Err("A device can only be in a group once".into());
        }
        let group = Group { name, devices };
        let g = group.clone();
        self.change(move |d| {
            if let Some(o) = old {
                if !d.groups.iter().any(|x| x.name == o) {
                    return Err(format!("No group called “{o}”"));
                }
            }
            for other in d.groups.iter().filter(|x| Some(x.name.as_str()) != old) {
                if other.name == g.name {
                    return Err(format!("There is already a group called “{}”", g.name));
                }
                if let Some(k) = g.devices.iter().find(|k| other.devices.contains(k)) {
                    return Err(format!("{} is already in “{}”", k.name, other.name));
                }
            }
            match old.and_then(|o| d.groups.iter_mut().find(|x| x.name == o)) {
                Some(x) => *x = g.clone(),
                None => d.groups.push(g.clone()),
            }
            let from = old.unwrap_or(&g.name).to_string();
            d.last = regroup(&d.last, &from, Some(&g));
            for p in &mut d.profiles {
                p.scenes = regroup(&p.scenes, &from, Some(&g));
            }
            Ok(())
        })?;
        Ok(group)
    }

    /// Delete a group; its look goes from the last look and from every profile, so its devices
    /// follow the all-devices look again.
    pub fn delete_group(&mut self, name: &str) -> Result<(), String> {
        self.change(|d| {
            let before = d.groups.len();
            d.groups.retain(|g| g.name != name);
            if d.groups.len() == before {
                return Err(format!("No group called “{name}”"));
            }
            d.last = regroup(&d.last, name, None);
            for p in &mut d.profiles {
                p.scenes = regroup(&p.scenes, name, None);
            }
            Ok(())
        })
    }

    pub fn profiles(&self) -> &[Profile] {
        &self.data.profiles
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.data.profiles.iter().find(|p| p.name == name)
    }

    /// The profile whose look is the one running now, if any.
    pub fn active(&self, current: &[Scene]) -> Option<&str> {
        self.data
            .profiles
            .iter()
            .find(|p| p.scenes == current)
            .map(|p| p.name.as_str())
    }

    /// Save under this name, replacing a profile of the same name in place.
    pub fn save_profile(&mut self, name: &str, scenes: &[Scene]) -> Result<String, String> {
        let name = clean_name(name)?;
        let n = name.clone();
        self.change(move |d| {
            let p = Profile {
                name: n,
                scenes: scenes.to_vec(),
            };
            match d.profiles.iter_mut().find(|q| q.name == p.name) {
                Some(q) => *q = p,
                None => d.profiles.push(p),
            }
            Ok(())
        })?;
        Ok(name)
    }

    pub fn rename_profile(&mut self, from: &str, to: &str) -> Result<String, String> {
        let to = clean_name(to)?;
        let t = to.clone();
        self.change(move |d| {
            if t != from && d.profiles.iter().any(|p| p.name == t) {
                return Err(format!("There is already a profile called “{t}”"));
            }
            let p = d
                .profiles
                .iter_mut()
                .find(|p| p.name == from)
                .ok_or_else(|| format!("No profile called “{from}”"))?;
            p.name = t.clone();
            d.schedule.renamed(from, &t);
            Ok(())
        })?;
        Ok(to)
    }

    pub fn delete_profile(&mut self, name: &str) -> Result<(), String> {
        self.change(|d| {
            let before = d.profiles.len();
            d.profiles.retain(|p| p.name != name);
            if d.profiles.len() == before {
                return Err(format!("No profile called “{name}”"));
            }
            d.schedule.deleted(name);
            Ok(())
        })
    }

    pub fn schedule(&self) -> &Schedule {
        &self.data.schedule
    }

    pub fn set_schedule(&mut self, schedule: Schedule) -> Result<(), String> {
        let names: Vec<&str> = self.data.profiles.iter().map(|p| p.name.as_str()).collect();
        schedule.check(&names)?;
        self.change(|d| {
            d.schedule = schedule;
            Ok(())
        })
    }

    /// Apply `f` to a copy and write it; memory changes only if the write succeeds, so what
    /// the UI shows never runs ahead of what is on disk.
    /// Forget everything kept: profiles, the last look, groups, My effects, the desk map,
    /// device names and hidden devices, settings and the schedule.
    pub fn reset(&mut self) -> Result<(), String> {
        self.change(|d| {
            *d = Data::default();
            Ok(())
        })
    }

    fn change(&mut self, f: impl FnOnce(&mut Data) -> Result<(), String>) -> Result<(), String> {
        let mut next = self.data.clone();
        f(&mut next)?;
        if self.read_only {
            return Err("Could not save: the profiles file could not be read at startup".into());
        }
        write(&self.path, &next).map_err(|e| format!("Could not save: {e}"))?;
        self.data = next;
        Ok(())
    }
}

fn parse(raw: &str) -> Result<Data, String> {
    let mut d: Data = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if d.version != VERSION {
        return Err(format!("unknown version {}", d.version));
    }
    for e in &d.effects {
        check_effect(&e.effect)?;
    }
    // A hand-edited frame rate is not worth setting the profiles aside for.
    if !FPS_CHOICES.contains(&d.settings.fps) {
        d.settings.fps = default_fps();
    }
    for s in d
        .last
        .iter()
        .chain(d.profiles.iter().flat_map(|p| &p.scenes))
    {
        s.validate().map_err(|e| e.to_string())?;
    }
    Ok(d)
}

fn write(path: &Path, data: &Data) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(data).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Rgb, Targets};
    use freergb_sdk::DeviceKey;

    /// A fresh directory per test, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            // Only path-safe characters: Windows rejects the `{:?}` of a SystemTime.
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let p = std::env::temp_dir().join(format!(
                "freergb-store-{tag}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn file(&self) -> PathBuf {
            self.0.join("sub").join("state.json")
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn static_on(name: &str, r: u8) -> Scene {
        Scene {
            effect: EffectSpec {
                kind: EffectKind::Static,
                colors: vec![Rgb { r, g: 0, b: 0 }],
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
            targets: Targets::Devices(vec![DeviceKey {
                name: name.into(),
                location: "loc".into(),
                serial: String::new(),
                nth: 0,
            }]),
            canvas: Canvas::PerDevice,
            group: None,
        }
    }

    #[test]
    fn a_missing_file_is_an_empty_store_and_changes_survive_a_reopen() {
        let dir = TempDir::new("roundtrip");
        let (mut s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert!(s.last().is_empty() && s.profiles().is_empty());

        s.set_last(&[static_on("kbd", 1)]).unwrap();
        s.save_profile("  Evening ", &[static_on("kbd", 2)])
            .unwrap();
        s.save_profile("Gaming", &[]).unwrap();

        let (s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert_eq!(s.last(), [static_on("kbd", 1)]);
        let names: Vec<_> = s.profiles().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Evening", "Gaming"], "trimmed, in save order");
        assert!(!dir.file().with_extension("json.tmp").exists());
    }

    #[test]
    fn a_reset_forgets_everything_kept_and_stays_forgotten() {
        let dir = TempDir::new("reset");
        let (mut s, _) = Store::open(dir.file());
        s.set_last(&[static_on("kbd", 1)]).unwrap();
        s.save_profile("Evening", &[static_on("kbd", 2)]).unwrap();
        s.set_fps(60).unwrap();
        s.set_device_note(static_key("kbd"), Some("Desk"), true)
            .unwrap();
        s.reset().unwrap();
        let (s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert_eq!(s.data, Data::default());
    }

    fn static_key(name: &str) -> DeviceKey {
        DeviceKey {
            name: name.into(),
            location: "loc".into(),
            serial: String::new(),
            nth: 0,
        }
    }

    #[test]
    fn the_schedule_is_kept_and_follows_its_profiles() {
        use crate::schedule::{Action, Rule};
        let dir = TempDir::new("schedule");
        let (mut s, _) = Store::open(dir.file());
        assert_eq!(s.schedule(), &Schedule::default());
        let work = Action::Profile {
            name: "Work".into(),
        };
        let with = |a: Action| Schedule {
            rules: vec![Rule {
                at: 480,
                days: 0b1_1111,
                action: a,
            }],
            ..Default::default()
        };
        assert!(
            s.set_schedule(with(work.clone())).is_err(),
            "no such profile yet"
        );
        s.save_profile("Work", &[]).unwrap();
        s.set_schedule(with(work)).unwrap();
        assert_eq!(Store::open(dir.file()).0.schedule().rules.len(), 1);

        s.rename_profile("Work", "Desk").unwrap();
        assert_eq!(
            s.schedule().rules[0].action,
            Action::Profile {
                name: "Desk".into()
            }
        );
        s.delete_profile("Desk").unwrap();
        assert!(Store::open(dir.file()).0.schedule().rules.is_empty());
    }

    #[test]
    fn device_names_and_hidden_flags_are_kept_until_back_to_defaults() {
        let dir = TempDir::new("notes");
        let (mut s, _) = Store::open(dir.file());
        s.set_device_note(key("Strip"), Some("  Desk edge "), false)
            .unwrap();
        s.set_device_note(key("Fan"), None, true).unwrap();
        let back = Store::open(dir.file()).0;
        assert_eq!(back.device_notes().len(), 2);
        assert_eq!(back.device_notes()[0].name.as_deref(), Some("Desk edge"));
        assert_eq!(back.hidden(), vec![key("Fan")]);

        // Renaming again replaces the entry; a blank name and shown again drops it.
        s.set_device_note(key("Strip"), Some("Shelf"), false)
            .unwrap();
        assert_eq!(s.device_notes().len(), 2);
        s.set_device_note(key("Fan"), Some(" "), false).unwrap();
        assert_eq!(s.device_notes().len(), 1);
        assert!(s.hidden().is_empty());

        assert!(s
            .set_device_note(key("Strip"), Some(&"x".repeat(MAX_NAME + 1)), false)
            .is_err());
        assert!(s
            .set_device_note(key("Strip"), Some("a\nb"), false)
            .is_err());
        assert_eq!(s.device_notes()[0].name.as_deref(), Some("Shelf"));
    }

    #[test]
    fn the_desk_map_is_saved_and_rejects_bad_entries() {
        use freergb_effects::layout::Placement;
        let dir = TempDir::new("map");
        let (mut s, _) = Store::open(dir.file());
        assert!(s.map().is_empty());
        let at = Placement {
            x: 1.0,
            y: 2.0,
            w: 30.0,
            h: 2.0,
            rot: 90.0,
        };
        let placed = |name: &str, at: Placement| Placed {
            device: key(name),
            at: Some(at),
            points: Some(vec![[0.0, 0.5], [1.0, 0.5]]),
            shape: Some("strip".into()),
            length: Some(12),
            segment: None,
        };
        s.set_map(vec![placed("Strip", at), placed("Fan", at)])
            .unwrap();
        assert_eq!(Store::open(dir.file()).0.map().len(), 2);

        let zero = Placement { w: 0.0, ..at };
        assert!(s.set_map(vec![placed("Strip", zero)]).is_err());
        let nan = Placement { x: f64::NAN, ..at };
        assert!(s.set_map(vec![placed("Strip", nan)]).is_err());
        assert!(s
            .set_map(vec![placed("Strip", at), placed("Strip", at)])
            .is_err());
        let mut off = placed("Strip", at);
        off.points = Some(vec![[1.5, 0.5]]);
        assert!(s.set_map(vec![off]).is_err());
        let mut long = placed("Strip", at);
        long.length = Some(0);
        assert!(s.set_map(vec![long]).is_err());
        assert_eq!(s.map().len(), 2, "a refused map changes nothing");
        // Not placed yet, but drawn as a ring: fine.
        let mut ring = placed("Ring", at);
        ring.at = None;
        s.set_map(vec![ring]).unwrap();
        // A strip and each of its segments have their own entries.
        let mut shelf = placed("Strip", at);
        shelf.segment = Some(freergb_effects::layout::SegmentKey { zone: 0, start: 10 });
        s.set_map(vec![placed("Strip", at), shelf.clone()]).unwrap();
        assert!(s.set_map(vec![shelf.clone(), shelf]).is_err());
    }

    #[test]
    fn the_frame_rate_defaults_to_30_is_saved_and_a_bad_one_falls_back() {
        let dir = TempDir::new("fps");
        let (mut s, _) = Store::open(dir.file());
        assert_eq!(s.fps(), 30);
        assert!(s.set_fps(45).is_err());
        s.set_fps(60).unwrap();
        s.save_profile("Kept", &[]).unwrap();
        assert_eq!(Store::open(dir.file()).0.fps(), 60);

        // A file from before the setting existed reads as 30.
        std::fs::write(
            dir.file(),
            r#"{"version":1,"profiles":[{"name":"Old","scenes":[]}]}"#,
        )
        .unwrap();
        let (s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert_eq!((s.fps(), s.profiles().len()), (30, 1));

        // A hand-edited rate falls back without losing the profiles.
        std::fs::write(
            dir.file(),
            r#"{"version":1,"profiles":[{"name":"Old","scenes":[]}],"settings":{"fps":7}}"#,
        )
        .unwrap();
        let (s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert_eq!((s.fps(), s.profiles().len()), (30, 1));
    }

    fn key(name: &str) -> DeviceKey {
        DeviceKey {
            name: name.into(),
            location: "loc".into(),
            serial: String::new(),
            nth: 0,
        }
    }

    fn group_look(group: &str, devices: &[&str]) -> Scene {
        Scene {
            targets: Targets::Devices(devices.iter().map(|d| key(d)).collect()),
            canvas: Canvas::Shared,
            group: Some(group.into()),
            ..static_on("x", 5)
        }
    }

    #[test]
    fn groups_are_saved_need_two_distinct_devices_and_a_device_joins_one_group_only() {
        let dir = TempDir::new("groups");
        let (mut s, _) = Store::open(dir.file());
        assert!(s.save_group(None, "Desk", vec![key("a")]).is_err());
        assert!(s
            .save_group(None, "Desk", vec![key("a"), key("a")])
            .is_err());
        assert!(s.save_group(None, "  ", vec![key("a"), key("b")]).is_err());
        s.save_group(None, " Desk ", vec![key("b"), key("a")])
            .unwrap();
        let err = s
            .save_group(None, "Shelf", vec![key("c"), key("a")])
            .unwrap_err();
        assert!(err.contains("already in “Desk”"), "{err}");
        assert!(
            s.save_group(None, "Desk", vec![key("c"), key("d")])
                .is_err(),
            "name taken"
        );
        s.save_group(None, "Shelf", vec![key("c"), key("d")])
            .unwrap();
        assert!(s
            .save_group(Some("Nope"), "X", vec![key("e"), key("f")])
            .is_err());

        let (s, _) = Store::open(dir.file());
        let g: Vec<_> = s
            .groups()
            .iter()
            .map(|g| (g.name.as_str(), g.devices.len()))
            .collect();
        assert_eq!(g, [("Desk", 2), ("Shelf", 2)]);
        assert_eq!(s.groups()[0].devices, [key("b"), key("a")], "order is kept");
    }

    #[test]
    fn changing_a_group_moves_its_look_everywhere_and_deleting_it_drops_the_look() {
        let dir = TempDir::new("regroup");
        let (mut s, _) = Store::open(dir.file());
        s.save_group(None, "Desk", vec![key("a"), key("b")])
            .unwrap();
        let other = static_on("z", 1);
        s.set_last(&[group_look("Desk", &["a", "b"]), other.clone()])
            .unwrap();
        s.save_profile("Night", &[group_look("Desk", &["a", "b"])])
            .unwrap();

        s.save_group(
            Some("Desk"),
            "Desk strip",
            vec![key("b"), key("a"), key("c")],
        )
        .unwrap();
        let moved = group_look("Desk strip", &["b", "a", "c"]);
        assert_eq!(s.last(), [moved.clone(), other.clone()]);
        assert_eq!(s.profile("Night").unwrap().scenes, [moved]);
        // Editing a group can make the running look match a profile again: plain equality.

        s.delete_group("Desk strip").unwrap();
        assert!(s.groups().is_empty());
        assert_eq!(s.last(), [other]);
        assert!(s.profile("Night").unwrap().scenes.is_empty());
        assert!(s.delete_group("Desk strip").is_err());
    }

    #[test]
    fn regroup_leaves_other_scenes_alone() {
        let plain = static_on("a", 1);
        let scenes = [plain.clone(), group_look("Other", &["x", "y"])];
        assert_eq!(regroup(&scenes, "Desk", None), scenes);
    }

    fn fire() -> EffectSpec {
        serde_json::from_str(r##"{"type":"fire","colors":["#ff0000","#ffff00"],"speed":0.4}"##)
            .unwrap()
    }

    #[test]
    fn saved_effects_keep_software_looks_rename_and_replace_and_survive_a_reopen() {
        let dir = TempDir::new("effects");
        let (mut s, _) = Store::open(dir.file());
        s.save_effect(None, " Embers ", fire()).unwrap();
        let painted = EffectSpec {
            kind: EffectKind::Painted,
            ..fire()
        };
        assert!(s.save_effect(None, "Paint", painted).is_err());
        let faster = EffectSpec {
            speed: 2.0,
            ..fire()
        };
        s.save_effect(None, "Embers", faster.clone()).unwrap();
        assert_eq!(s.effects().len(), 1, "same name replaces");
        s.save_effect(None, "Other", fire()).unwrap();
        assert!(
            s.save_effect(Some("Embers"), "Other", fire()).is_err(),
            "rename onto a taken name"
        );
        s.save_effect(Some("Embers"), "Hearth", faster.clone())
            .unwrap();
        s.delete_effect("Other").unwrap();
        assert!(s.delete_effect("Other").is_err());

        let (s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert_eq!(
            s.effects(),
            [SavedEffect {
                name: "Hearth".into(),
                effect: faster
            }]
        );
    }

    #[test]
    fn an_effect_file_round_trips_and_imports_under_a_free_name() {
        let dir = TempDir::new("effect-file");
        let (mut s, _) = Store::open(dir.file());
        let e = SavedEffect {
            name: "Embers".into(),
            effect: fire(),
        };
        let raw = effect_to_file(&e);
        assert!(raw.contains(r#""freergb_effect": 1"#), "{raw}");
        assert_eq!(effect_from_file(&raw).unwrap(), e);
        s.import_effect(effect_from_file(&raw).unwrap()).unwrap();
        let second = s.import_effect(effect_from_file(&raw).unwrap()).unwrap();
        assert_eq!(second.name, "Embers 2");
        let long = SavedEffect {
            name: "x".repeat(MAX_NAME),
            effect: fire(),
        };
        s.import_effect(long.clone()).unwrap();
        let numbered = s.import_effect(long).unwrap();
        assert_eq!(
            numbered.name.chars().count(),
            MAX_NAME,
            "a long name is cut to fit the number"
        );
        assert!(numbered.name.ends_with(" 2"));
    }

    #[test]
    fn an_effect_file_name_is_safe_on_every_os() {
        assert_eq!(effect_file_name("Embers"), "Embers.json");
        assert_eq!(effect_file_name("a/b:c*?"), "a_b_c__.json");
        assert_eq!(effect_file_name("  "), "effect.json");
    }

    #[test]
    fn a_bad_effect_file_says_what_is_wrong() {
        assert!(effect_from_file("{")
            .unwrap_err()
            .contains("not a freeRGB effect file"));
        let newer =
            r##"{"freergb_effect":2,"name":"x","effect":{"type":"fire","colors":["#ff0000"]}}"##;
        assert!(effect_from_file(newer)
            .unwrap_err()
            .contains("newer version"));
        let painted =
            r##"{"freergb_effect":1,"name":"x","effect":{"type":"painted","colors":["#ff0000"]}}"##;
        assert!(effect_from_file(painted)
            .unwrap_err()
            .contains("one device"));
        let typo =
            r##"{"freergb_effect":1,"name":"x","effect":{"type":"fire","colours":["#ff0000"]}}"##;
        assert!(effect_from_file(typo).is_err());
        let unnamed = r##"{"freergb_effect":1,"name":" ","effect":{"type":"rainbow"}}"##;
        assert!(effect_from_file(unnamed).is_err());
    }

    #[test]
    fn saving_an_existing_name_replaces_it_in_place() {
        let dir = TempDir::new("replace");
        let (mut s, _) = Store::open(dir.file());
        s.save_profile("A", &[static_on("x", 1)]).unwrap();
        s.save_profile("B", &[]).unwrap();
        s.save_profile("A", &[static_on("x", 9)]).unwrap();
        assert_eq!(s.profiles().len(), 2);
        assert_eq!(s.profiles()[0].name, "A");
        assert_eq!(s.profile("A").unwrap().scenes, [static_on("x", 9)]);
    }

    #[test]
    fn names_are_checked() {
        let dir = TempDir::new("names");
        let (mut s, _) = Store::open(dir.file());
        assert!(s.save_profile("   ", &[]).is_err());
        assert!(s.save_profile(&"x".repeat(MAX_NAME + 1), &[]).is_err());
        assert!(s.save_profile("a\nb", &[]).is_err());
        assert!(s.save_profile(&"é".repeat(MAX_NAME), &[]).is_ok());
        assert_eq!(s.profiles().len(), 1);
    }

    #[test]
    fn rename_and_delete() {
        let dir = TempDir::new("rename");
        let (mut s, _) = Store::open(dir.file());
        s.save_profile("A", &[]).unwrap();
        s.save_profile("B", &[]).unwrap();
        assert!(s.rename_profile("A", "B").is_err(), "name taken");
        assert!(s.rename_profile("Z", "Y").is_err(), "no such profile");
        assert_eq!(
            s.rename_profile("A", " A ").unwrap(),
            "A",
            "same name is fine"
        );
        assert_eq!(s.rename_profile("A", "C").unwrap(), "C");
        assert!(s.delete_profile("A").is_err());
        s.delete_profile("B").unwrap();
        let (s, _) = Store::open(dir.file());
        let names: Vec<_> = s.profiles().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["C"]);
    }

    #[test]
    fn the_active_profile_is_the_one_whose_look_is_running() {
        let dir = TempDir::new("active");
        let (mut s, _) = Store::open(dir.file());
        s.save_profile("Red", &[static_on("x", 255)]).unwrap();
        s.save_profile("Off", &[]).unwrap();
        assert_eq!(s.active(&[static_on("x", 255)]), Some("Red"));
        assert_eq!(s.active(&[]), Some("Off"));
        assert_eq!(s.active(&[static_on("x", 254)]), None, "edited look");
    }

    #[test]
    fn an_unreadable_file_is_moved_aside_not_overwritten() {
        let dir = TempDir::new("bad");
        std::fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        for bad in [
            "{not json",
            r#"{"version":2,"last":[],"profiles":[]}"#,
            r#"{"version":1,"last":[{"effect":{"type":"rainbow","speed":-5},"targets":"all"}]}"#,
        ] {
            std::fs::write(dir.file(), bad).unwrap();
            let (mut s, warn) = Store::open(dir.file());
            assert!(warn.unwrap().contains("unreadable"), "{bad}");
            assert!(s.profiles().is_empty());
            let aside = dir.file().with_extension("json.unreadable");
            assert_eq!(std::fs::read_to_string(&aside).unwrap(), bad);
            s.save_profile("New", &[]).unwrap();
            assert_eq!(std::fs::read_to_string(&aside).unwrap(), bad, "kept");
        }
    }

    #[test]
    fn a_file_that_exists_but_cannot_be_read_is_never_written() {
        let dir = TempDir::new("noread");
        // Reading a directory fails with something other than "not found".
        std::fs::create_dir_all(dir.file()).unwrap();
        let (mut s, warn) = Store::open(dir.file());
        assert!(warn.unwrap().contains("will not be saved"));
        assert!(s.save_profile("A", &[]).is_err());
        assert!(s.profiles().is_empty());
        assert!(dir.file().is_dir());
    }

    #[test]
    fn a_failed_write_changes_nothing_in_memory() {
        let dir = TempDir::new("fail");
        // A directory where the temp file goes makes the write fail.
        let tmp = dir.file().with_extension("json.tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("x"), "").unwrap();
        let (mut s, warn) = Store::open(dir.file());
        assert!(warn.is_none());
        assert!(s.save_profile("A", &[]).is_err());
        assert!(s.profiles().is_empty());
        assert!(s.set_last(&[static_on("x", 1)]).is_err());
        assert!(s.last().is_empty());
    }
}
