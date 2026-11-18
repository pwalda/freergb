// SPDX-License-Identifier: GPL-2.0-or-later
//! Turning a `device_mode` effect into the device's own `Mode` values.
//!
//! The effect stores speed and brightness as positions 0..1, so the same look means the same
//! thing on every device. The device's ranges are raw integers and may be inverted (OpenRGB's
//! own device page treats `min > max` as an inverted slider), so position 0 maps to `*_min` and
//! position 1 to `*_max`, whichever is larger.

use crate::spec::{EffectSpec, ModeDirection};
use freergb_sdk::ids::{color_mode, direction, mode_flag};
use freergb_sdk::{Color, Mode};

fn along(min: u32, max: u32, position: f64) -> u32 {
    let p = position.clamp(0.0, 1.0);
    let v = f64::from(min) + (f64::from(max) - f64::from(min)) * p;
    v.round() as u32
}

/// The position of `value` in `min..=max` (or `max..=min`), 0 when the range is empty.
pub fn position(min: u32, max: u32, value: u32) -> f64 {
    if min == max {
        return 0.0;
    }
    ((f64::from(value) - f64::from(min)) / (f64::from(max) - f64::from(min))).clamp(0.0, 1.0)
}

fn direction_code(d: ModeDirection) -> u32 {
    match d {
        ModeDirection::Left => direction::LEFT,
        ModeDirection::Right => direction::RIGHT,
        ModeDirection::Up => direction::UP,
        ModeDirection::Down => direction::DOWN,
        ModeDirection::Horizontal => direction::HORIZONTAL,
        ModeDirection::Vertical => direction::VERTICAL,
        ModeDirection::UpLeft => direction::UP_LEFT,
        ModeDirection::UpRight => direction::UP_RIGHT,
        ModeDirection::DownLeft => direction::DOWN_LEFT,
        ModeDirection::DownRight => direction::DOWN_RIGHT,
    }
}

/// The direction a device reports, if it is one we know.
pub fn direction_of(code: u32) -> Option<ModeDirection> {
    ModeDirection::ALL
        .into_iter()
        .find(|d| direction_code(*d) == code)
}

/// Every direction the mode's flags offer, in a stable order.
pub fn directions_of(flags: u32) -> Vec<ModeDirection> {
    ModeDirection::ALL
        .into_iter()
        .filter(|d| supports(flags, *d))
        .collect()
}

/// Whether the mode's flags allow this direction.
pub fn supports(flags: u32, d: ModeDirection) -> bool {
    use ModeDirection::*;
    let need = match d {
        Left | Right => mode_flag::HAS_DIRECTION_LR,
        Up | Down => mode_flag::HAS_DIRECTION_UD,
        Horizontal | Vertical => mode_flag::HAS_DIRECTION_HV,
        UpLeft | UpRight | DownLeft | DownRight => mode_flag::HAS_DIRECTION_DIAG,
    };
    flags & need != 0
}

/// `base` (the device's current copy of the mode) with the effect's settings applied. Only what
/// the mode supports is touched; the rest keeps the device's own value.
pub fn apply(base: &Mode, spec: &EffectSpec) -> Mode {
    let mut m = base.clone();
    if m.flags & mode_flag::HAS_SPEED != 0 {
        m.speed = along(m.speed_min, m.speed_max, spec.speed);
    }
    if m.flags & mode_flag::HAS_BRIGHTNESS != 0 {
        m.brightness = along(m.brightness_min, m.brightness_max, spec.brightness);
    }
    if let Some(d) = spec.mode_direction.filter(|d| supports(m.flags, *d)) {
        m.direction = direction_code(d);
    }
    if m.color_mode == color_mode::MODE_SPECIFIC && !spec.colors.is_empty() {
        let (lo, hi) = (m.colors_min as usize, (m.colors_max as usize).max(1));
        let mut colors: Vec<Color> = spec
            .colors
            .iter()
            .take(hi)
            .map(|c| Color {
                r: c.r,
                g: c.g,
                b: c.b,
            })
            .collect();
        // Too few colours: repeat the last one rather than send a short list the device rejects.
        let last = colors.last().copied().unwrap_or(Color { r: 0, g: 0, b: 0 });
        while colors.len() < lo {
            colors.push(last);
        }
        m.colors = colors;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{EffectKind, Rgb};

    fn spec() -> EffectSpec {
        EffectSpec {
            kind: EffectKind::DeviceMode,
            colors: vec![],
            speed: 0.5,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: Some("Breathing".into()),
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        }
    }

    fn mode() -> Mode {
        Mode {
            name: "Breathing".into(),
            flags: mode_flag::HAS_SPEED
                | mode_flag::HAS_BRIGHTNESS
                | mode_flag::HAS_DIRECTION_LR
                | mode_flag::HAS_MODE_SPECIFIC_COLOR,
            speed_min: 0,
            speed_max: 100,
            brightness_min: 10,
            brightness_max: 90,
            colors_min: 2,
            colors_max: 3,
            speed: 7,
            brightness: 10,
            direction: direction::LEFT,
            color_mode: color_mode::MODE_SPECIFIC,
            colors: vec![Color { r: 1, g: 2, b: 3 }; 2],
            ..Default::default()
        }
    }

    #[test]
    fn positions_map_onto_the_devices_own_range() {
        let m = apply(&mode(), &spec());
        assert_eq!((m.speed, m.brightness), (50, 90));
        let slow = apply(
            &mode(),
            &EffectSpec {
                speed: 0.0,
                brightness: 0.0,
                ..spec()
            },
        );
        assert_eq!((slow.speed, slow.brightness), (0, 10));
    }

    #[test]
    fn an_inverted_range_still_maps_position_zero_to_min_and_one_to_max() {
        let mut base = mode();
        base.speed_min = 100;
        base.speed_max = 10;
        assert_eq!(
            apply(
                &base,
                &EffectSpec {
                    speed: 0.0,
                    ..spec()
                }
            )
            .speed,
            100
        );
        assert_eq!(
            apply(
                &base,
                &EffectSpec {
                    speed: 1.0,
                    ..spec()
                }
            )
            .speed,
            10
        );
        assert_eq!(
            apply(
                &base,
                &EffectSpec {
                    speed: 0.5,
                    ..spec()
                }
            )
            .speed,
            55
        );
        // and the reverse mapping the UI uses for what the device reports
        assert_eq!(position(100, 10, 55), 0.5);
        assert_eq!(position(0, 100, 25), 0.25);
        assert_eq!(position(5, 5, 5), 0.0);
    }

    #[test]
    fn directions_listed_for_a_mode_match_its_flags() {
        assert_eq!(
            directions_of(mode_flag::HAS_DIRECTION_LR | mode_flag::HAS_DIRECTION_HV),
            vec![
                ModeDirection::Left,
                ModeDirection::Right,
                ModeDirection::Horizontal,
                ModeDirection::Vertical
            ]
        );
        assert!(directions_of(0).is_empty());
        assert_eq!(direction_of(direction::DOWN), Some(ModeDirection::Down));
        assert_eq!(direction_of(99), None);
    }

    #[test]
    fn only_what_the_mode_supports_is_touched() {
        let mut base = mode();
        base.flags = mode_flag::HAS_MODE_SPECIFIC_COLOR;
        let m = apply(
            &base,
            &EffectSpec {
                mode_direction: Some(ModeDirection::Up),
                angle_deg: None,
                zones: vec![],
                react: None,
                ..spec()
            },
        );
        assert_eq!(
            (m.speed, m.brightness, m.direction),
            (7, 10, direction::LEFT)
        );
    }

    #[test]
    fn direction_needs_the_matching_flag() {
        let m = apply(
            &mode(),
            &EffectSpec {
                mode_direction: Some(ModeDirection::Right),
                angle_deg: None,
                zones: vec![],
                react: None,
                ..spec()
            },
        );
        assert_eq!(m.direction, direction::RIGHT);
        let m = apply(
            &mode(),
            &EffectSpec {
                mode_direction: Some(ModeDirection::Up),
                angle_deg: None,
                zones: vec![],
                react: None,
                ..spec()
            },
        );
        assert_eq!(
            m.direction,
            direction::LEFT,
            "up is not offered by an LR-only mode"
        );
    }

    #[test]
    fn colours_are_limited_to_the_modes_count_and_padded_up_to_its_minimum() {
        let c = |r| Rgb { r, g: 0, b: 0 };
        let m = apply(
            &mode(),
            &EffectSpec {
                colors: vec![c(9)],
                ..spec()
            },
        );
        assert_eq!(m.colors.len(), 2);
        assert!(m.colors.iter().all(|x| x.r == 9));
        let m = apply(
            &mode(),
            &EffectSpec {
                colors: vec![c(1), c(2), c(3), c(4)],
                ..spec()
            },
        );
        assert_eq!(m.colors.len(), 3);
        let none = apply(&mode(), &spec());
        assert_eq!(
            none.colors,
            mode().colors,
            "no colours given: keep the device's"
        );
        let mut per_led = mode();
        per_led.color_mode = color_mode::PER_LED;
        assert_eq!(
            apply(
                &per_led,
                &EffectSpec {
                    colors: vec![c(1)],
                    ..spec()
                }
            )
            .colors,
            per_led.colors
        );
    }
}
