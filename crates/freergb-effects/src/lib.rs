// SPDX-License-Identifier: GPL-2.0-or-later
//! Effects engine.
//!
//! * [`spec`]: the declarative, JSON-serialisable effect format. The webview preview and this
//!   runtime share it; `tests/vectors.json` pins the maths for both.
//! * [`render`]: pure functions, `(effect, time, position) -> colour`. No I/O, no clock.
//! * [`layout`]: where each LED sits on the 0..1 canvas.
//! * [`react`]: a look's setting following a reading (temperature, load, sound, screen).
//! * [`engine`]: the frame loop that drives a backend at a capped frame rate.

pub mod engine;
pub mod hwmode;
pub mod layout;
pub mod react;
pub mod render;
pub mod spec;

pub use engine::{Engine, EngineConfig, Stats, StopBehavior};
pub use spec::{
    Canvas, Direction, EffectKind, EffectSpec, ModeDirection, Rgb, Scene, SpecError, Targets,
    ZoneLook,
};
