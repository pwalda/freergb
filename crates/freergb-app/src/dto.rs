// SPDX-License-Identifier: GPL-2.0-or-later
//! What the UI sees. Kept apart from the wire types so the IPC shape does not change when the
//! protocol structs do.

use freergb_effects::hwmode;
use freergb_effects::layout::{self, SegmentKey};
use freergb_effects::ModeDirection;
use freergb_sdk::ids::{color_mode, mode_flag, zone_flag};
use freergb_sdk::{keys_of, Client, ControllerData, DeviceKey, Event, Mode};
use freergb_supervisor::{FailureKind, State};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerState {
    Starting {
        owned: bool,
    },
    Connected {
        owned: bool,
        server_name: String,
    },
    Reconnecting {
        attempt: u32,
        retry_in_secs: u64,
    },
    Failed {
        reason: &'static str,
        message: String,
    },
}

impl From<&State> for ServerState {
    fn from(s: &State) -> Self {
        match s {
            State::Starting { owned } => Self::Starting { owned: *owned },
            State::Connected { owned, server_name } => Self::Connected {
                owned: *owned,
                server_name: server_name.clone(),
            },
            State::Reconnecting { attempt, retry_in } => Self::Reconnecting {
                attempt: *attempt,
                retry_in_secs: retry_in.as_secs(),
            },
            State::Failed(f) => Self::Failed {
                reason: match f.kind {
                    FailureKind::UnsupportedVersion => "unsupported_version",
                    FailureKind::PortInUse => "port_in_use",
                    FailureKind::NoServerCommand => "no_server",
                    FailureKind::LaunchFailed => "launch_failed",
                    FailureKind::StartTimeout => "start_timeout",
                    FailureKind::CrashLoop => "crash_loop",
                },
                message: f.message.clone(),
            },
        }
    }
}

/// A zone's LED grid (a keyboard). `cells` is row-major, `None` for an empty cell, and holds
/// device-wide LED indices: the zone's own offset is already added, so the UI never has to
/// work out where a zone starts.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct MatrixView {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<Option<u32>>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ZoneView {
    /// Index in the device's zone list; what `resize_zone` takes.
    pub index: u32,
    pub name: String,
    pub leds: u32,
    /// Device-wide index of the zone's first LED (zones are concatenated in order); `painted`
    /// colours and `DeviceView::colors` use the same indices.
    pub start: u32,
    pub matrix: Option<MatrixView>,
    /// The LED counts the zone accepts, and whether it accepts a change at all (the server
    /// silently ignores a resize of any other zone, so the UI must not offer one).
    pub min_leds: u32,
    pub max_leds: u32,
    pub resizable: bool,
    /// The zone's own built-in modes (OpenRGB 1.1 per-zone modes); empty on most devices.
    pub modes: Vec<ModeView>,
    /// Index into the zone's own mode list, -1 when the zone follows the device's mode.
    pub active_mode: i32,
}

/// One of a device's own built-in modes, as the picker needs it. Speed and brightness are
/// positions 0..1 in the mode's own range (what a `device_mode` effect stores), `None` when the
/// mode has no such setting.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ModeView {
    pub index: u32,
    pub name: String,
    pub speed: Option<f64>,
    pub brightness: Option<f64>,
    /// Empty when the mode has no direction.
    pub directions: Vec<ModeDirection>,
    pub direction: Option<ModeDirection>,
    /// How many colours the mode takes; both 0 when it takes none of its own.
    pub colors_min: u32,
    pub colors_max: u32,
    /// `#rrggbb`: the mode's own colours.
    pub colors: Vec<String>,
    /// The device can keep this mode in its own memory (`MANUAL_SAVE`).
    pub saveable: bool,
    /// The mode drives the whole device, so its zones cannot run modes of their own
    /// (`REQUIRES_ENTIRE_DEVICE`).
    pub whole_device: bool,
}

/// `None` for the modes the engine uses for streaming frames. OpenRGB's `SetCustomMode` picks
/// the first per-LED (or mode-specific) mode named "Direct", then "Custom", then "Static"; the
/// first two show whatever was last written, so they are not a look to pick. Other per-LED
/// modes (a "Breathing" that takes each LED's colour) are real built-in looks and stay.
fn mode_view(index: usize, m: &Mode) -> Option<ModeView> {
    let per_led = m.color_mode == color_mode::PER_LED;
    if per_led && matches!(m.name.as_str(), "Direct" | "Custom") {
        return None;
    }
    let has = |f: u32| m.flags & f != 0;
    let directions = hwmode::directions_of(m.flags);
    let own_colors = m.color_mode == color_mode::MODE_SPECIFIC;
    Some(ModeView {
        index: index as u32,
        name: m.name.clone(),
        speed: has(mode_flag::HAS_SPEED)
            .then(|| hwmode::position(m.speed_min, m.speed_max, m.speed)),
        brightness: has(mode_flag::HAS_BRIGHTNESS)
            .then(|| hwmode::position(m.brightness_min, m.brightness_max, m.brightness)),
        direction: hwmode::direction_of(m.direction).filter(|d| directions.contains(d)),
        directions,
        colors_min: if own_colors { m.colors_min } else { 0 },
        colors_max: if own_colors { m.colors_max } else { 0 },
        colors: if own_colors {
            m.colors
                .iter()
                .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
                .collect()
        } else {
            vec![]
        },
        saveable: has(mode_flag::MANUAL_SAVE),
        whole_device: has(mode_flag::REQUIRES_ENTIRE_DEVICE),
    })
}

/// Whether OpenRGB is looking for devices, and how far it got.
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Detection {
    pub scanning: bool,
    /// 0..100; `None` until the server reports progress.
    pub percent: Option<u32>,
    /// The detector running now (e.g. "Detecting I2C interfaces"); may be empty.
    pub label: String,
}

impl Detection {
    pub fn started() -> Self {
        Self {
            scanning: true,
            ..Self::default()
        }
    }

    /// Follow a server event; true when the state changed.
    pub fn follow(&mut self, ev: &Event) -> bool {
        let next = match ev {
            Event::DetectionStarted => Self::started(),
            Event::DetectionProgress { percent, label } => Self {
                scanning: true,
                percent: Some((*percent).min(100)),
                label: label.clone(),
            },
            Event::DetectionComplete | Event::Disconnected => Self::default(),
            _ => return false,
        };
        let changed = *self != next;
        *self = next;
        changed
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct DeviceView {
    /// Valid for this server process only; never persist it (see `DeviceKey`).
    pub id: u32,
    /// Survives an OpenRGB restart; what scenes use to name a device.
    pub key: DeviceKey,
    pub name: String,
    pub vendor: String,
    /// What the device page's info panel lists; any of them may be empty.
    pub description: String,
    pub version: String,
    pub serial: String,
    pub location: String,
    pub device_type: i32,
    pub active_mode: i32,
    /// The built-in modes worth picking (see `mode_view`).
    pub modes: Vec<ModeView>,
    pub zones: Vec<ZoneView>,
    /// How the device sits on the desk map: one piece, or one per OpenRGB segment plus the
    /// LEDs left over (`layout::pieces`; the preview places these as the engine does).
    pub pieces: Vec<PieceView>,
    /// `#rrggbb`, one per LED.
    pub colors: Vec<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PieceView {
    /// The segment's key in the desk map, `None` for the device itself (or what is left).
    pub segment: Option<SegmentKey>,
    /// The segment's name, empty for the device's own piece.
    pub name: String,
    /// Device-wide LED indices, ascending.
    pub leds: Vec<u32>,
}

fn device_view(id: u32, key: DeviceKey, d: &ControllerData) -> DeviceView {
    DeviceView {
        id,
        key,
        name: if d.display_name.is_empty() {
            d.name.clone()
        } else {
            d.display_name.clone()
        },
        vendor: d.vendor.clone(),
        description: d.description.clone(),
        version: d.version.clone(),
        serial: d.serial.clone(),
        location: d.location.clone(),
        device_type: d.device_type,
        active_mode: d.active_mode,
        modes: d
            .modes
            .iter()
            .enumerate()
            .filter_map(|(i, m)| mode_view(i, m))
            .collect(),
        zones: d
            .zones
            .iter()
            .enumerate()
            .scan(0u32, |start, (i, z)| {
                let first = *start;
                *start += z.leds_count;
                Some(ZoneView {
                    index: i as u32,
                    name: z.name.clone(),
                    leds: z.leds_count,
                    start: first,
                    matrix: z
                        .matrix_map
                        .as_ref()
                        .filter(|m| m.width > 0)
                        .map(|m| MatrixView {
                            width: m.width,
                            height: m.height,
                            cells: m
                                .data
                                .iter()
                                .map(|&led| (led < z.leds_count).then(|| first + led))
                                .collect(),
                        }),
                    min_leds: z.leds_min,
                    max_leds: z.leds_max,
                    resizable: z.flags
                        & (zone_flag::CONFIGURABLE_SIZE | zone_flag::SIZE_EFFECTS_ONLY)
                        != 0
                        && z.leds_max > z.leds_min,
                    modes: z
                        .modes
                        .iter()
                        .enumerate()
                        .filter_map(|(i, m)| mode_view(i, m))
                        .collect(),
                    active_mode: if usize::try_from(z.active_mode).is_ok_and(|i| i < z.modes.len())
                    {
                        z.active_mode
                    } else {
                        -1
                    },
                })
            })
            .collect(),
        pieces: layout::pieces(d)
            .into_iter()
            .map(|p| PieceView {
                name: p
                    .segment
                    .and_then(|k| {
                        let z = d.zones.get(k.zone as usize)?;
                        z.segments.iter().find(|s| s.start_idx == k.start)
                    })
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                segment: p.segment,
                leds: p.leds.into_iter().map(|i| i as u32).collect(),
            })
            .collect(),
        colors: d
            .colors
            .iter()
            .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
            .collect(),
    }
}

/// Keys are computed over the whole list: `DeviceKey::nth` only means something relative to
/// the other devices.
pub fn views_of(devices: &[(u32, ControllerData)]) -> Vec<DeviceView> {
    let keys = keys_of(devices.iter().map(|(id, d)| (*id, d)));
    devices
        .iter()
        .zip(keys)
        .map(|((id, d), (kid, key))| {
            debug_assert_eq!(*id, kid);
            device_view(*id, key, d)
        })
        .collect()
}

pub fn devices_of(client: Option<&Client>) -> Vec<DeviceView> {
    use freergb_sdk::RgbBackend;
    client.map(|c| views_of(&c.devices())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use freergb_supervisor::Failure;
    use std::time::Duration;

    #[test]
    fn detection_follows_started_progress_and_complete() {
        let mut d = Detection::default();
        assert!(d.follow(&Event::DetectionStarted));
        assert_eq!(d, Detection::started());
        assert!(d.follow(&Event::DetectionProgress {
            percent: 140,
            label: "Corsair".into()
        }));
        assert_eq!((d.percent, d.label.as_str()), (Some(100), "Corsair"));
        assert!(!d.follow(&Event::ProfileLoaded));
        assert!(d.follow(&Event::DetectionComplete));
        assert_eq!(d, Detection::default());
        assert!(!d.follow(&Event::DetectionComplete));
        assert_eq!(
            serde_json::to_string(&Detection::started()).unwrap(),
            r#"{"scanning":true,"percent":null,"label":""}"#
        );
    }

    #[test]
    fn state_json_is_tagged_and_flat() {
        let v = serde_json::to_string(&ServerState::from(&State::Reconnecting {
            attempt: 2,
            retry_in: Duration::from_secs(4),
        }))
        .unwrap();
        assert_eq!(
            v,
            r#"{"kind":"reconnecting","attempt":2,"retry_in_secs":4}"#
        );
    }

    #[test]
    fn identical_devices_get_distinct_keys_and_the_key_has_the_shape_the_ui_sends_back() {
        let one = freergb_mock::devices::ram_stick(0);
        let views = views_of(&[(4, one.clone()), (9, one)]);
        assert_eq!(views[0].key.nth, 0);
        assert_eq!(views[1].key.nth, 1);
        assert_ne!(views[0].key, views[1].key);
        let json = serde_json::to_value(&views[0]).unwrap();
        let mut fields: Vec<_> = json["key"].as_object().unwrap().keys().cloned().collect();
        fields.sort();
        assert_eq!(fields, ["location", "name", "nth", "serial"]);
    }

    #[test]
    fn modes_are_views_with_positions_directions_and_colours_and_skip_per_led_modes() {
        let d = freergb_mock::devices::with_rich_modes(freergb_mock::devices::ram_stick(0));
        let v = &views_of(&[(1, d)])[0];
        let names: Vec<_> = v.modes.iter().map(|m| (m.index, m.name.as_str())).collect();
        // "Direct" (index 0) is the streaming mode: not offered. Indices still point into the device's list.
        assert_eq!(
            names,
            [
                (1, "Static"),
                (2, "Rainbow Wave"),
                (3, "Breathing"),
                (4, "Off")
            ]
        );
        let breathing = &v.modes[2];
        // speed 55 in an inverted 100..10 range is halfway
        assert_eq!(breathing.speed, Some(0.5));
        assert_eq!(breathing.brightness, Some(1.0));
        assert_eq!(breathing.direction, Some(ModeDirection::Up));
        assert_eq!(
            breathing.directions,
            [
                ModeDirection::Left,
                ModeDirection::Right,
                ModeDirection::Up,
                ModeDirection::Down
            ]
        );
        assert_eq!((breathing.colors_min, breathing.colors_max), (2, 3));
        assert_eq!(breathing.colors, ["#ff0000", "#0000ff"]);
        assert!(breathing.saveable);
        assert!(v
            .zones
            .iter()
            .all(|z| z.modes.is_empty() && z.active_mode == -1));
        let hub = &views_of(&[(2, freergb_mock::devices::fan_hub())])[0];
        let fan = &hub.zones[1];
        let names: Vec<_> = fan.modes.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            (names, fan.active_mode),
            (vec!["Fixed", "Pulse", "Spectrum"], -1)
        );
        assert_eq!(fan.modes[1].colors_max, 2);
        let rainbow = hub
            .modes
            .iter()
            .find(|m| m.name == "Super Rainbow")
            .unwrap();
        assert!(rainbow.whole_device && !hub.modes[0].whole_device);
        assert!(!v.modes[0].saveable, "Static has no MANUAL_SAVE");
        // Another per-LED mode is a real look and stays; "Custom" is hidden like "Direct".
        let mut d = freergb_mock::devices::ram_stick(0);
        for name in ["Custom", "Per-LED Breathing"] {
            d.modes.push(Mode {
                name: name.into(),
                color_mode: color_mode::PER_LED,
                flags: mode_flag::HAS_PER_LED_COLOR | mode_flag::HAS_SPEED,
                ..Default::default()
            });
        }
        let names: Vec<_> = views_of(&[(1, d)])[0]
            .modes
            .iter()
            .map(|m| m.name.clone())
            .collect();
        assert_eq!(names, ["Static", "Rainbow Wave", "Per-LED Breathing"]);
        let off = &v.modes[3];
        assert_eq!(
            (off.speed, off.brightness, off.direction),
            (None, None, None)
        );
        assert!(off.directions.is_empty() && off.colors.is_empty());
        let json = serde_json::to_value(breathing).unwrap();
        assert_eq!(json["directions"][2], "up");
    }

    #[test]
    fn only_zones_the_server_will_actually_resize_are_marked_resizable() {
        let strip = &views_of(&[(1, freergb_mock::devices::resizable_strip())])[0];
        let z = &strip.zones[0];
        assert_eq!(
            (z.index, z.leds, z.min_leds, z.max_leds, z.resizable),
            (0, 60, 1, 300, true)
        );
        let ram = &views_of(&[(2, freergb_mock::devices::ram_stick(0))])[0];
        assert!(ram.zones.iter().all(|z| !z.resizable));
    }

    #[test]
    fn zones_carry_their_device_wide_start_and_matrix_cells_use_device_indices() {
        use freergb_mock::devices;
        let mut d = devices::keyboard();
        // Put a 4-LED zone in front of the keys: the matrix must now start at LED 4.
        d.zones.insert(
            0,
            devices::zone("Logo", freergb_sdk::ids::zone_type::LINEAR, 4, 4, 4, 0),
        );
        devices::rebuild_leds(&mut d);
        let v = &views_of(&[(1, d)])[0];
        assert_eq!([v.zones[0].start, v.zones[1].start], [0, 4]);
        let m = v.zones[1].matrix.as_ref().unwrap();
        assert_eq!((m.width, m.height), (5, 3));
        assert_eq!(m.cells[0], Some(4), "zone-relative LED 0 is device LED 4");
        assert_eq!(m.cells[9], Some(13));
        assert_eq!(m.cells[10], None, "an empty cell");
        assert!(v.zones[0].matrix.is_none());
        let json = serde_json::to_value(m).unwrap();
        assert!(json["cells"][10].is_null());
    }

    #[test]
    fn failure_carries_a_stable_reason_code() {
        let s = ServerState::from(&State::Failed(Failure {
            kind: FailureKind::PortInUse,
            message: "x".into(),
        }));
        assert_eq!(
            s,
            ServerState::Failed {
                reason: "port_in_use",
                message: "x".into()
            }
        );
    }

    #[test]
    fn a_split_strip_lists_its_pieces_with_the_segment_names() {
        let mut strip = freergb_mock::devices::big_300();
        strip.zones[0].segments = vec![freergb_sdk::Segment {
            name: "Shelf".into(),
            start_idx: 250,
            leds_count: 50,
            ..Default::default()
        }];
        let v = &views_of(&[(1, strip)])[0];
        let p: Vec<_> = v
            .pieces
            .iter()
            .map(|p| (p.segment.map(|k| k.start), p.name.as_str(), p.leds.len()))
            .collect();
        assert_eq!(p, [(None, "", 250), (Some(250), "Shelf", 50)]);
        assert_eq!(v.pieces[1].leds[0], 250);
    }
}
