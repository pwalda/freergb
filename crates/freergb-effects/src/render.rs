// SPDX-License-Identifier: GPL-2.0-or-later
//! Pure effect maths. Every formula is mirrored by the webview preview; change one and
//! `tests/vectors.json` must be regenerated and the JS side updated with it.
//!
//! `t` is seconds since the scene started, `x` the canvas position in 0..=1 along the way the
//! effect moves and `y` across it (see `layout::canvas`; a strip has `y = 0`). Only starfield,
//! fire and rain use `y`; each reduces to its one-dimensional self at `y = 0`. Colours are
//! interpolated linearly in sRGB space (what LED hardware shows), in 0..255 floats, and
//! rounded once at the end.

use crate::layout::Point;
use crate::spec::{Direction, EffectKind, EffectSpec, Rgb};

type F3 = [f64; 3];

fn frac(v: f64) -> f64 {
    v - v.floor()
}

fn to_f3(c: Rgb) -> F3 {
    [c.r as f64, c.g as f64, c.b as f64]
}

fn lerp(a: F3, b: F3, u: f64) -> F3 {
    [
        a[0] + (b[0] - a[0]) * u,
        a[1] + (b[1] - a[1]) * u,
        a[2] + (b[2] - a[2]) * u,
    ]
}

/// Piecewise-linear palette lookup, `u` in 0..=1. One colour is a constant.
fn sample(palette: &[Rgb], u: f64) -> F3 {
    match palette {
        [] => [0.0; 3],
        [only] => to_f3(*only),
        _ => {
            let pos = u.clamp(0.0, 1.0) * (palette.len() - 1) as f64;
            let i = (pos.floor() as usize).min(palette.len() - 2);
            lerp(to_f3(palette[i]), to_f3(palette[i + 1]), pos - i as f64)
        }
    }
}

/// HSV with S = V = 1, hue in 0..1.
fn hue(h: f64) -> F3 {
    let h6 = frac(h) * 6.0;
    let f = h6 - h6.floor();
    let (r, g, b) = match h6.floor() as u32 {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    };
    [r * 255.0, g * 255.0, b * 255.0]
}

// The random-looking effects are pure functions of (t, x) too: they hash integer cells of
// time and space. Every step below is plain IEEE arithmetic or 32-bit integer maths, which JS
// reproduces exactly (`Math.imul`, `>>>`), so thresholds flip at the same instant on both sides.

/// A 32-bit integer hash of two words.
fn hash(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

/// A hash as a number in 0..1.
fn unit(h: u32) -> f64 {
    h as f64 / 4_294_967_296.0
}

/// `floor(v)` wrapped to 32 bits, like JS `Math.floor(v) | 0`.
fn cell(v: f64) -> u32 {
    v.floor() as i64 as u32
}

fn smooth(f: f64) -> f64 {
    f * f * (3.0 - 2.0 * f)
}

/// Smooth value noise in 0..1 over a lattice of hashed corners.
fn noise(a: f64, b: f64, salt: u32) -> f64 {
    let (ia, ib) = (cell(a), cell(b));
    let (fa, fb) = (smooth(a - a.floor()), smooth(b - b.floor()));
    let corner = |i: u32, j: u32| unit(hash(hash(i, salt), j));
    let top = corner(ia, ib) + (corner(ia.wrapping_add(1), ib) - corner(ia, ib)) * fa;
    let bottom = corner(ia, ib.wrapping_add(1))
        + (corner(ia.wrapping_add(1), ib.wrapping_add(1)) - corner(ia, ib.wrapping_add(1))) * fa;
    top + (bottom - top) * fb
}

fn pick(palette: &[Rgb], h: u32) -> F3 {
    match palette.len() {
        0 => [0.0; 3],
        n => to_f3(palette[h as usize % n]),
    }
}

/// Brightness of a tail `d` behind its head (0 at the head), `len` long.
fn tail(d: f64, len: f64) -> f64 {
    if (0.0..len).contains(&d) {
        let k = 1.0 - d / len;
        k * k
    } else {
        0.0
    }
}

const COMET_TAIL: f64 = 0.4;
const MARQUEE_DASHES: f64 = 8.0;
const BEAM_WIDTH: f64 = 0.15;
/// Star slots across the canvas: finer than any device's LED spacing, so neighbours differ.
const STAR_CELLS: f64 = 4096.0;
const FIRE_SCALE: f64 = 6.0;
const FIRE_RATE: f64 = 10.0;
const RAIN_TAIL: f64 = 0.12;
const RAIN_MAX: f64 = 32.0;
/// Rain lanes across the canvas: each lane has its own drops.
const RAIN_LANES: f64 = 12.0;

fn finish(c: F3, brightness: f64) -> Rgb {
    let ch = |v: f64| (v * brightness).round().clamp(0.0, 255.0) as u8;
    Rgb {
        r: ch(c[0]),
        g: ch(c[1]),
        b: ch(c[2]),
    }
}

/// The colour of one LED. `spec` is assumed valid (`EffectSpec::validate`); an invalid one
/// renders without panicking but its look is unspecified.
pub fn render_at(spec: &EffectSpec, t: f64, x: f64, y: f64) -> Rgb {
    let sign = match spec.direction {
        Direction::Forward => 1.0,
        Direction::Reverse => -1.0,
    };
    let c = match spec.kind {
        EffectKind::Static => sample(&spec.colors, 0.0),
        EffectKind::Gradient => {
            let u = if sign > 0.0 { x } else { 1.0 - x };
            sample(&spec.colors, u)
        }
        EffectKind::SpectrumCycle => hue(sign * spec.speed * t),
        EffectKind::Rainbow => hue(spec.density * x - sign * spec.speed * t),
        EffectKind::Breathing => {
            let cycles = spec.speed * t;
            let n = spec.colors.len().max(1);
            let idx = (cycles.floor().max(0.0) as usize) % n;
            let level = 0.5 - 0.5 * (std::f64::consts::TAU * frac(cycles)).cos();
            let base = spec.colors.get(idx).copied().map(to_f3).unwrap_or([0.0; 3]);
            lerp([0.0; 3], base, level)
        }
        EffectKind::Wave => {
            let phase = spec.density * x - sign * spec.speed * t;
            let u = 0.5 - 0.5 * (std::f64::consts::TAU * phase).cos();
            match spec.colors.as_slice() {
                [only] => lerp([0.0; 3], to_f3(*only), u),
                palette => sample(palette, u),
            }
        }
        EffectKind::Comet => {
            let d = frac(spec.speed * t - sign * spec.density * x);
            lerp(
                [0.0; 3],
                sample(&spec.colors, d / COMET_TAIL),
                tail(d, COMET_TAIL),
            )
        }
        EffectKind::Marquee => {
            let q = MARQUEE_DASHES * spec.density * x - sign * spec.speed * t;
            if frac(q) < 0.5 {
                let n = spec.colors.len().max(1) as i64;
                pick(&spec.colors, (q.floor() as i64).rem_euclid(n) as u32)
            } else {
                [0.0; 3]
            }
        }
        EffectKind::BouncingBeam => {
            let mut at = 0.5 - 0.5 * (std::f64::consts::TAU * spec.speed * t).cos();
            if sign < 0.0 {
                at = 1.0 - at;
            }
            let off = ((x - at).abs() / (BEAM_WIDTH / spec.density)).min(1.0);
            let level = 0.5 + 0.5 * (std::f64::consts::PI * off).cos();
            lerp([0.0; 3], sample(&spec.colors, at), level)
        }
        EffectKind::Starfield => {
            let k = cell(x * STAR_CELLS)
                .wrapping_add(cell(y * STAR_CELLS).wrapping_mul(STAR_CELLS as u32 + 1));
            let rate = spec.speed * (0.5 + unit(hash(k, 0x51A2)));
            let cycles = rate * t + unit(hash(k, 0x51A3));
            let h = hash(hash(k, cell(cycles)), 0x51A4);
            if unit(h) < (0.25 * spec.density).min(1.0) {
                let s = (std::f64::consts::PI * frac(cycles)).sin();
                lerp([0.0; 3], pick(&spec.colors, hash(h, 1)), s * s)
            } else {
                [0.0; 3]
            }
        }
        EffectKind::Fire => {
            let u = if sign > 0.0 { x } else { 1.0 - x };
            let a = FIRE_SCALE * spec.density * u;
            let s = FIRE_RATE * spec.speed * t;
            let b = FIRE_SCALE * spec.density * y;
            let n = 0.6 * noise(a - s, s + b, 0xF1)
                + 0.4 * noise(2.0 * a - 2.0 * s, 2.0 * (s + b), 0xF2);
            let heat = ((1.0 - 0.75 * u) * (0.25 + n)).clamp(0.0, 1.0);
            lerp([0.0; 3], sample(&spec.colors, heat), (1.6 * heat).min(1.0))
        }
        EffectKind::Rain => {
            let u = if sign > 0.0 { x } else { 1.0 - x };
            let drops = (4.0 * spec.density).round().clamp(1.0, RAIN_MAX) as u32;
            let lane = (y * RAIN_LANES).floor().clamp(0.0, RAIN_LANES - 1.0) as u32;
            let mut c = [0.0f64; 3];
            for j in (0..drops).map(|j| j + lane * RAIN_MAX as u32) {
                let cycles = spec.speed * t + unit(hash(j, 0xA1));
                let h = hash(hash(j, cell(cycles)), 0xA2);
                // A faster drop finishes early and leaves a gap before the next one.
                let head = frac(cycles) * (1.0 + unit(h)) * (1.0 + RAIN_TAIL);
                let level = tail(head - u, RAIN_TAIL);
                if level > 0.0 {
                    let d = lerp([0.0; 3], pick(&spec.colors, hash(h, 1)), level);
                    c = [c[0].max(d[0]), c[1].max(d[1]), c[2].max(d[2])];
                }
            }
            c
        }
        // Neither is rendered from a canvas position: the engine uses `painted_frame`, and the
        // hardware animates a device mode itself.
        EffectKind::DeviceMode | EffectKind::Painted => [0.0; 3],
    };
    finish(c, spec.brightness)
}

/// The frame of a `painted` look for a device with `leds` LEDs: the painted colours by LED
/// index, black where the device has more LEDs than were painted, extra paint cut off.
pub fn painted_frame(spec: &EffectSpec, leds: usize, out: &mut Vec<Rgb>) {
    out.clear();
    out.extend((0..leds).map(|i| {
        let c = spec.colors.get(i).copied().unwrap_or(Rgb::BLACK);
        finish(to_f3(c), spec.brightness)
    }));
}

/// A whole frame: one colour per canvas position.
pub fn render_frame(spec: &EffectSpec, t: f64, positions: &[Point], out: &mut Vec<Rgb>) {
    out.clear();
    out.extend(positions.iter().map(|&[x, y]| render_at(spec, t, x, y)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(json: &str) -> EffectSpec {
        let e: EffectSpec = serde_json::from_str(json).unwrap();
        e.validate().unwrap();
        e
    }
    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    #[test]
    fn static_ignores_time_and_position() {
        let e = spec(r##"{"type":"static","colors":["#102030"]}"##);
        for (t, x) in [(0.0, 0.0), (9.7, 0.5), (100.0, 1.0)] {
            assert_eq!(render_at(&e, t, x, 0.0), rgb(0x10, 0x20, 0x30));
        }
    }

    #[test]
    fn gradient_hits_its_endpoints_and_reverses() {
        let e = spec(r##"{"type":"gradient","colors":["#ff0000","#0000ff"]}"##);
        assert_eq!(render_at(&e, 0.0, 0.0, 0.0), rgb(255, 0, 0));
        assert_eq!(render_at(&e, 0.0, 1.0, 0.0), rgb(0, 0, 255));
        assert_eq!(render_at(&e, 0.0, 0.5, 0.0), rgb(128, 0, 128));
        let r =
            spec(r##"{"type":"gradient","colors":["#ff0000","#0000ff"],"direction":"reverse"}"##);
        assert_eq!(render_at(&r, 0.0, 0.0, 0.0), rgb(0, 0, 255));
    }

    #[test]
    fn three_colour_gradient_passes_through_the_middle_colour() {
        let e = spec(r##"{"type":"gradient","colors":["#ff0000","#00ff00","#0000ff"]}"##);
        assert_eq!(render_at(&e, 0.0, 0.5, 0.0), rgb(0, 255, 0));
    }

    #[test]
    fn rainbow_has_the_primaries_at_sixths_and_wraps_once_per_density() {
        let e = spec(r#"{"type":"rainbow","speed":0}"#);
        assert_eq!(render_at(&e, 0.0, 0.0, 0.0), rgb(255, 0, 0));
        assert_eq!(render_at(&e, 0.0, 1.0 / 3.0, 0.0), rgb(0, 255, 0));
        assert_eq!(render_at(&e, 0.0, 2.0 / 3.0, 0.0), rgb(0, 0, 255));
        let two = spec(r#"{"type":"rainbow","speed":0,"density":2}"#);
        assert_eq!(render_at(&two, 0.0, 0.5, 0.0), rgb(255, 0, 0));
    }

    #[test]
    fn rainbow_scrolls_in_the_requested_direction() {
        let fwd = spec(r#"{"type":"rainbow","speed":1}"#);
        // Forward motion: the colour seen at x moves to larger x, so at a later time the
        // colour at x equals the earlier colour at a smaller x.
        assert_eq!(
            render_at(&fwd, 0.25, 0.25, 0.0),
            render_at(&fwd, 0.0, 0.0, 0.0)
        );
        let rev = spec(r#"{"type":"rainbow","speed":1,"direction":"reverse"}"#);
        assert_eq!(
            render_at(&rev, 0.25, 0.0, 0.0),
            render_at(&rev, 0.0, 0.25, 0.0)
        );
    }

    #[test]
    fn spectrum_cycle_is_uniform_and_periodic() {
        let e = spec(r#"{"type":"spectrum_cycle","speed":0.5}"#);
        assert_eq!(render_at(&e, 0.7, 0.0, 0.0), render_at(&e, 0.7, 1.0, 0.0));
        assert_eq!(render_at(&e, 0.0, 0.3, 0.0), rgb(255, 0, 0));
        assert_eq!(render_at(&e, 2.0, 0.3, 0.0), rgb(255, 0, 0));
        assert_eq!(render_at(&e, 2.0 / 3.0, 0.3, 0.0), rgb(0, 255, 0));
    }

    #[test]
    fn breathing_starts_dark_peaks_mid_cycle_and_takes_the_next_colour() {
        let e = spec(r##"{"type":"breathing","colors":["#ff0000","#0000ff"],"speed":1}"##);
        assert_eq!(render_at(&e, 0.0, 0.0, 0.0), rgb(0, 0, 0));
        assert_eq!(render_at(&e, 0.5, 0.0, 0.0), rgb(255, 0, 0));
        assert_eq!(render_at(&e, 1.0, 0.0, 0.0), rgb(0, 0, 0));
        assert_eq!(render_at(&e, 1.5, 0.0, 0.0), rgb(0, 0, 255));
        assert_eq!(render_at(&e, 2.5, 0.0, 0.0), rgb(255, 0, 0));
    }

    #[test]
    fn wave_with_one_colour_swings_between_black_and_it() {
        let e = spec(r##"{"type":"wave","colors":["#00ff00"],"speed":0}"##);
        assert_eq!(render_at(&e, 0.0, 0.0, 0.0), rgb(0, 0, 0));
        assert_eq!(render_at(&e, 0.0, 0.5, 0.0), rgb(0, 255, 0));
    }

    #[test]
    fn wave_travels_forward() {
        let e = spec(r##"{"type":"wave","colors":["#000000","#ffffff"],"speed":1}"##);
        // The crest (white) is at x = 0.5 at t = 0 and has moved to x = 0.75 at t = 0.25.
        assert_eq!(render_at(&e, 0.0, 0.5, 0.0), rgb(255, 255, 255));
        assert_eq!(render_at(&e, 0.25, 0.75, 0.0), rgb(255, 255, 255));
    }

    #[test]
    fn brightness_scales_and_zero_is_black() {
        let e = spec(r##"{"type":"static","colors":["#c8c8c8"],"brightness":0.5}"##);
        assert_eq!(render_at(&e, 0.0, 0.0, 0.0), rgb(100, 100, 100));
        let z = spec(r##"{"type":"static","colors":["#c8c8c8"],"brightness":0}"##);
        assert_eq!(render_at(&z, 0.0, 0.0, 0.0), rgb(0, 0, 0));
    }

    #[test]
    fn extreme_inputs_do_not_panic() {
        let e = spec(r##"{"type":"wave","colors":["#ff0000","#00ff00"],"speed":20,"density":64}"##);
        for (t, x) in [(0.0, 0.0), (1e9, 1.0), (-5.0, 0.3)] {
            let _ = render_at(&e, t, x, 0.0);
        }
        let b = spec(r##"{"type":"breathing","colors":["#ff0000"],"speed":20}"##);
        let _ = render_at(&b, -3.0, 0.0, 0.0);
    }

    #[test]
    fn comet_has_a_full_head_a_dimmer_tail_behind_and_darkness_ahead() {
        let e = spec(r##"{"type":"comet","colors":["#ffffff"],"speed":1}"##);
        // At t = 0.25 the head has travelled to x = 0.25.
        assert_eq!(render_at(&e, 0.25, 0.25, 0.0), rgb(255, 255, 255));
        let behind = render_at(&e, 0.25, 0.15, 0.0);
        assert!(behind.r > 0 && behind.r < 255, "{behind:?}");
        assert_eq!(render_at(&e, 0.25, 0.3, 0.0), rgb(0, 0, 0));
        // Reverse: the tail trails on the other side.
        let r = spec(r##"{"type":"comet","colors":["#ffffff"],"speed":1,"direction":"reverse"}"##);
        assert_eq!(render_at(&r, 0.25, 0.75, 0.0), rgb(255, 255, 255));
        assert!(render_at(&r, 0.25, 0.85, 0.0).r > 0);
        assert_eq!(render_at(&r, 0.25, 0.7, 0.0), rgb(0, 0, 0));
    }

    #[test]
    fn comet_fades_from_the_first_colour_at_the_head_to_the_last() {
        let e = spec(r##"{"type":"comet","colors":["#ff0000","#0000ff"],"speed":0}"##);
        let near = render_at(&e, 0.0, 0.99, 0.0);
        let far = render_at(&e, 0.0, 0.7, 0.0);
        assert!(near.r > near.b && far.b > far.r, "{near:?} {far:?}");
    }

    #[test]
    fn marquee_alternates_palette_dashes_with_gaps_and_moves() {
        let e = spec(r##"{"type":"marquee","colors":["#ff0000","#00ff00"],"speed":0}"##);
        let w = 1.0 / 16.0;
        assert_eq!(render_at(&e, 0.0, 0.5 * w, 0.0), rgb(255, 0, 0));
        assert_eq!(render_at(&e, 0.0, 1.5 * w, 0.0), rgb(0, 0, 0));
        assert_eq!(render_at(&e, 0.0, 2.5 * w, 0.0), rgb(0, 255, 0));
        assert_eq!(render_at(&e, 0.0, 4.5 * w, 0.0), rgb(255, 0, 0));
        let m = spec(r##"{"type":"marquee","colors":["#ff0000","#00ff00"],"speed":1}"##);
        // One dash period (two dashes' width) per second, forwards.
        assert_eq!(render_at(&m, 1.0, 2.5 * w, 0.0), rgb(255, 0, 0));
    }

    #[test]
    fn bouncing_beam_sweeps_there_and_back() {
        let e = spec(r##"{"type":"bouncing_beam","colors":["#00ff00"],"speed":1}"##);
        assert_eq!(render_at(&e, 0.0, 0.0, 0.0), rgb(0, 255, 0));
        assert_eq!(render_at(&e, 0.0, 0.5, 0.0), rgb(0, 0, 0));
        assert_eq!(render_at(&e, 0.5, 1.0, 0.0), rgb(0, 255, 0));
        assert_eq!(render_at(&e, 1.0, 0.0, 0.0), rgb(0, 255, 0));
        let r = spec(
            r##"{"type":"bouncing_beam","colors":["#00ff00"],"speed":1,"direction":"reverse"}"##,
        );
        assert_eq!(render_at(&r, 0.0, 1.0, 0.0), rgb(0, 255, 0));
        // A wider beam (lower density) still lights x = 0.2 when it is at 0.
        let wide =
            spec(r##"{"type":"bouncing_beam","colors":["#00ff00"],"speed":1,"density":0.5}"##);
        assert!(render_at(&wide, 0.0, 0.2, 0.0).g > 0);
        assert_eq!(render_at(&e, 0.0, 0.2, 0.0), rgb(0, 0, 0));
    }

    fn lit_share(e: &EffectSpec, t: f64) -> f64 {
        let n = 2000;
        let lit = (0..n)
            .filter(|&i| render_at(e, t, i as f64 / n as f64, 0.0) != Rgb::BLACK)
            .count();
        lit as f64 / n as f64
    }

    #[test]
    fn starfield_lights_a_scattered_few_that_change_and_density_sets_how_many() {
        let e = spec(r##"{"type":"starfield","colors":["#ffffff","#ffaa00"],"speed":0.5}"##);
        let share = lit_share(&e, 3.0);
        assert!((0.1..0.4).contains(&share), "{share}");
        let more = spec(r##"{"type":"starfield","colors":["#ffffff"],"speed":0.5,"density":3}"##);
        assert!(lit_share(&more, 3.0) > 2.0 * share);
        let none = spec(r##"{"type":"starfield","colors":["#ffffff"],"speed":0.5,"density":0}"##);
        assert_eq!(lit_share(&none, 3.0), 0.0);
        let changed = (0..200)
            .filter(|&i| {
                let x = i as f64 / 200.0;
                render_at(&e, 3.0, x, 0.0) != render_at(&e, 4.0, x, 0.0)
            })
            .count();
        assert!(changed > 20, "{changed}");
    }

    fn mean_level(e: &EffectSpec, x: f64) -> f64 {
        (0..400)
            .map(|i| render_at(e, i as f64 * 0.05, x, 0.0).r as f64)
            .sum::<f64>()
            / 400.0
    }

    #[test]
    fn fire_burns_hotter_at_its_base_and_reverse_mirrors_it() {
        let e = spec(r##"{"type":"fire","colors":["#ff0000","#ffff00"],"speed":0.3}"##);
        assert!(mean_level(&e, 0.05) > 2.0 * mean_level(&e, 0.95));
        let r = spec(
            r##"{"type":"fire","colors":["#ff0000","#ffff00"],"speed":0.3,"direction":"reverse"}"##,
        );
        for (t, x) in [(0.0, 0.1), (1.3, 0.4), (7.7, 0.9)] {
            assert_eq!(render_at(&e, t, x, 0.0), render_at(&r, t, 1.0 - x, 0.0));
        }
        // It flickers.
        assert_ne!(render_at(&e, 0.0, 0.1, 0.0), render_at(&e, 0.5, 0.1, 0.0));
    }

    #[test]
    fn rain_drops_come_and_go_everywhere_and_reverse_mirrors_it() {
        let e = spec(r##"{"type":"rain","colors":["#0080ff"],"speed":0.5}"##);
        for x in [0.05, 0.5, 0.95] {
            let lit = (0..400)
                .filter(|&i| render_at(&e, i as f64 * 0.05, x, 0.0) != Rgb::BLACK)
                .count();
            assert!((5..300).contains(&lit), "x={x}: lit {lit}/400");
        }
        let r = spec(r##"{"type":"rain","colors":["#0080ff"],"speed":0.5,"direction":"reverse"}"##);
        for (t, x) in [(0.3, 0.1), (2.1, 0.6), (9.9, 0.8)] {
            assert_eq!(render_at(&e, t, x, 0.0), render_at(&r, t, 1.0 - x, 0.0));
        }
    }

    #[test]
    fn new_effects_survive_extreme_inputs() {
        for kind in [
            "comet",
            "marquee",
            "bouncing_beam",
            "starfield",
            "fire",
            "rain",
        ] {
            for density in [0.0, 64.0] {
                let e = spec(&format!(
                    r##"{{"type":"{kind}","colors":["#ff0000","#00ff00"],"speed":20,"density":{density}}}"##
                ));
                for (t, x) in [(0.0, 0.0), (1e9, 1.0), (-5.0, 0.3)] {
                    let _ = render_at(&e, t, x, 0.0);
                }
            }
        }
    }

    #[test]
    fn a_painted_frame_is_by_led_index_padded_with_black_or_cut_and_scaled_by_brightness() {
        let c = |r| Rgb { r, g: 0, b: 0 };
        let spec = EffectSpec {
            kind: EffectKind::Painted,
            colors: vec![c(10), c(20), c(30)],
            speed: 0.2,
            direction: Direction::Forward,
            brightness: 1.0,
            density: 1.0,
            mode: None,
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        };
        let mut out = vec![];
        painted_frame(&spec, 5, &mut out);
        assert_eq!(out, [c(10), c(20), c(30), Rgb::BLACK, Rgb::BLACK]);
        painted_frame(&spec, 2, &mut out);
        assert_eq!(out, [c(10), c(20)]);
        painted_frame(
            &EffectSpec {
                brightness: 0.5,
                ..spec
            },
            3,
            &mut out,
        );
        assert_eq!(out, [c(5), c(10), c(15)]);
    }
}
