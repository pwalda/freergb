// SPDX-License-Identifier: GPL-2.0-or-later
//! Fake device builders. Shapes follow the spec's list: motherboard with ARGB
//! headers, keyboard, RAM sticks, resizable strip, a 300-LED device.

use freergb_protocol::ids::*;
use freergb_protocol::{Color, ControllerData, Led, MatrixMap, Mode, Zone};

fn mode(name: &str, flags: u32, color_mode: u32) -> Mode {
    Mode {
        name: name.into(),
        flags,
        speed_min: 0,
        speed_max: 100,
        brightness_min: 0,
        brightness_max: 100,
        colors_min: 0,
        colors_max: 0,
        speed: 50,
        brightness: 100,
        direction: direction::LEFT,
        color_mode,
        ..Default::default()
    }
}

pub fn standard_modes() -> Vec<Mode> {
    vec![
        mode(
            "Direct",
            mode_flag::HAS_PER_LED_COLOR | mode_flag::HAS_BRIGHTNESS,
            color_mode::PER_LED,
        ),
        Mode {
            colors_min: 1,
            colors_max: 1,
            colors: vec![Color::new(255, 0, 0)],
            ..mode(
                "Static",
                mode_flag::HAS_MODE_SPECIFIC_COLOR | mode_flag::HAS_BRIGHTNESS,
                color_mode::MODE_SPECIFIC,
            )
        },
        mode(
            "Rainbow Wave",
            mode_flag::HAS_SPEED | mode_flag::HAS_DIRECTION_LR | mode_flag::HAS_BRIGHTNESS,
            color_mode::NONE,
        ),
    ]
}

/// `c` with two extra modes that real hardware has and `standard_modes` lacks: one whose speed
/// range is inverted (min above max) with two-to-three mode colours and up/down plus left/right
/// directions that the device can keep in its own memory, and an "Off" mode that takes nothing.
pub fn with_rich_modes(mut c: ControllerData) -> ControllerData {
    c.modes.push(Mode {
        speed_min: 100,
        speed_max: 10,
        speed: 55,
        colors_min: 2,
        colors_max: 3,
        colors: vec![Color::new(255, 0, 0), Color::new(0, 0, 255)],
        direction: direction::UP,
        ..mode(
            "Breathing",
            mode_flag::HAS_SPEED
                | mode_flag::HAS_BRIGHTNESS
                | mode_flag::HAS_DIRECTION_LR
                | mode_flag::HAS_DIRECTION_UD
                | mode_flag::HAS_MODE_SPECIFIC_COLOR
                | mode_flag::MANUAL_SAVE,
            color_mode::MODE_SPECIFIC,
        )
    });
    c.modes.push(mode("Off", 0, color_mode::NONE));
    c
}

fn led_name(zone: &str, i: usize) -> String {
    format!("{zone} LED {i}")
}

/// Rebuild `leds`/`colors` from the zone sizes (what real controllers do after a resize).
pub fn rebuild_leds(c: &mut ControllerData) {
    let old = std::mem::take(&mut c.colors);
    c.leds = c
        .zones
        .iter()
        .flat_map(|z| {
            (0..z.leds_count as usize).map(move |i| Led {
                name: led_name(&z.name, i),
                value: 0,
            })
        })
        .collect();
    c.colors = (0..c.leds.len())
        .map(|i| old.get(i).copied().unwrap_or_default())
        .collect();
}

pub fn zone(name: &str, zone_type: i32, min: u32, max: u32, count: u32, flags: u32) -> Zone {
    Zone {
        name: name.into(),
        display_name: name.into(),
        zone_type,
        leds_min: min,
        leds_max: max,
        leds_count: count,
        flags,
        ..Default::default()
    }
}

fn device(
    device_type: i32,
    name: &str,
    vendor: &str,
    serial: &str,
    zones: Vec<Zone>,
) -> ControllerData {
    let mut c = ControllerData {
        device_type,
        name: name.into(),
        display_name: name.into(),
        vendor: vendor.into(),
        description: format!("Mock {name}"),
        version: "1.0".into(),
        serial: serial.into(),
        location: format!("mock:{serial}"),
        modes: standard_modes(),
        zones,
        flags: controller_flag::LOCAL,
        configuration: "{}".into(),
        ..Default::default()
    };
    rebuild_leds(&mut c);
    c
}

pub fn motherboard() -> ControllerData {
    device(
        device_type::MOTHERBOARD,
        "Mock Motherboard",
        "MockCorp",
        "MB001",
        vec![
            zone("Chipset", zone_type::SINGLE, 1, 1, 1, 0),
            zone(
                "ARGB Header 1",
                zone_type::LINEAR,
                0,
                120,
                30,
                zone_flag::CONFIGURABLE_SIZE,
            ),
            zone(
                "ARGB Header 2",
                zone_type::LINEAR,
                0,
                120,
                30,
                zone_flag::CONFIGURABLE_SIZE,
            ),
        ],
    )
}

pub fn ram_stick(slot: u32) -> ControllerData {
    device(
        device_type::DRAM,
        &format!("Mock RAM {slot}"),
        "MockMem",
        &format!("RAM{slot:03}"),
        vec![zone("RAM", zone_type::LINEAR, 5, 5, 5, 0)],
    )
}

pub fn keyboard() -> ControllerData {
    let mut z = zone("Keys", zone_type::MATRIX, 12, 12, 12, 0);
    z.matrix_map = Some(MatrixMap {
        height: 3,
        width: 5,
        data: vec![
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            9,
            u32::MAX,
            10,
            11,
            u32::MAX,
            u32::MAX,
        ],
    });
    device(
        device_type::KEYBOARD,
        "Mock Keyboard",
        "MockKeys",
        "KB001",
        vec![z],
    )
}

pub fn resizable_strip() -> ControllerData {
    device(
        device_type::LEDSTRIP,
        "Mock Strip",
        "MockLight",
        "STRIP01",
        vec![zone(
            "Strip",
            zone_type::LINEAR,
            1,
            300,
            60,
            zone_flag::CONFIGURABLE_SIZE,
        )],
    )
}

pub fn big_300() -> ControllerData {
    device(
        device_type::LEDSTRIP,
        "Mock Big 300",
        "MockLight",
        "BIG300",
        vec![zone("All", zone_type::LINEAR, 300, 300, 300, 0)],
    )
}

/// The device the "disappears mid-session" scenario removes.
pub fn flaky() -> ControllerData {
    device(
        device_type::ACCESSORY,
        "Mock Flaky USB",
        "MockUSB",
        "FLAKY01",
        vec![zone("Pad", zone_type::SINGLE, 1, 1, 1, 0)],
    )
}

/// A fan hub whose channels each run their own built-in mode (OpenRGB 1.1 per-zone modes, as
/// NZXT Hue 2 or a Corsair Lighting Node report them). Every zone starts on the device's mode
/// (`active_mode` -1). "Super Rainbow" drives the whole hub, so no zone may differ under it.
pub fn fan_hub() -> ControllerData {
    let zone_modes = || {
        vec![
            Mode {
                colors_min: 1,
                colors_max: 1,
                colors: vec![Color::new(0, 128, 255)],
                ..mode(
                    "Fixed",
                    mode_flag::HAS_MODE_SPECIFIC_COLOR,
                    color_mode::MODE_SPECIFIC,
                )
            },
            Mode {
                colors_min: 1,
                colors_max: 2,
                colors: vec![Color::new(255, 64, 0)],
                ..mode(
                    "Pulse",
                    mode_flag::HAS_SPEED | mode_flag::HAS_MODE_SPECIFIC_COLOR,
                    color_mode::MODE_SPECIFIC,
                )
            },
            mode("Spectrum", mode_flag::HAS_SPEED, color_mode::NONE),
        ]
    };
    let fans = (1..=3)
        .map(|n| Zone {
            active_mode: -1,
            modes: zone_modes(),
            ..zone(&format!("Fan {n}"), zone_type::LINEAR, 8, 8, 8, 0)
        })
        .collect();
    let mut c = device(
        device_type::COOLER,
        "Mock Fan Hub",
        "MockAir",
        "HUB001",
        fans,
    );
    c.modes.push(mode(
        "Super Rainbow",
        mode_flag::HAS_SPEED | mode_flag::REQUIRES_ENTIRE_DEVICE,
        color_mode::NONE,
    ));
    c
}

pub fn default_set() -> Vec<ControllerData> {
    vec![
        motherboard(),
        keyboard(),
        ram_stick(0),
        ram_stick(1),
        resizable_strip(),
        big_300(),
    ]
}
