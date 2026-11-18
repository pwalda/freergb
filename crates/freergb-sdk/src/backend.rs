// SPDX-License-Identifier: GPL-2.0-or-later

use crate::error::Result;
use freergb_protocol::{Color, ControllerData, Mode};
use std::future::Future;

/// What the rest of the app (effects engine, profile manager, UI commands)
/// needs from an RGB backend. `Client` implements it over TCP; tests can
/// implement it in memory.
pub trait RgbBackend: Send + Sync {
    /// Snapshot of known devices as `(stable id, data)`, ordered by id.
    fn devices(&self) -> Vec<(u32, ControllerData)>;

    /// Streaming path for effects: waits only for a flow-control permit, not for the ACK.
    fn update_leds_nowait(
        &self,
        device: u32,
        colors: &[Color],
    ) -> impl Future<Output = Result<()>> + Send;

    fn update_leds(&self, device: u32, colors: &[Color])
        -> impl Future<Output = Result<()>> + Send;
    fn set_custom_mode(&self, device: u32) -> impl Future<Output = Result<()>> + Send;
    fn update_mode(
        &self,
        device: u32,
        mode_idx: u32,
        mode: &Mode,
    ) -> impl Future<Output = Result<()>> + Send;
    /// Give a zone its own mode (`mode_idx` into the zone's mode list, with the values in
    /// `mode`), or `-1` to put it back on the device's mode.
    fn update_zone_mode(
        &self,
        device: u32,
        zone: u32,
        mode_idx: i32,
        mode: Option<&Mode>,
    ) -> impl Future<Output = Result<()>> + Send;
    fn resize_zone(
        &self,
        device: u32,
        zone: u32,
        size: u32,
    ) -> impl Future<Output = Result<()>> + Send;
    fn list_profiles(&self) -> impl Future<Output = Result<Vec<String>>> + Send;
    fn save_profile(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn load_profile(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn delete_profile(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
}
