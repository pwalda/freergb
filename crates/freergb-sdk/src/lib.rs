// SPDX-License-Identifier: GPL-2.0-or-later
//! Async OpenRGB SDK client. Requires protocol 6 (stable controller IDs, ACKs,
//! ProfileManager/SettingsManager); older servers are refused with a clear error.
//!
//! Behaviour here is pinned to what a real OpenRGB 1.1 server does
//! (docs/UPSTREAM-FACTS.md), not just to the SDK doc: every request is ACKed
//! after its reply, `PROFILE_ABOUT_TO_LOAD` must be answered by echoing an
//! empty 159 packet, and `ResizeZone` on an unflagged zone is a silent no-op.

mod backend;
mod client;
mod error;
mod identity;
mod settings;

pub use backend::RgbBackend;
pub use client::{Client, ConnectOptions, Event};
pub use error::{Result, SdkError};
pub use freergb_protocol::{diag, ids, Color, ControllerData, Mode, Segment};
pub use identity::{keys_of, resolve, DeviceKey};
