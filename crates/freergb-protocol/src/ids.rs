// SPDX-License-Identifier: GPL-2.0-or-later
//! Packet IDs, flags and enum values. Verified against OpenRGB release_1.1
//! (see docs/UPSTREAM-FACTS.md). Enum-like fields stay plain integers so a
//! newer server's unknown values round-trip instead of failing to decode.

pub const MAGIC: [u8; 4] = *b"ORGB";
pub const DEFAULT_PORT: u16 = 6742;
/// Highest protocol version this crate implements (OpenRGB 1.1 speaks 6).
pub const MAX_PROTOCOL_VERSION: u32 = 6;
pub const HEADER_LEN: usize = 16;

pub mod pkt {
    pub const REQUEST_CONTROLLER_COUNT: u32 = 0;
    pub const REQUEST_CONTROLLER_DATA: u32 = 1;
    pub const ACK: u32 = 10;
    pub const REQUEST_PROTOCOL_VERSION: u32 = 40;
    pub const SET_CLIENT_NAME: u32 = 50;
    pub const SET_SERVER_NAME: u32 = 51;
    pub const SET_CLIENT_FLAGS: u32 = 52;
    pub const SET_SERVER_FLAGS: u32 = 53;
    pub const DEVICE_LIST_UPDATED: u32 = 100;
    pub const DETECTION_STARTED: u32 = 101;
    pub const DETECTION_PROGRESS_CHANGED: u32 = 102;
    pub const DETECTION_COMPLETE: u32 = 103;
    pub const GET_I2C_BUS_INFO: u32 = 120;
    pub const GET_HID_DEVICE_INFO: u32 = 121;
    pub const GET_USB_DEVICE_INFO: u32 = 122;
    pub const GET_SERIAL_PORTS: u32 = 123;
    pub const GET_USB_SERIAL_PORTS: u32 = 124;
    pub const REQUEST_RESCAN_DEVICES: u32 = 140;
    pub const PROFILE_GET_LIST: u32 = 150;
    pub const PROFILE_SAVE: u32 = 151;
    pub const PROFILE_LOAD: u32 = 152;
    pub const PROFILE_DELETE: u32 = 153;
    pub const PROFILE_UPLOAD: u32 = 154;
    pub const PROFILE_DOWNLOAD: u32 = 155;
    pub const PROFILE_GET_ACTIVE: u32 = 156;
    pub const PROFILE_ACTIVE_CHANGED: u32 = 157;
    pub const PROFILE_LOADED: u32 = 158;
    pub const PROFILE_ABOUT_TO_LOAD: u32 = 159;
    pub const PROFILE_LIST_UPDATED: u32 = 160;
    pub const PROFILE_CLEAR_ACTIVE: u32 = 161;
    pub const PLUGIN_GET_LIST: u32 = 200;
    pub const PLUGIN_SPECIFIC: u32 = 201;
    pub const SETTINGS_GET: u32 = 250;
    pub const SETTINGS_GET_SCHEMA: u32 = 251;
    pub const SETTINGS_MODIFY: u32 = 252;
    pub const SETTINGS_SET: u32 = 253;
    pub const SETTINGS_SAVE: u32 = 254;
    pub const LOG_CLEAR_BUFFER: u32 = 300;
    pub const LOG_GET_BUFFER: u32 = 301;
    pub const LOG_GET_LEVEL: u32 = 302;
    pub const LOG_SET_LEVEL: u32 = 303;
    pub const LOG_LOGGED_ENTRY: u32 = 304;
    pub const RESIZE_ZONE: u32 = 1000;
    pub const CLEAR_SEGMENTS: u32 = 1001;
    pub const ADD_SEGMENT: u32 = 1002;
    pub const CONFIGURE_ZONE: u32 = 1003;
    pub const CONFIGURE_DEVICE: u32 = 1004;
    pub const SET_HIDDEN: u32 = 1005;
    pub const UPDATE_LEDS: u32 = 1050;
    pub const UPDATE_ZONE_LEDS: u32 = 1051;
    pub const UPDATE_SINGLE_LED: u32 = 1052;
    pub const SET_CUSTOM_MODE: u32 = 1100;
    pub const UPDATE_MODE: u32 = 1101;
    pub const SAVE_MODE: u32 = 1102;
    pub const UPDATE_ZONE_MODE: u32 = 1103;
    pub const SET_DEVICE_SPECIFIC_CONFIGURATION: u32 = 1130;
    pub const SET_DEVICE_SPECIFIC_ZONE_CONFIGURATION: u32 = 1131;
    pub const SIGNAL_UPDATE: u32 = 1150;
}

pub mod status {
    pub const OK: u32 = 0;
    pub const ERROR_GENERIC: u32 = 1;
    pub const ERROR_UNSUPPORTED: u32 = 2;
    pub const ERROR_NOT_ALLOWED: u32 = 3;
    pub const ERROR_INVALID_ID: u32 = 4;
    pub const ERROR_INVALID_DATA: u32 = 5;
}

pub mod client_flag {
    pub const SUPPORTS_RGBCONTROLLER: u32 = 1 << 0;
    pub const SUPPORTS_LOGMANAGER: u32 = 1 << 1;
    pub const SUPPORTS_PROFILEMANAGER: u32 = 1 << 2;
    pub const SUPPORTS_PLUGINMANAGER: u32 = 1 << 3;
    pub const SUPPORTS_SETTINGSMANAGER: u32 = 1 << 4;
    pub const REQUEST_LOCAL_CLIENT: u32 = 1 << 16;
}

pub mod server_flag {
    pub const SUPPORTS_RGBCONTROLLER: u32 = 1 << 0;
    pub const SUPPORTS_LOGMANAGER: u32 = 1 << 1;
    pub const SUPPORTS_PROFILEMANAGER: u32 = 1 << 2;
    pub const SUPPORTS_PLUGINMANAGER: u32 = 1 << 3;
    pub const SUPPORTS_SETTINGSMANAGER: u32 = 1 << 4;
    pub const SUPPORTS_DETECTION: u32 = 1 << 5;
    pub const SUPPORTS_DEVICE_INFO: u32 = 1 << 6;
    pub const LOCAL_CLIENT: u32 = 1 << 16;
}

pub mod mode_flag {
    pub const HAS_SPEED: u32 = 1 << 0;
    pub const HAS_DIRECTION_LR: u32 = 1 << 1;
    pub const HAS_DIRECTION_UD: u32 = 1 << 2;
    pub const HAS_DIRECTION_HV: u32 = 1 << 3;
    pub const HAS_BRIGHTNESS: u32 = 1 << 4;
    pub const HAS_PER_LED_COLOR: u32 = 1 << 5;
    pub const HAS_MODE_SPECIFIC_COLOR: u32 = 1 << 6;
    pub const HAS_RANDOM_COLOR: u32 = 1 << 7;
    pub const MANUAL_SAVE: u32 = 1 << 8;
    pub const AUTOMATIC_SAVE: u32 = 1 << 9;
    pub const REQUIRES_ENTIRE_DEVICE: u32 = 1 << 10;
    pub const HAS_DIRECTION_DIAG: u32 = 1 << 11;
}

pub mod direction {
    pub const LEFT: u32 = 0;
    pub const RIGHT: u32 = 1;
    pub const UP: u32 = 2;
    pub const DOWN: u32 = 3;
    pub const HORIZONTAL: u32 = 4;
    pub const VERTICAL: u32 = 5;
    pub const UP_LEFT: u32 = 6;
    pub const UP_RIGHT: u32 = 7;
    pub const DOWN_LEFT: u32 = 8;
    pub const DOWN_RIGHT: u32 = 9;
}

pub mod color_mode {
    pub const NONE: u32 = 0;
    pub const PER_LED: u32 = 1;
    pub const MODE_SPECIFIC: u32 = 2;
    pub const RANDOM: u32 = 3;
}

pub mod zone_type {
    pub const SINGLE: i32 = 0;
    pub const LINEAR: i32 = 1;
    pub const MATRIX: i32 = 2;
    pub const LINEAR_LOOP: i32 = 3;
    pub const MATRIX_LOOP_X: i32 = 4;
    pub const MATRIX_LOOP_Y: i32 = 5;
    pub const SEGMENTED: i32 = 6;
}

pub mod zone_flag {
    pub const SIZE_EFFECTS_ONLY: u32 = 1 << 0;
    pub const CONFIGURABLE_SIZE: u32 = 1 << 1;
    pub const CONFIGURABLE_NAME: u32 = 1 << 2;
    pub const CONFIGURABLE_TYPE: u32 = 1 << 3;
    pub const CONFIGURABLE_MATRIX_MAP: u32 = 1 << 4;
    pub const CONFIGURABLE_SEGMENTS: u32 = 1 << 5;
    pub const CONFIGURABLE_DEVICE_SPECIFIC: u32 = 1 << 6;
    pub const GEOMETRY_MAY_CHANGE: u32 = 1 << 24;
}

pub mod device_type {
    pub const MOTHERBOARD: i32 = 0;
    pub const DRAM: i32 = 1;
    pub const GPU: i32 = 2;
    pub const COOLER: i32 = 3;
    pub const LEDSTRIP: i32 = 4;
    pub const KEYBOARD: i32 = 5;
    pub const MOUSE: i32 = 6;
    pub const MOUSEMAT: i32 = 7;
    pub const HEADSET: i32 = 8;
    pub const HEADSET_STAND: i32 = 9;
    pub const GAMEPAD: i32 = 10;
    pub const LIGHT: i32 = 11;
    pub const SPEAKER: i32 = 12;
    pub const VIRTUAL: i32 = 13;
    pub const STORAGE: i32 = 14;
    pub const CASE: i32 = 15;
    pub const MICROPHONE: i32 = 16;
    pub const ACCESSORY: i32 = 17;
    pub const KEYPAD: i32 = 18;
    pub const LAPTOP: i32 = 19;
    pub const MONITOR: i32 = 20;
    pub const UNKNOWN: i32 = 21;
}

pub mod controller_flag {
    pub const LOCAL: u32 = 1 << 0;
    pub const REMOTE: u32 = 1 << 1;
    pub const VIRTUAL: u32 = 1 << 2;
    pub const HIDDEN: u32 = 1 << 3;
    pub const RESET_BEFORE_UPDATE: u32 = 1 << 8;
}

/// Reason codes carried by `SIGNAL_UPDATE`, sequential from 0.
pub mod update_reason {
    pub const UPDATE_LEDS: u32 = 0;
    pub const UPDATE_MODE: u32 = 1;
    pub const SAVE_MODE: u32 = 2;
    pub const CONFIGURE_ZONE: u32 = 3;
    pub const CLEAR_SEGMENTS: u32 = 4;
    pub const ADD_SEGMENT: u32 = 5;
    pub const HIDDEN: u32 = 6;
    pub const UNHIDDEN: u32 = 7;
    pub const SET_DEVICE_SPECIFIC_CONFIGURATION: u32 = 8;
    pub const SET_DEVICE_SPECIFIC_ZONE_CONFIGURATION: u32 = 9;
    pub const CONFIGURE_DEVICE: u32 = 10;
    pub const DEVICE_CHANGED: u32 = 11;
}
