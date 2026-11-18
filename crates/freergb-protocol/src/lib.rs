// SPDX-License-Identifier: GPL-2.0-or-later
//! OpenRGB SDK wire protocol codec (protocol versions 0-6).
//!
//! Pure data and bytes: no I/O, no async. Used by both the SDK client and the
//! mock server so the two cannot drift apart. Verified against OpenRGB
//! `release_1.1`; see `docs/UPSTREAM-FACTS.md`.

pub mod device;
pub mod diag;
pub mod error;
pub mod header;
pub mod ids;
pub mod messages;
pub mod wire;

pub use device::{Color, ControllerData, Led, MatrixMap, Mode, Segment, Zone};
pub use error::{ProtocolError, Result};
pub use header::Packet;
