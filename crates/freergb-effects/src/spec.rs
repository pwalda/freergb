// SPDX-License-Identifier: GPL-2.0-or-later
//! The declarative effect format.
//!
//! ```json
//! { "effect": { "type": "wave", "colors": ["#000000", "#00aaff"], "speed": 0.5,
//!               "direction": "forward", "brightness": 1.0, "density": 1.0 },
//!   "targets": "all", "canvas": "shared" }
//! ```
//!
//! Units are physical so a JS preview and the Rust runtime cannot disagree about them:
//! `speed` is cycles per second, `density` is spatial cycles across the canvas, `brightness`
//! is a 0..1 multiplier. Unknown fields are rejected: a typo should be an error, not a
//! silently different effect.

use crate::react::React;
use freergb_sdk::DeviceKey;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b))
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let hex = s.strip_prefix('#').unwrap_or(&s);
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(de::Error::custom(format!("colour {s:?} is not #rrggbb")));
        }
        let p = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(de::Error::custom);
        Ok(Rgb {
            r: p(0)?,
            g: p(2)?,
            b: p(4)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    /// One colour, no motion.
    Static,
    /// Colours spread along the canvas, no motion.
    Gradient,
    /// Every LED shows the same hue, cycling through the spectrum over time.
    SpectrumCycle,
    /// A hue gradient across the canvas that scrolls over time.
    Rainbow,
    /// Every LED fades in and out; with several colours each breath takes the next one.
    Breathing,
    /// A travelling wave that sweeps the palette across the canvas.
    Wave,
    /// A bright head with a fading tail (`colors[0]` at the head towards the last colour at
    /// the tail end); `density` comets across the canvas.
    Comet,
    /// Moving dashes with gaps, each dash the next palette colour; `8 * density` dashes.
    Marquee,
    /// A soft band sweeping back and forth, coloured by where it is on the palette; `speed`
    /// is round trips per second, wider for a lower `density`.
    BouncingBeam,
    /// Spots twinkle on and off at random, each in a random palette colour; more of them for a
    /// higher `density`.
    Starfield,
    /// Flickering flames, hot at the start of the canvas (the end for `reverse`); the palette
    /// runs from embers to the hottest colour.
    Fire,
    /// Drops falling along the canvas at random times and speeds, each a random palette colour;
    /// `4 * density` drops at most at once.
    Rain,
    /// Colours set by hand: `colors[i]` is LED `i` of the device (zones concatenated, the order
    /// of the device's own colour list). Padded with black or cut to the device's LED count,
    /// so a zone resize needs no edit. `brightness` scales it. Single device only.
    Painted,
    /// One of the device's own built-in modes (`mode`), run by the hardware itself: no frames
    /// are streamed. `speed` and `brightness` are positions 0..1 in the mode's own range,
    /// `mode_direction` is the device's direction, `colors` its mode colours. Single device only.
    DeviceMode,
}

/// The directions a device's built-in mode can offer (OpenRGB's `MODE_DIRECTION_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeDirection {
    Left,
    Right,
    Up,
    Down,
    Horizontal,
    Vertical,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl ModeDirection {
    pub const ALL: [ModeDirection; 10] = [
        ModeDirection::Left,
        ModeDirection::Right,
        ModeDirection::Up,
        ModeDirection::Down,
        ModeDirection::Horizontal,
        ModeDirection::Vertical,
        ModeDirection::UpLeft,
        ModeDirection::UpRight,
        ModeDirection::DownLeft,
        ModeDirection::DownRight,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Motion towards increasing x (left to right along a strip).
    #[default]
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectSpec {
    #[serde(rename = "type")]
    pub kind: EffectKind,
    #[serde(default)]
    pub colors: Vec<Rgb>,
    /// Cycles per second.
    #[serde(default = "default_speed")]
    pub speed: f64,
    #[serde(default)]
    pub direction: Direction,
    /// 0..=1.
    #[serde(default = "one")]
    pub brightness: f64,
    /// Spatial cycles across the canvas (rainbow, wave); a size or amount for the other moving
    /// effects, see each [`EffectKind`].
    #[serde(default = "one")]
    pub density: f64,
    /// `device_mode` only: the name of the device's mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// `device_mode` only. Software effects use `direction`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_direction: Option<ModeDirection>,
    /// Which way a software effect moves across the desk map, in degrees 0..360 (0 = right,
    /// 90 = down). Unset means the effect's own default, see [`EffectSpec::angle`].
    #[serde(default, rename = "angle", skip_serializing_if = "Option::is_none")]
    pub angle_deg: Option<f64>,
    /// `device_mode` only, on devices with per-zone modes: zones that run a mode of their own.
    /// A zone left out follows the device's mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<ZoneLook>,
    /// Software effects only: one of the look's settings follows a reading (see `react`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub react: Option<React>,
}

/// One zone's own built-in mode inside a `device_mode` look. The fields mean what they mean on
/// the look itself, applied to the zone's own mode list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneLook {
    /// Index in the device's zone list.
    pub zone: u32,
    pub mode: String,
    #[serde(default)]
    pub colors: Vec<Rgb>,
    /// Position 0..1 in the mode's speed range.
    #[serde(default = "half")]
    pub speed: f64,
    #[serde(default = "one")]
    pub brightness: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_direction: Option<ModeDirection>,
}

impl ZoneLook {
    /// The zone's settings as a `device_mode` effect, for validating and for `hwmode::apply`.
    pub fn effect(&self) -> EffectSpec {
        EffectSpec {
            kind: EffectKind::DeviceMode,
            colors: self.colors.clone(),
            speed: self.speed,
            direction: Direction::default(),
            brightness: self.brightness,
            density: 1.0,
            mode: Some(self.mode.clone()),
            mode_direction: self.mode_direction,
            angle_deg: None,
            zones: vec![],
            react: None,
        }
    }
}

impl EffectSpec {
    /// The angle the canvas is laid out along: the stored one, else rain falls, fire rises and
    /// everything else runs left to right.
    pub fn angle(&self) -> f64 {
        self.angle_deg.unwrap_or(match self.kind {
            EffectKind::Rain => 90.0,
            EffectKind::Fire => 270.0,
            _ => 0.0,
        })
    }
}

fn default_speed() -> f64 {
    0.2
}
fn half() -> f64 {
    0.5
}
fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecError {
    #[error("{0:?} needs at least one colour")]
    NeedsColors(EffectKind),
    #[error("speed must be a finite number from 0 to {max}", max = MAX_SPEED)]
    Speed,
    #[error("brightness must be between 0 and 1")]
    Brightness,
    #[error("density must be a finite number from 0 to {max}", max = MAX_DENSITY)]
    Density,
    #[error("at most {0} colours are supported")]
    TooManyColors(usize),
    #[error("a scene needs at least one target device")]
    NoTargets,
    #[error("a device mode needs the mode's name")]
    NeedsMode,
    #[error("`mode` and `mode_direction` only belong to a device mode")]
    ModeFieldsOnSoftwareEffect,
    #[error("{0:?} runs on exactly one device")]
    NeedsOneDevice(EffectKind),
    #[error("a device mode's speed is a position between 0 and 1")]
    ModeSpeed,
    #[error("a group's look needs the group's name and its devices")]
    BadGroup,
    #[error("an angle is 0 to 360 degrees, and only software effects have one")]
    Angle,
    #[error("zone modes belong to a device mode, at most {MAX_ZONES}, each zone once")]
    Zones,
    #[error("a look follows a reading only as a software effect, with a range low to high and a setting that reading can move")]
    React,
}

pub const MAX_SPEED: f64 = 20.0;
pub const MAX_DENSITY: f64 = 64.0;
pub const MAX_COLORS: usize = 32;
/// More zones than any controller OpenRGB knows has.
pub const MAX_ZONES: usize = 64;
/// A painted look names every LED of a device, so it may be much longer than a palette.
pub const MAX_PAINTED: usize = 4096;

impl EffectSpec {
    pub fn validate(&self) -> Result<(), SpecError> {
        let device_mode = self.kind == EffectKind::DeviceMode;
        // A mode's own colours are optional: many modes take none.
        let needs_colors = !matches!(
            self.kind,
            EffectKind::SpectrumCycle | EffectKind::Rainbow | EffectKind::DeviceMode
        );
        if needs_colors && self.colors.is_empty() {
            return Err(SpecError::NeedsColors(self.kind));
        }
        let cap = if self.kind == EffectKind::Painted {
            MAX_PAINTED
        } else {
            MAX_COLORS
        };
        if self.colors.len() > cap {
            return Err(SpecError::TooManyColors(cap));
        }
        if let Some(a) = self.angle_deg {
            if device_mode || !a.is_finite() || !(0.0..=360.0).contains(&a) {
                return Err(SpecError::Angle);
            }
        }
        if device_mode {
            if self.mode.as_deref().map_or(true, str::is_empty) {
                return Err(SpecError::NeedsMode);
            }
            if !self.speed.is_finite() || !(0.0..=1.0).contains(&self.speed) {
                return Err(SpecError::ModeSpeed);
            }
            if self.zones.len() > MAX_ZONES {
                return Err(SpecError::Zones);
            }
            let mut seen = std::collections::HashSet::new();
            for z in &self.zones {
                if !seen.insert(z.zone) {
                    return Err(SpecError::Zones);
                }
                z.effect().validate()?;
            }
        } else if !self.zones.is_empty() {
            return Err(SpecError::Zones);
        } else if self.mode.is_some() || self.mode_direction.is_some() {
            return Err(SpecError::ModeFieldsOnSoftwareEffect);
        } else if !self.speed.is_finite() || !(0.0..=MAX_SPEED).contains(&self.speed) {
            return Err(SpecError::Speed);
        }
        if !self.brightness.is_finite() || !(0.0..=1.0).contains(&self.brightness) {
            return Err(SpecError::Brightness);
        }
        if !self.density.is_finite() || !(0.0..=MAX_DENSITY).contains(&self.density) {
            return Err(SpecError::Density);
        }
        if let Some(m) = &self.react {
            let (lo, hi) = m.range();
            let software = !matches!(self.kind, EffectKind::DeviceMode | EffectKind::Painted);
            if !software
                || !m.source.drives(m.drives)
                || !lo.is_finite()
                || !hi.is_finite()
                || lo >= hi
                || [m.sensitivity, m.strength]
                    .iter()
                    .flatten()
                    .any(|v| !(0.0..=1.0).contains(v))
            {
                return Err(SpecError::React);
            }
        }
        Ok(())
    }
}

/// How the target devices share a coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Canvas {
    /// Each device spans the whole 0..1 range: every device shows the same picture.
    PerDevice,
    /// The devices sit side by side in target order, each in an equal slice of 0..1, so a
    /// wave travels across all of them as one surface.
    #[default]
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Targets {
    /// Every device present, including ones that appear later.
    All,
    /// These devices, in this order (the order is the left-to-right order of a shared canvas).
    /// Identified by `DeviceKey` so the scene survives an OpenRGB restart.
    Devices(Vec<DeviceKey>),
}

/// One effect on one group of devices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub effect: EffectSpec,
    pub targets: Targets,
    #[serde(default)]
    pub canvas: Canvas,
    /// The named group of devices this is the look of, if any. Only a label: the engine plays
    /// `targets`, which the app keeps in step with the group's members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl Scene {
    pub fn validate(&self) -> Result<(), SpecError> {
        self.effect.validate()?;
        if matches!(&self.targets, Targets::Devices(d) if d.is_empty()) {
            return Err(SpecError::NoTargets);
        }
        if let Some(g) = &self.group {
            if g.trim().is_empty() || !matches!(self.targets, Targets::Devices(_)) {
                return Err(SpecError::BadGroup);
            }
        }
        if matches!(
            self.effect.kind,
            EffectKind::DeviceMode | EffectKind::Painted
        ) && !matches!(&self.targets, Targets::Devices(d) if d.len() == 1)
        {
            return Err(SpecError::NeedsOneDevice(self.effect.kind));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<EffectSpec, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn defaults_fill_in_and_round_trip() {
        let e = parse(r##"{"type":"wave","colors":["#000000","#00aaff"]}"##).unwrap();
        assert_eq!(e.speed, 0.2);
        assert_eq!(e.brightness, 1.0);
        assert_eq!(e.direction, Direction::Forward);
        let back: EffectSpec = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn typos_and_bad_colours_are_errors_not_silent_defaults() {
        assert!(parse(r##"{"type":"wave","colours":["#000000"]}"##).is_err());
        assert!(parse(r#"{"type":"sparkle"}"#).is_err());
        assert!(parse(r##"{"type":"static","colors":["#12345"]}"##).is_err());
        assert!(parse(r##"{"type":"static","colors":["#12345g"]}"##).is_err());
    }

    #[test]
    fn validation_rules() {
        let ok = |s: &str| parse(s).unwrap().validate();
        assert_eq!(ok(r#"{"type":"rainbow"}"#), Ok(()));
        assert_eq!(
            ok(r#"{"type":"static"}"#),
            Err(SpecError::NeedsColors(EffectKind::Static))
        );
        assert_eq!(
            ok(r##"{"type":"static","colors":["#fff000"],"speed":-1}"##),
            Err(SpecError::Speed)
        );
        assert_eq!(
            ok(r##"{"type":"static","colors":["#fff000"],"speed":21}"##),
            Err(SpecError::Speed)
        );
        assert_eq!(
            ok(r##"{"type":"static","colors":["#fff000"],"brightness":1.5}"##),
            Err(SpecError::Brightness)
        );
        assert_eq!(
            ok(r##"{"type":"wave","colors":["#fff000"],"density":100}"##),
            Err(SpecError::Density)
        );
    }

    #[test]
    fn non_finite_numbers_are_rejected() {
        let mut e = parse(r#"{"type":"rainbow"}"#).unwrap();
        e.speed = f64::NAN;
        assert_eq!(e.validate(), Err(SpecError::Speed));
        e.speed = 0.2;
        e.brightness = f64::INFINITY;
        assert_eq!(e.validate(), Err(SpecError::Brightness));
    }

    #[test]
    fn a_reading_follows_software_looks_only_and_only_where_it_can() {
        let ok = |j: &str| parse(j).map(|e| e.validate());
        let wave = r##"{"type":"wave","colors":["#ff0000"],"react":"##;
        assert_eq!(
            ok(&format!(
                r#"{wave}{{"source":"sound","drives":"brightness"}}}}"#
            ))
            .unwrap(),
            Ok(())
        );
        assert_eq!(
            ok(&format!(
                r#"{wave}{{"source":"cpu_temp","drives":"color","low":40,"high":70}}}}"#
            ))
            .unwrap(),
            Ok(())
        );
        // The screen gives colours, the spectrum levels; a range runs low to high; sensitivity
        // and strength are 0..1.
        for bad in [
            r#"{"source":"screen","drives":"brightness"}"#,
            r#"{"source":"spectrum","drives":"amount"}"#,
            r#"{"source":"cpu_load","drives":"amount","low":50,"high":50}"#,
            r#"{"source":"sound","drives":"brightness","sensitivity":1.5}"#,
            r#"{"source":"beat","drives":"brightness","strength":-0.1}"#,
        ] {
            assert_eq!(
                ok(&format!("{wave}{bad}}}")).unwrap(),
                Err(SpecError::React),
                "{bad}"
            );
        }
        let mode = r#"{"type":"device_mode","mode":"Static","speed":0.5,"react":{"source":"sound","drives":"brightness"}}"#;
        assert_eq!(ok(mode).unwrap(), Err(SpecError::React));
    }

    #[test]
    fn scene_needs_targets() {
        let s: Scene =
            serde_json::from_str(r#"{"effect":{"type":"rainbow"},"targets":{"devices":[]}}"#)
                .unwrap();
        assert_eq!(s.validate(), Err(SpecError::NoTargets));
        let all: Scene =
            serde_json::from_str(r#"{"effect":{"type":"rainbow"},"targets":"all"}"#).unwrap();
        assert_eq!(all.validate(), Ok(()));
        assert_eq!(all.canvas, Canvas::Shared);
    }

    fn mode_scene(targets: &str, extra: &str) -> Result<Scene, serde_json::Error> {
        serde_json::from_str(&format!(
            r#"{{"effect":{{"type":"device_mode","mode":"Breathing","speed":0.5{extra}}},"targets":{targets}}}"#
        ))
    }
    const ONE: &str = r#"{"devices":[{"name":"RAM","location":"i2c","serial":"","nth":0}]}"#;

    #[test]
    fn a_device_mode_validates_and_round_trips_without_software_fields() {
        let s = mode_scene(ONE, r##","mode_direction":"up_left","colors":["#102030"]"##).unwrap();
        assert_eq!(s.validate(), Ok(()));
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(), s);
        // existing effects serialise exactly as before: no `mode` keys appear
        let sw: Scene =
            serde_json::from_str(r#"{"effect":{"type":"rainbow"},"targets":"all"}"#).unwrap();
        assert!(!serde_json::to_string(&sw).unwrap().contains("mode"));
    }

    #[test]
    fn a_device_mode_needs_its_name_one_device_and_a_0_to_1_speed() {
        let no_name: Scene = serde_json::from_str(&format!(
            r#"{{"effect":{{"type":"device_mode","speed":0.5}},"targets":{ONE}}}"#
        ))
        .unwrap();
        assert_eq!(no_name.validate(), Err(SpecError::NeedsMode));
        assert_eq!(
            mode_scene(r#""all""#, "").unwrap().validate(),
            Err(SpecError::NeedsOneDevice(EffectKind::DeviceMode))
        );
        let two = r#"{"devices":[{"name":"A","location":"","serial":"","nth":0},{"name":"B","location":"","serial":"","nth":0}]}"#;
        assert_eq!(
            mode_scene(two, "").unwrap().validate(),
            Err(SpecError::NeedsOneDevice(EffectKind::DeviceMode))
        );
        let fast: Scene = serde_json::from_str(&format!(
            r#"{{"effect":{{"type":"device_mode","mode":"X","speed":2}},"targets":{ONE}}}"#
        ))
        .unwrap();
        assert_eq!(fast.validate(), Err(SpecError::ModeSpeed));
    }

    #[test]
    fn zone_looks_belong_to_a_device_mode_and_name_each_zone_once() {
        let fan =
            |z: u32| format!(r##"{{"zone":{z},"mode":"Pulse","speed":0.3,"colors":["#ff4000"]}}"##);
        let s = mode_scene(ONE, &format!(r#","zones":[{},{}]"#, fan(0), fan(2))).unwrap();
        assert_eq!(s.validate(), Ok(()));
        assert_eq!(s.effect.zones[1].zone, 2);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(), s);
        assert!(
            !serde_json::to_string(&mode_scene(ONE, "").unwrap())
                .unwrap()
                .contains("zones"),
            "no zone looks: the key stays out"
        );
        let twice = mode_scene(ONE, &format!(r#","zones":[{},{}]"#, fan(1), fan(1))).unwrap();
        assert_eq!(twice.validate(), Err(SpecError::Zones));
        let fast = mode_scene(ONE, r#","zones":[{"zone":0,"mode":"Pulse","speed":3}]"#).unwrap();
        assert_eq!(fast.validate(), Err(SpecError::ModeSpeed));
        let nameless = mode_scene(ONE, r#","zones":[{"zone":0,"mode":""}]"#).unwrap();
        assert_eq!(nameless.validate(), Err(SpecError::NeedsMode));
        let sw: Scene = serde_json::from_str(&format!(
            r#"{{"effect":{{"type":"rainbow","zones":[{}]}},"targets":"all"}}"#,
            fan(0)
        ))
        .unwrap();
        assert_eq!(sw.validate(), Err(SpecError::Zones));
    }

    #[test]
    fn mode_fields_on_a_software_effect_are_an_error() {
        let s: Scene = serde_json::from_str(
            r##"{"effect":{"type":"wave","colors":["#ffffff"],"mode":"Breathing"},"targets":"all"}"##,
        )
        .unwrap();
        assert_eq!(s.validate(), Err(SpecError::ModeFieldsOnSoftwareEffect));
    }

    #[test]
    fn a_painted_look_names_every_led_for_exactly_one_device() {
        let paint = |n: usize, targets: &str| -> Result<(), SpecError> {
            let colors = vec!["#010203"; n].join("\",\"");
            let s: Scene = serde_json::from_str(&format!(
                r#"{{"effect":{{"type":"painted","colors":["{colors}"]}},"targets":{targets}}}"#
            ))
            .unwrap();
            s.validate()
        };
        assert_eq!(paint(300, ONE), Ok(()));
        assert_eq!(paint(MAX_PAINTED, ONE), Ok(()));
        assert_eq!(
            paint(MAX_PAINTED + 1, ONE),
            Err(SpecError::TooManyColors(MAX_PAINTED))
        );
        assert_eq!(
            paint(5, r#""all""#),
            Err(SpecError::NeedsOneDevice(EffectKind::Painted))
        );
        let empty: Scene = serde_json::from_str(&format!(
            r#"{{"effect":{{"type":"painted","colors":[]}},"targets":{ONE}}}"#
        ))
        .unwrap();
        assert_eq!(
            empty.validate(),
            Err(SpecError::NeedsColors(EffectKind::Painted))
        );
        // the palette cap of the other kinds is unchanged
        let long: EffectSpec = serde_json::from_str(&format!(
            r##"{{"type":"gradient","colors":["{}"]}}"##,
            vec!["#010203"; MAX_COLORS + 1].join("\",\"")
        ))
        .unwrap();
        assert_eq!(long.validate(), Err(SpecError::TooManyColors(MAX_COLORS)));
    }

    #[test]
    fn a_group_look_needs_a_name_and_named_devices() {
        let ok: Scene = serde_json::from_str(
            r#"{"effect":{"type":"rainbow"},"targets":{"devices":[{"name":"a","location":"l","serial":"","nth":0}]},"group":"Desk"}"#,
        )
        .unwrap();
        ok.validate().unwrap();
        assert_eq!(serde_json::to_value(&ok).unwrap()["group"], "Desk");
        let plain: Scene =
            serde_json::from_str(r#"{"effect":{"type":"rainbow"},"targets":"all"}"#).unwrap();
        assert!(
            serde_json::to_value(&plain).unwrap().get("group").is_none(),
            "absent, not null, so older readers accept it"
        );
        let all = Scene {
            group: Some("Desk".into()),
            ..plain.clone()
        };
        assert_eq!(all.validate(), Err(SpecError::BadGroup));
        let blank = Scene {
            group: Some(" ".into()),
            ..ok
        };
        assert_eq!(blank.validate(), Err(SpecError::BadGroup));
    }
}
