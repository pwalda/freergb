// SPDX-License-Identifier: GPL-2.0-or-later
//! Typed payload helpers for the packets Phase 1 needs. Each `encode_*`
//! returns a payload; wrap it in a [`crate::Packet`] with the right IDs.
//! Profile/Settings JSON packets are added with the client crate.

use crate::device::{decode_color_list, encode_color_list, Color, Mode, Segment};
use crate::error::{ProtocolError, Result};
use crate::wire::{Reader, Writer};

fn u32_payload(v: u32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn read_u32_exact(p: &[u8]) -> Result<u32> {
    let mut r = Reader::new(p);
    let v = r.u32()?;
    r.finish()?;
    Ok(v)
}

/// Wrap a body in the `data_size`-prefixed form (size includes its own 4 bytes).
fn with_size(body: Vec<u8>) -> Result<Vec<u8>> {
    let total = u32::try_from(body.len() + 4)
        .map_err(|_| ProtocolError::TooLargeToEncode("packet body"))?;
    let mut out = total.to_le_bytes().to_vec();
    out.extend_from_slice(&body);
    Ok(out)
}

// ---- handshake ------------------------------------------------------------

pub fn encode_protocol_version(v: u32) -> Vec<u8> {
    u32_payload(v)
}

pub fn decode_protocol_version(p: &[u8]) -> Result<u32> {
    read_u32_exact(p)
}

pub fn encode_flags(flags: u32) -> Vec<u8> {
    u32_payload(flags)
}

pub fn decode_flags(p: &[u8]) -> Result<u32> {
    read_u32_exact(p)
}

pub fn encode_name(name: &str) -> Vec<u8> {
    let mut w = Writer::new();
    w.cstr(name);
    w.into_vec()
}

pub fn decode_name(p: &[u8]) -> Result<String> {
    Reader::new(p).rest_cstr()
}

// ---- controller list --------------------------------------------------------

/// `ids` is empty for protocol < 6 (count only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerList {
    pub count: u32,
    pub ids: Vec<u32>,
}

pub fn encode_controller_list(ids: &[u32], v: u32) -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(ids.len() as u32);
    if v >= 6 {
        for &id in ids {
            w.u32(id);
        }
    }
    w.into_vec()
}

pub fn decode_controller_list(p: &[u8], v: u32) -> Result<ControllerList> {
    let mut r = Reader::new(p);
    let count = r.u32()?;
    let mut ids = Vec::new();
    if v >= 6 {
        r.check_count(count as usize, 4)?;
        for _ in 0..count {
            ids.push(r.u32()?);
        }
    }
    r.finish()?;
    Ok(ControllerList { count, ids })
}

// ---- ack ------------------------------------------------------------------

pub fn encode_ack(acked_pkt_id: u32, status: u32) -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(acked_pkt_id);
    w.u32(status);
    w.into_vec()
}

pub fn decode_ack(p: &[u8]) -> Result<(u32, u32)> {
    let mut r = Reader::new(p);
    let out = (r.u32()?, r.u32()?);
    r.finish()?;
    Ok(out)
}

// ---- LED updates ------------------------------------------------------------

pub fn encode_update_leds(colors: &[Color]) -> Result<Vec<u8>> {
    with_size(encode_color_list(colors)?)
}

pub fn decode_update_leds(p: &[u8]) -> Result<Vec<Color>> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    decode_color_list(&mut r)
}

pub fn encode_update_zone_leds(zone_idx: u32, colors: &[Color]) -> Result<Vec<u8>> {
    let mut body = zone_idx.to_le_bytes().to_vec();
    body.extend_from_slice(&encode_color_list(colors)?);
    with_size(body)
}

pub fn decode_update_zone_leds(p: &[u8]) -> Result<(u32, Vec<Color>)> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let zone = r.u32()?;
    Ok((zone, decode_color_list(&mut r)?))
}

pub fn encode_update_single_led(led_idx: i32, color: Color) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(led_idx);
    color.write(&mut w);
    w.into_vec()
}

pub fn decode_update_single_led(p: &[u8]) -> Result<(i32, Color)> {
    let mut r = Reader::new(p);
    let out = (r.i32()?, Color::read(&mut r)?);
    r.finish()?;
    Ok(out)
}

// ---- zones / modes ------------------------------------------------------------

pub fn encode_resize_zone(zone_idx: i32, new_size: i32) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(zone_idx);
    w.i32(new_size);
    w.into_vec()
}

pub fn decode_resize_zone(p: &[u8]) -> Result<(i32, i32)> {
    let mut r = Reader::new(p);
    let out = (r.i32()?, r.i32()?);
    r.finish()?;
    Ok(out)
}

/// UPDATEMODE / SAVEMODE body: `data_size`, `mode_idx`, mode data.
pub fn encode_update_mode(mode_idx: i32, mode: &Mode, v: u32) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.i32(mode_idx);
    mode.write(&mut w, v)?;
    with_size(w.into_vec())
}

pub fn decode_update_mode(p: &[u8], v: u32) -> Result<(i32, Mode)> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let idx = r.i32()?;
    Ok((idx, Mode::read(&mut r, v)?))
}

/// UPDATEZONEMODE body (protocol >= 6): `data_size`, `zone_idx`, `mode_idx`, then the mode's
/// data. `mode_idx` -1 (`mode` `None`) puts the zone back on the device's mode; the server
/// reads no mode data then (`ProcessRequest_RGBController_UpdateZoneMode`).
pub fn encode_update_zone_mode(
    zone_idx: i32,
    mode_idx: i32,
    mode: Option<&Mode>,
    v: u32,
) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.i32(zone_idx);
    w.i32(mode_idx);
    if let Some(m) = mode.filter(|_| mode_idx >= 0) {
        m.write(&mut w, v)?;
    }
    with_size(w.into_vec())
}

pub fn decode_update_zone_mode(p: &[u8], v: u32) -> Result<(i32, i32, Option<Mode>)> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let zone = r.i32()?;
    let idx = r.i32()?;
    let mode = if idx >= 0 {
        Some(Mode::read(&mut r, v)?)
    } else {
        None
    };
    r.finish()?;
    Ok((zone, idx, mode))
}

/// CLEARSEGMENTS body: the zone index alone (the server checks the size is exactly 4).
pub fn encode_clear_segments(zone_idx: i32) -> Vec<u8> {
    zone_idx.to_le_bytes().to_vec()
}

pub fn decode_clear_segments(p: &[u8]) -> Result<i32> {
    let mut r = Reader::new(p);
    let zone = r.i32()?;
    r.finish()?;
    Ok(zone)
}

/// ADDSEGMENT body: `data_size`, `zone_idx`, then the segment's description.
pub fn encode_add_segment(zone_idx: i32, segment: &Segment, v: u32) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.i32(zone_idx);
    segment.write(&mut w, v)?;
    with_size(w.into_vec())
}

pub fn decode_add_segment(p: &[u8], v: u32) -> Result<(i32, Segment)> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let zone = r.i32()?;
    let s = Segment::read(&mut r, v)?;
    r.finish()?;
    Ok((zone, s))
}

pub fn encode_set_hidden(hidden: bool) -> Vec<u8> {
    vec![hidden as u8]
}

pub fn decode_set_hidden(p: &[u8]) -> Result<bool> {
    let mut r = Reader::new(p);
    let b = r.u8()?;
    r.finish()?;
    Ok(b != 0)
}

// ---- detection ---------------------------------------------------------------

/// DETECTION_PROGRESS_CHANGED body (protocol 6): `data_size`, percent (u32, 0..100), then a
/// `string16` naming the detector that is running.
pub fn encode_detection_progress(percent: u32, label: &str) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u32(percent);
    w.string16(label)?;
    with_size(w.into_vec())
}

pub fn decode_detection_progress(p: &[u8]) -> Result<(u32, String)> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let percent = r.u32()?;
    let label = r.string16()?;
    r.finish()?;
    Ok((percent, label))
}

// ---- profile manager / settings (string payloads) ---------------------------

/// PROFILE_GET_LIST response and PROFILE_LIST_UPDATED body:
/// `data_size`, `num_profiles` (u16), then `string16` names.
pub fn encode_profile_list(names: &[String]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u16(u16::try_from(names.len()).map_err(|_| ProtocolError::TooLargeToEncode("profile list"))?);
    for n in names {
        w.string16(n)?;
    }
    with_size(w.into_vec())
}

pub fn decode_profile_list(p: &[u8]) -> Result<Vec<String>> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let n = r.u16()? as usize;
    r.check_count(n, 2)?;
    let names = (0..n).map(|_| r.string16()).collect::<Result<Vec<_>>>()?;
    Ok(names)
}

/// Payloads that are a single NUL-terminated string filling the packet:
/// profile names, settings keys, profile/settings JSON, active profile.
pub fn encode_cstring(s: &str) -> Vec<u8> {
    encode_name(s)
}

pub fn decode_cstring(p: &[u8]) -> Result<String> {
    decode_name(p)
}
