// SPDX-License-Identifier: GPL-2.0-or-later
//! Looks that follow a reading: one of a look's settings (brightness, colour or amount) moves
//! with a temperature, a load, the sound or the screen. Mirrored by the webview preview
//! (`ui/src/lib/react.ts`) and pinned for both by `tests/vectors.json`.
//!
//! A frame is rendered in two steps. [`frame_spec`] folds single-number readings into the
//! look's own brightness or amount once per frame; [`per_led`] then recolours or dims each
//! rendered LED for the readings that differ per LED (the screen, the sound spectrum) and for
//! a reading that drives the colour. A reading that is not there (the source is starting, or
//! the machine has no such sensor) leaves the look as it is.

use crate::layout::Point;
use crate::spec::{EffectSpec, Rgb};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// What a look can follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    /// Degrees Celsius.
    CpuTemp,
    GpuTemp,
    /// Percent, 0..100.
    CpuLoad,
    GpuLoad,
    /// Loudness of what the PC plays, 0..1.
    Sound,
    /// A pulse on each kick or bass hit that fades out, 0..1.
    Beat,
    /// Loudness per frequency band, low to high, each 0..1; per LED along the canvas.
    Spectrum,
    /// The screen's colours; per LED by its place on the desk map.
    Screen,
}

/// Which of the look's settings a reading moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drive {
    /// From off at the low end of the range to the look's brightness at the high end.
    Brightness,
    /// A point along the look's colours: the first at the low end, the last at the high end.
    Color,
    /// From a tenth of the look's amount (density) to all of it.
    Amount,
}

/// A look's link to a reading.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct React {
    pub source: SourceId,
    pub drives: Drive,
    /// The reading at which the setting is at its low and its high end; unset takes the
    /// source's own range ([`SourceId::range`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
    /// 0..1, how readily the setting moves: the reading is amplified (above 0.5) or damped
    /// (below) before it is placed in the range. Unset is 0.5, the reading as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<f64>,
    /// 0..1, how far the setting swings: below 1, brightness and amount no longer fall all the
    /// way and colour no longer travels the whole palette. Unset is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
}

impl SourceId {
    /// The usual span of the reading: what `low` and `high` default to.
    pub fn range(self) -> (f64, f64) {
        match self {
            SourceId::CpuTemp | SourceId::GpuTemp => (35.0, 85.0),
            SourceId::CpuLoad | SourceId::GpuLoad => (0.0, 100.0),
            SourceId::Sound | SourceId::Beat | SourceId::Spectrum | SourceId::Screen => (0.0, 1.0),
        }
    }

    /// Whether the reading is a colour or a level per LED rather than one number.
    pub fn per_led(self) -> bool {
        matches!(self, SourceId::Spectrum | SourceId::Screen)
    }

    /// The settings this reading can move: the screen gives colours, the spectrum levels.
    pub fn drives(self, d: Drive) -> bool {
        match self {
            SourceId::Screen => d == Drive::Color,
            SourceId::Spectrum => d == Drive::Brightness,
            _ => true,
        }
    }
}

impl React {
    pub fn range(&self) -> (f64, f64) {
        let (lo, hi) = self.source.range();
        (self.low.unwrap_or(lo), self.high.unwrap_or(hi))
    }

    /// `v` placed in the range after the sensitivity, 0..1.
    pub fn level(&self, v: f64) -> f64 {
        let (lo, hi) = self.range();
        if hi > lo {
            let placed = (v - lo) / (hi - lo);
            // 0 damps the reading to a quarter, 1 amplifies it four times.
            match self.sensitivity {
                Some(s) => placed * 4f64.powf(2.0 * s - 1.0),
                None => placed,
            }
            .clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// A level as a factor on brightness or amount: at strength `s` it swings over 1-s..1.
    pub fn swing(&self, level: f64) -> f64 {
        self.strength.map_or(level, |s| 1.0 - s * (1.0 - level))
    }

    /// A level as a place along the palette: at strength `s` it travels 0..s.
    pub fn travel(&self, level: f64) -> f64 {
        self.strength.map_or(level, |s| level * s)
    }
}

/// The screen as a small grid of colours, row-major, top-left first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub colors: Vec<Rgb>,
}

/// The latest value of every source that runs; `None` for one that does not (or has no value
/// yet). One snapshot is used for a whole frame.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Readings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_temp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_temp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_load: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_load: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectrum: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<Screen>,
}

impl Readings {
    /// A single-number reading, `None` for per-LED sources or a missing value.
    pub fn value(&self, s: SourceId) -> Option<f64> {
        match s {
            SourceId::CpuTemp => self.cpu_temp,
            SourceId::GpuTemp => self.gpu_temp,
            SourceId::CpuLoad => self.cpu_load,
            SourceId::GpuLoad => self.gpu_load,
            SourceId::Sound => self.sound,
            SourceId::Beat => self.beat,
            SourceId::Spectrum | SourceId::Screen => None,
        }
        .filter(|v| v.is_finite())
    }

    /// Forget the value of a source that stopped.
    pub fn clear(&mut self, s: SourceId) {
        match s {
            SourceId::CpuTemp => self.cpu_temp = None,
            SourceId::GpuTemp => self.gpu_temp = None,
            SourceId::CpuLoad => self.cpu_load = None,
            SourceId::GpuLoad => self.gpu_load = None,
            SourceId::Sound => self.sound = None,
            SourceId::Beat => self.beat = None,
            SourceId::Spectrum => self.spectrum = None,
            SourceId::Screen => self.screen = None,
        }
    }
}

/// The look for this frame: a single-number reading that drives brightness or amount folded
/// into the look's own setting. Anything else is left for [`per_led`].
pub fn frame_spec<'a>(spec: &'a EffectSpec, r: &Readings) -> Cow<'a, EffectSpec> {
    let Some(m) = spec.react else {
        return Cow::Borrowed(spec);
    };
    let Some(level) = r.value(m.source).map(|v| m.level(v)) else {
        return Cow::Borrowed(spec);
    };
    match m.drives {
        Drive::Brightness => {
            let mut s = spec.clone();
            s.brightness *= m.swing(level);
            Cow::Owned(s)
        }
        Drive::Amount => {
            let mut s = spec.clone();
            s.density *= 0.1 + 0.9 * m.swing(level);
            Cow::Owned(s)
        }
        Drive::Color => Cow::Borrowed(spec),
    }
}

/// LED `c` (rendered from [`frame_spec`]) after the per-LED part of the link: `x` is its place
/// along the canvas, `desk` its place on the scene's desk area (0..1 both ways, y down).
pub fn per_led(spec: &EffectSpec, r: &Readings, c: Rgb, x: f64, desk: Point) -> Rgb {
    let Some(m) = spec.react else { return c };
    match (m.source, m.drives) {
        (SourceId::Spectrum, _) => match r.spectrum.as_deref() {
            Some(bands) if !bands.is_empty() => scale(c, m.swing(m.level(band_at(bands, x)))),
            _ => c,
        },
        (SourceId::Screen, _) => match &r.screen {
            Some(s) => match screen_at(s, desk) {
                Some(px) => tint(c, [px.r as f64, px.g as f64, px.b as f64]),
                None => c,
            },
            None => c,
        },
        (src, Drive::Color) => match r.value(src) {
            Some(v) => tint(c, palette(&spec.colors, m.travel(m.level(v)))),
            None => c,
        },
        _ => c,
    }
}

/// One LED of a look that may follow a reading: both steps at once (the engine folds
/// [`frame_spec`] once per frame instead).
pub fn render_led(spec: &EffectSpec, r: &Readings, t: f64, x: f64, y: f64, desk: Point) -> Rgb {
    let c = crate::render::render_at(&frame_spec(spec, r), t, x, y);
    per_led(spec, r, c, x, desk)
}

/// The level at `x` along the bands, interpolated between band centres.
fn band_at(bands: &[f64], x: f64) -> f64 {
    let n = bands.len();
    if n == 1 {
        return bands[0];
    }
    let pos = x.clamp(0.0, 1.0) * (n - 1) as f64;
    let i = (pos.floor() as usize).min(n - 2);
    let f = pos - i as f64;
    bands[i] + (bands[i + 1] - bands[i]) * f
}

/// The screen cell under `desk`, `None` for an empty grid.
fn screen_at(s: &Screen, desk: Point) -> Option<Rgb> {
    let (w, h) = (s.width as usize, s.height as usize);
    if w == 0 || h == 0 || s.colors.len() < w * h {
        return None;
    }
    let cell = |v: f64, n: usize| ((v.clamp(0.0, 1.0) * n as f64).floor() as usize).min(n - 1);
    Some(s.colors[cell(desk[1], h) * w + cell(desk[0], w)])
}

fn palette(colors: &[Rgb], u: f64) -> [f64; 3] {
    let f = |c: Rgb| [c.r as f64, c.g as f64, c.b as f64];
    match colors {
        [] => [255.0; 3],
        [only] => f(*only),
        _ => {
            let pos = u * (colors.len() - 1) as f64;
            let i = (pos.floor() as usize).min(colors.len() - 2);
            let (a, b, k) = (f(colors[i]), f(colors[i + 1]), pos - i as f64);
            [
                a[0] + (b[0] - a[0]) * k,
                a[1] + (b[1] - a[1]) * k,
                a[2] + (b[2] - a[2]) * k,
            ]
        }
    }
}

fn scale(c: Rgb, k: f64) -> Rgb {
    let ch = |v: u8| (v as f64 * k).round().clamp(0.0, 255.0) as u8;
    Rgb {
        r: ch(c.r),
        g: ch(c.g),
        b: ch(c.b),
    }
}

/// `to` at the lit level of `c` (its brightest channel), so motion and brightness stay.
fn tint(c: Rgb, to: [f64; 3]) -> Rgb {
    let lit = c.r.max(c.g).max(c.b) as f64 / 255.0;
    let ch = |v: f64| (v * lit).round().clamp(0.0, 255.0) as u8;
    Rgb {
        r: ch(to[0]),
        g: ch(to[1]),
        b: ch(to[2]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::EffectKind;

    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    fn look(source: SourceId, drives: Drive) -> EffectSpec {
        let mut s: EffectSpec =
            serde_json::from_str(r##"{"type":"static","colors":["#0000ff","#ff0000"]}"##).unwrap();
        s.react = Some(React {
            source,
            drives,
            low: None,
            high: None,
            sensitivity: None,
            strength: None,
        });
        s
    }

    #[test]
    fn sensitivity_scales_the_reading_and_strength_narrows_the_swing() {
        let mut s = look(SourceId::Sound, Drive::Brightness);
        let quiet = Readings {
            sound: Some(0.2),
            ..Default::default()
        };
        let m = s.react.as_mut().unwrap();
        m.sensitivity = Some(1.0);
        assert!((frame_spec(&s, &quiet).brightness - 0.8).abs() < 1e-12);
        let m = s.react.as_mut().unwrap();
        m.sensitivity = Some(0.0);
        m.strength = Some(0.4);
        // 0.2 damped to 0.05, then swinging over 0.6..1.
        assert!((frame_spec(&s, &quiet).brightness - 0.62).abs() < 1e-12);
        let silent = Readings {
            sound: Some(0.0),
            ..Default::default()
        };
        assert!((frame_spec(&s, &silent).brightness - 0.6).abs() < 1e-12);
        let m = s.react.as_mut().unwrap();
        m.drives = Drive::Color;
        assert!((m.travel(1.0) - 0.4).abs() < 1e-12);
    }

    #[test]
    fn a_temperature_moves_brightness_or_amount_within_its_range() {
        let s = look(SourceId::CpuTemp, Drive::Brightness);
        let hot = Readings {
            cpu_temp: Some(60.0),
            ..Default::default()
        };
        // 60 °C is halfway through the default 35..85.
        assert_eq!(frame_spec(&s, &hot).brightness, 0.5);
        assert_eq!(frame_spec(&s, &Readings::default()).brightness, 1.0);
        let mut a = look(SourceId::CpuTemp, Drive::Amount);
        a.density = 2.0;
        a.react.as_mut().unwrap().high = Some(60.0);
        assert!((frame_spec(&a, &hot).density - 2.0).abs() < 1e-12);
        let cold = Readings {
            cpu_temp: Some(10.0),
            ..Default::default()
        };
        assert!((frame_spec(&a, &cold).density - 0.2).abs() < 1e-12);
    }

    #[test]
    fn a_reading_that_drives_colour_picks_along_the_palette_at_the_leds_level() {
        let s = look(SourceId::CpuLoad, Drive::Color);
        let r = Readings {
            cpu_load: Some(100.0),
            ..Default::default()
        };
        assert_eq!(
            per_led(&s, &r, rgb(255, 255, 255), 0.0, [0.0; 2]),
            rgb(255, 0, 0)
        );
        // A half-lit LED stays half lit.
        assert_eq!(
            per_led(&s, &r, rgb(0, 128, 0), 0.0, [0.0; 2]),
            rgb(128, 0, 0)
        );
        let idle = Readings {
            cpu_load: Some(0.0),
            ..Default::default()
        };
        assert_eq!(
            per_led(&s, &idle, rgb(255, 0, 0), 0.0, [0.0; 2]),
            rgb(0, 0, 255)
        );
    }

    #[test]
    fn the_spectrum_dims_each_led_by_its_band_and_the_screen_colours_it_by_its_spot() {
        let s = look(SourceId::Spectrum, Drive::Brightness);
        let r = Readings {
            spectrum: Some(vec![1.0, 0.0]),
            screen: Some(Screen {
                width: 2,
                height: 1,
                colors: vec![rgb(0, 255, 0), rgb(0, 0, 255)],
            }),
            ..Default::default()
        };
        let white = rgb(255, 255, 255);
        assert_eq!(per_led(&s, &r, white, 0.0, [0.0; 2]), white);
        assert_eq!(per_led(&s, &r, white, 0.5, [0.0; 2]), rgb(128, 128, 128));
        assert_eq!(per_led(&s, &r, white, 1.0, [0.0; 2]), rgb(0, 0, 0));
        let sc = look(SourceId::Screen, Drive::Color);
        assert_eq!(per_led(&sc, &r, white, 0.0, [0.9, 0.5]), rgb(0, 0, 255));
        assert_eq!(per_led(&sc, &r, white, 0.0, [0.1, 0.5]), rgb(0, 255, 0));
        // Nothing to follow yet: the look as it is.
        assert_eq!(
            per_led(&sc, &Readings::default(), white, 0.0, [0.1, 0.5]),
            white
        );
        assert_eq!(s.kind, EffectKind::Static);
    }
}
