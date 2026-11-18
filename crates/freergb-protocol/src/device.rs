// SPDX-License-Identifier: GPL-2.0-or-later
//! Controller ("device") description blocks, version-aware for protocols 0-6.
//! Layout verified against OpenRGB release_1.1 `RGBController::Get*DescriptionData`.

use crate::error::{ProtocolError, Result};
use crate::wire::{Reader, Writer};

/// RGBColor on the wire is 4 bytes: R, G, B, then one unused byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn write(&self, w: &mut Writer) {
        w.bytes(&[self.r, self.g, self.b, 0]);
    }

    pub fn read(r: &mut Reader) -> Result<Self> {
        let b = r.bytes(4)?;
        Ok(Self {
            r: b[0],
            g: b[1],
            b: b[2],
        })
    }
}

fn write_colors16(w: &mut Writer, colors: &[Color]) -> Result<()> {
    let n =
        u16::try_from(colors.len()).map_err(|_| ProtocolError::TooLargeToEncode("color list"))?;
    w.u16(n);
    for c in colors {
        c.write(w);
    }
    Ok(())
}

fn read_colors16(r: &mut Reader) -> Result<Vec<Color>> {
    let n = r.u16()? as usize;
    r.check_count(n, 4)?;
    (0..n).map(|_| Color::read(r)).collect()
}

/// Color list as carried by UPDATELEDS bodies and SIGNALUPDATE(UPDATELEDS).
pub fn encode_color_list(colors: &[Color]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    write_colors16(&mut w, colors)?;
    Ok(w.into_vec())
}

pub fn decode_color_list(r: &mut Reader) -> Result<Vec<Color>> {
    read_colors16(r)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mode {
    pub name: String,
    /// Only on the wire for protocol < 6; decoded as 0 otherwise.
    pub value: i32,
    pub flags: u32,
    pub speed_min: u32,
    pub speed_max: u32,
    pub brightness_min: u32,
    pub brightness_max: u32,
    pub colors_min: u32,
    pub colors_max: u32,
    pub speed: u32,
    pub brightness: u32,
    pub direction: u32,
    pub color_mode: u32,
    pub colors: Vec<Color>,
}

impl Mode {
    pub fn write(&self, w: &mut Writer, v: u32) -> Result<()> {
        w.string16(&self.name)?;
        if v < 6 {
            w.i32(self.value);
        }
        w.u32(self.flags);
        w.u32(self.speed_min);
        w.u32(self.speed_max);
        if v >= 3 {
            w.u32(self.brightness_min);
            w.u32(self.brightness_max);
        }
        w.u32(self.colors_min);
        w.u32(self.colors_max);
        w.u32(self.speed);
        if v >= 3 {
            w.u32(self.brightness);
        }
        w.u32(self.direction);
        w.u32(self.color_mode);
        write_colors16(w, &self.colors)
    }

    pub fn read(r: &mut Reader, v: u32) -> Result<Self> {
        let mut m = Mode {
            name: r.string16()?,
            ..Default::default()
        };
        if v < 6 {
            m.value = r.i32()?;
        }
        m.flags = r.u32()?;
        m.speed_min = r.u32()?;
        m.speed_max = r.u32()?;
        if v >= 3 {
            m.brightness_min = r.u32()?;
            m.brightness_max = r.u32()?;
        }
        m.colors_min = r.u32()?;
        m.colors_max = r.u32()?;
        m.speed = r.u32()?;
        if v >= 3 {
            m.brightness = r.u32()?;
        }
        m.direction = r.u32()?;
        m.color_mode = r.u32()?;
        m.colors = read_colors16(r)?;
        Ok(m)
    }
}

/// Row-major LED index grid; `u32::MAX` marks an empty cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixMap {
    pub height: u32,
    pub width: u32,
    pub data: Vec<u32>,
}

/// Encoded as a `u16` byte length (0 = no map) followed by the map.
fn write_matrix(w: &mut Writer, m: &Option<MatrixMap>) -> Result<()> {
    match m {
        None => w.u16(0),
        // Upstream treats a 0x0 map with no data as "empty" and writes length 0.
        Some(m) if m.height == 0 && m.width == 0 && m.data.is_empty() => w.u16(0),
        Some(m) => {
            let cells = (m.height as usize)
                .checked_mul(m.width as usize)
                .filter(|&c| c == m.data.len())
                .ok_or(ProtocolError::InvalidMatrixMap(
                    "data length != height*width",
                ))?;
            let len = u16::try_from(8 + 4 * cells)
                .map_err(|_| ProtocolError::TooLargeToEncode("matrix map (max 16383 cells)"))?;
            w.u16(len);
            w.u32(m.height);
            w.u32(m.width);
            for &c in &m.data {
                w.u32(c);
            }
        }
    }
    Ok(())
}

fn read_matrix(r: &mut Reader) -> Result<Option<MatrixMap>> {
    let len = r.u16()? as usize;
    if len == 0 {
        return Ok(None);
    }
    if len < 8 {
        return Err(ProtocolError::InvalidMatrixMap("length < 8"));
    }
    let body = r.bytes(len)?;
    let mut br = Reader::new(body);
    let height = br.u32()?;
    let width = br.u32()?;
    let cells = (height as usize)
        .checked_mul(width as usize)
        .ok_or(ProtocolError::InvalidMatrixMap("height*width overflow"))?;
    if cells.checked_mul(4) != Some(len - 8) {
        return Err(ProtocolError::InvalidMatrixMap(
            "length does not match height*width",
        ));
    }
    let data = (0..cells).map(|_| br.u32()).collect::<Result<Vec<_>>>()?;
    Ok(Some(MatrixMap {
        height,
        width,
        data,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Segment {
    pub name: String,
    pub segment_type: i32,
    pub start_idx: u32,
    pub leds_count: u32,
    /// protocol >= 6
    pub matrix_map: Option<MatrixMap>,
    /// protocol >= 6
    pub flags: u32,
}

impl Segment {
    pub fn write(&self, w: &mut Writer, v: u32) -> Result<()> {
        w.string16(&self.name)?;
        w.i32(self.segment_type);
        w.u32(self.start_idx);
        w.u32(self.leds_count);
        if v >= 6 {
            write_matrix(w, &self.matrix_map)?;
            w.u32(self.flags);
        }
        Ok(())
    }

    pub fn read(r: &mut Reader, v: u32) -> Result<Self> {
        let mut s = Segment {
            name: r.string16()?,
            segment_type: r.i32()?,
            start_idx: r.u32()?,
            leds_count: r.u32()?,
            ..Default::default()
        };
        if v >= 6 {
            s.matrix_map = read_matrix(r)?;
            s.flags = r.u32()?;
        }
        Ok(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Zone {
    pub name: String,
    pub zone_type: i32,
    pub leds_min: u32,
    pub leds_max: u32,
    pub leds_count: u32,
    pub matrix_map: Option<MatrixMap>,
    /// protocol >= 4
    pub segments: Vec<Segment>,
    /// protocol >= 5
    pub flags: u32,
    /// protocol >= 6
    pub active_mode: i32,
    /// protocol >= 6 (per-zone modes)
    pub modes: Vec<Mode>,
    /// protocol >= 6
    pub display_name: String,
}

impl Zone {
    pub fn write(&self, w: &mut Writer, v: u32) -> Result<()> {
        w.string16(&self.name)?;
        w.i32(self.zone_type);
        w.u32(self.leds_min);
        w.u32(self.leds_max);
        w.u32(self.leds_count);
        write_matrix(w, &self.matrix_map)?;
        if v >= 4 {
            let n = u16::try_from(self.segments.len())
                .map_err(|_| ProtocolError::TooLargeToEncode("segments"))?;
            w.u16(n);
            for s in &self.segments {
                s.write(w, v)?;
            }
        }
        if v >= 5 {
            w.u32(self.flags);
        }
        if v >= 6 {
            w.i32(self.active_mode);
            let n = u16::try_from(self.modes.len())
                .map_err(|_| ProtocolError::TooLargeToEncode("zone modes"))?;
            w.u16(n);
            for m in &self.modes {
                m.write(w, v)?;
            }
            w.string16(&self.display_name)?;
        }
        Ok(())
    }

    pub fn read(r: &mut Reader, v: u32) -> Result<Self> {
        let mut z = Zone {
            name: r.string16()?,
            zone_type: r.i32()?,
            leds_min: r.u32()?,
            leds_max: r.u32()?,
            leds_count: r.u32()?,
            matrix_map: read_matrix(r)?,
            ..Default::default()
        };
        if v >= 4 {
            let n = r.u16()? as usize;
            r.check_count(n, 2)?;
            z.segments = (0..n).map(|_| Segment::read(r, v)).collect::<Result<_>>()?;
        }
        if v >= 5 {
            z.flags = r.u32()?;
        }
        if v >= 6 {
            z.active_mode = r.i32()?;
            let n = r.u16()? as usize;
            r.check_count(n, 2)?;
            z.modes = (0..n).map(|_| Mode::read(r, v)).collect::<Result<_>>()?;
            z.display_name = r.string16()?;
        }
        Ok(z)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Led {
    pub name: String,
    /// Only on the wire for protocol < 6; decoded as 0 otherwise.
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControllerData {
    pub device_type: i32,
    pub name: String,
    /// protocol >= 1
    pub vendor: String,
    pub description: String,
    pub version: String,
    pub serial: String,
    pub location: String,
    pub active_mode: i32,
    pub modes: Vec<Mode>,
    pub zones: Vec<Zone>,
    pub leds: Vec<Led>,
    pub colors: Vec<Color>,
    /// protocol >= 5
    pub led_display_names: Vec<String>,
    /// protocol >= 5
    pub flags: u32,
    /// protocol >= 6
    pub display_name: String,
    /// protocol >= 6; JSON string
    pub configuration: String,
}

impl ControllerData {
    /// Encode the bare device block (no leading `data_size`).
    pub fn encode_block(&self, v: u32) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        w.i32(self.device_type);
        w.string16(&self.name)?;
        if v >= 1 {
            w.string16(&self.vendor)?;
        }
        w.string16(&self.description)?;
        w.string16(&self.version)?;
        w.string16(&self.serial)?;
        w.string16(&self.location)?;
        let n = |len: usize, what| {
            u16::try_from(len).map_err(|_| ProtocolError::TooLargeToEncode(what))
        };
        w.u16(n(self.modes.len(), "modes")?);
        w.i32(self.active_mode);
        for m in &self.modes {
            m.write(&mut w, v)?;
        }
        w.u16(n(self.zones.len(), "zones")?);
        for z in &self.zones {
            z.write(&mut w, v)?;
        }
        w.u16(n(self.leds.len(), "leds")?);
        for l in &self.leds {
            w.string16(&l.name)?;
            if v < 6 {
                w.u32(l.value);
            }
        }
        write_colors16(&mut w, &self.colors)?;
        if v >= 5 {
            w.u16(n(self.led_display_names.len(), "led display names")?);
            for s in &self.led_display_names {
                w.string16(s)?;
            }
            w.u32(self.flags);
        }
        if v >= 6 {
            w.string16(&self.display_name)?;
            w.string32(&self.configuration)?;
        }
        Ok(w.into_vec())
    }

    pub fn decode_block(r: &mut Reader, v: u32) -> Result<Self> {
        let mut c = ControllerData {
            device_type: r.i32()?,
            name: r.string16()?,
            ..Default::default()
        };
        if v >= 1 {
            c.vendor = r.string16()?;
        }
        c.description = r.string16()?;
        c.version = r.string16()?;
        c.serial = r.string16()?;
        c.location = r.string16()?;
        let num_modes = r.u16()? as usize;
        c.active_mode = r.i32()?;
        r.check_count(num_modes, 2)?;
        c.modes = (0..num_modes)
            .map(|_| Mode::read(r, v))
            .collect::<Result<_>>()?;
        let num_zones = r.u16()? as usize;
        r.check_count(num_zones, 2)?;
        c.zones = (0..num_zones)
            .map(|_| Zone::read(r, v))
            .collect::<Result<_>>()?;
        let num_leds = r.u16()? as usize;
        r.check_count(num_leds, 2)?;
        c.leds = (0..num_leds)
            .map(|_| {
                let name = r.string16()?;
                let value = if v < 6 { r.u32()? } else { 0 };
                Ok(Led { name, value })
            })
            .collect::<Result<_>>()?;
        c.colors = read_colors16(r)?;
        if v >= 5 {
            let n = r.u16()? as usize;
            r.check_count(n, 2)?;
            c.led_display_names = (0..n).map(|_| r.string16()).collect::<Result<_>>()?;
            c.flags = r.u32()?;
        }
        if v >= 6 {
            c.display_name = r.string16()?;
            c.configuration = r.string32()?;
        }
        Ok(c)
    }

    /// Full REQUEST_CONTROLLER_DATA response payload: `data_size` (which
    /// includes its own 4 bytes) followed by the device block.
    pub fn encode_packet(&self, v: u32) -> Result<Vec<u8>> {
        let block = self.encode_block(v)?;
        let total = u32::try_from(block.len() + 4)
            .map_err(|_| ProtocolError::TooLargeToEncode("controller data"))?;
        let mut out = Vec::with_capacity(block.len() + 4);
        out.extend_from_slice(&total.to_le_bytes());
        out.extend_from_slice(&block);
        Ok(out)
    }

    pub fn decode_packet(payload: &[u8], v: u32) -> Result<Self> {
        let mut r = Reader::new(payload);
        let declared = r.u32()? as usize;
        if declared > payload.len() {
            return Err(ProtocolError::Truncated {
                needed: declared - payload.len(),
                available: payload.len(),
            });
        }
        let c = Self::decode_block(&mut r, v)?;
        // Servers may pad; tolerate trailing bytes inside the declared size.
        Ok(c)
    }
}
