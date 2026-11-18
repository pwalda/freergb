// SPDX-License-Identifier: GPL-2.0-or-later
//! Where each LED sits on an effect's canvas.
//!
//! Three steps, mirrored by the webview preview (`ui/src/lib/preview.ts`) and pinned for both
//! by `tests/layout_vectors.json`:
//!
//! 0. [`pieces`]: a device whose zones have OpenRGB segments is split, each segment its own
//!    piece on the map with its own [`Placed`] entry; the LEDs no segment holds stay with the
//!    device's entry. A device without segments is one piece.
//! 1. [`local_points`]: each LED's spot in the device's own unit square. Linear devices run
//!    along x in controller order, all zones concatenated, at mid-height. A zone with a matrix
//!    map (a keyboard) uses its columns and rows instead, so a sweep crosses the keys in space
//!    rather than in wiring order.
//! 2. [`arrange`]: devices go onto the desk map, each by its [`Placement`]. A device without
//!    one takes the next square slot to the right of the placed devices (with nothing placed,
//!    slot `k` of `n` is today's `k/n..(k+1)/n` slice), in target order.
//! 3. [`canvas`]: the desk points become `(u, v)` in 0..1. `u` runs along the effect's angle
//!    (where it moves), `v` across it. LEDs that all lie on one line (a strip, or strips in a
//!    row) use their position along that line, so a strip still animates whatever the angle;
//!    the angle only decides which end it starts from.

use freergb_sdk::{ControllerData, DeviceKey};
use serde::{Deserialize, Serialize};

pub type Point = [f64; 2];

/// Where a device sits on the desk map: its centre, width and height (of its unit square) and
/// a clockwise rotation in degrees. Units are the map editor's; only proportions matter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub rot: f64,
}

impl Placement {
    pub fn is_valid(&self) -> bool {
        [self.x, self.y, self.w, self.h, self.rot]
            .iter()
            .all(|v| v.is_finite())
            && self.w > 0.0
            && self.h > 0.0
    }
}

/// One device's entry in the desk map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    pub device: DeviceKey,
    /// Where it sits; `None` until the user places it (it then takes a slot).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Placement>,
    /// Each LED's spot in the unit square, as the UI draws the device. Used instead of
    /// [`local_points`] while it has one entry per LED (a zone resize falls back).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<Point>>,
    /// The UI's drawing of the device (its stage shape and a strip's length in dots); kept
    /// here so it travels with `points`, not read by the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<u32>,
    /// Set on a segment's own entry; `None` is the device (or what is left of it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<SegmentKey>,
}

/// A segment on the map, by its zone and its first LED within the zone. Splitting a strip
/// again keys the new runs afresh; an entry whose segment is gone is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentKey {
    pub zone: u32,
    pub start: u32,
}

/// One piece of a device on the map: which of its LEDs (indexes into `colors`, ascending).
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub segment: Option<SegmentKey>,
    pub leds: Vec<usize>,
}

/// The device's pieces: the LEDs no segment holds first (left out when there are none), then
/// each segment in zone order. Only zones without a key grid split, and a segment that is
/// empty, runs past its zone or overlaps the one before it is skipped (the server checks none
/// of this). Mirrored by `devicePieces` in the webview.
pub fn pieces(d: &ControllerData) -> Vec<Piece> {
    let n = d.colors.len();
    let mut taken = vec![false; n];
    let mut segs = vec![];
    let mut offset = 0usize;
    for (zi, z) in d.zones.iter().enumerate() {
        let count = z.leds_count as usize;
        if !z.matrix_map.as_ref().is_some_and(|m| m.width > 0) {
            let mut end = 0usize;
            for s in &z.segments {
                let (start, len) = (s.start_idx as usize, s.leds_count as usize);
                if len == 0 || start < end || start + len > count {
                    continue;
                }
                end = start + len;
                let leds: Vec<usize> = (offset + start..offset + end).filter(|&i| i < n).collect();
                if leds.is_empty() {
                    continue;
                }
                for &i in &leds {
                    taken[i] = true;
                }
                segs.push(Piece {
                    segment: Some(SegmentKey {
                        zone: zi as u32,
                        start: s.start_idx,
                    }),
                    leds,
                });
            }
        }
        offset += count;
    }
    let rest: Vec<usize> = (0..n).filter(|&i| !taken[i]).collect();
    let mut out = vec![];
    if !rest.is_empty() || segs.is_empty() {
        out.push(Piece {
            segment: None,
            leds: rest,
        });
    }
    out.extend(segs);
    out
}

/// Each piece of `d` with its LED spots and placement, from the device's entries in the map.
/// A piece uses its drawn spots while they fit its LED count; otherwise the device's own
/// spots ([`local_points`]) for the rest and a straight run for a segment.
pub fn placed_pieces(
    d: &ControllerData,
    entries: &[&Placed],
) -> Vec<(Vec<usize>, Vec<Point>, Option<Placement>)> {
    let all = local_points(d);
    pieces(d)
        .into_iter()
        .map(|p| {
            let e = entries.iter().find(|e| e.segment == p.segment);
            let local = match e.and_then(|e| e.points.as_ref()) {
                Some(pts) if pts.len() == p.leds.len() => pts.clone(),
                _ if p.segment.is_some() => {
                    let k = p.leds.len();
                    (0..k).map(|i| [linear(i, k), 0.5]).collect()
                }
                _ => p.leds.iter().map(|&i| all[i]).collect(),
            };
            (p.leds, local, e.and_then(|e| e.at))
        })
        .collect()
}

fn linear(i: usize, n: usize) -> f64 {
    if n <= 1 {
        0.5
    } else {
        i as f64 / (n - 1) as f64
    }
}

/// One point per LED (`colors.len()`) in the device's unit square, y down.
pub fn local_points(d: &ControllerData) -> Vec<Point> {
    let n = d.colors.len();
    let mut out: Vec<Point> = (0..n).map(|i| [linear(i, n), 0.5]).collect();

    // Overwrite the LEDs of matrix zones with their cell.
    let mut offset = 0usize;
    for z in &d.zones {
        let count = z.leds_count as usize;
        if let Some(m) = &z.matrix_map {
            if m.width > 0 {
                let cols = m.width as usize;
                let rows = m.data.len().div_ceil(cols);
                for (cell, &led) in m.data.iter().enumerate() {
                    // 0xFFFF_FFFF marks a cell with no LED.
                    if led == u32::MAX {
                        continue;
                    }
                    if let Some(slot) = out.get_mut(offset + led as usize) {
                        *slot = [linear(cell % cols, cols), linear(cell / cols, rows)];
                    }
                }
            }
        }
        offset += count;
    }
    out
}

/// `local` (unit square) placed on the desk.
pub fn on_desk(local: &[Point], at: &Placement) -> Vec<Point> {
    let (s, c) = at.rot.to_radians().sin_cos();
    local
        .iter()
        .map(|&[lx, ly]| {
            let dx = (lx - 0.5) * at.w;
            let dy = (ly - 0.5) * at.h;
            [at.x + dx * c - dy * s, at.y + dx * s + dy * c]
        })
        .collect()
}

/// Desk points of several devices, one list per device in the order given. `None` takes the
/// next free slot, see the module docs.
pub fn arrange(pieces: &[(Vec<Point>, Option<Placement>)]) -> Vec<Vec<Point>> {
    let placed: Vec<Option<Vec<Point>>> = pieces
        .iter()
        .map(|(local, at)| at.map(|a| on_desk(local, &a)))
        .collect();
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in placed.iter().flatten().flatten() {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let (x0, y0, size) = if min[0].is_finite() {
        let h = max[1] - min[1];
        (max[0], min[1], if h > 0.0 { h } else { 1.0 })
    } else {
        (0.0, 0.0, 1.0)
    };
    let mut next = 0.0;
    pieces
        .iter()
        .zip(placed)
        .map(|((local, _), desk)| {
            desk.unwrap_or_else(|| {
                let at = Placement {
                    x: x0 + size * (next + 0.5),
                    y: y0 + size * 0.5,
                    w: size,
                    h: size,
                    rot: 0.0,
                };
                next += 1.0;
                on_desk(local, &at)
            })
        })
        .collect()
}

/// `(u, v)` in 0..1 for each desk point, for an effect moving at `angle` degrees (0 = towards
/// +x, 90 = towards +y, that is down the map).
pub fn canvas(points: &[Point], angle: f64) -> Vec<Point> {
    let n = points.len();
    if n == 0 {
        return vec![];
    }
    let (s, c) = angle.to_radians().sin_cos();
    let along = [c, s];
    let across = [-s, c];
    let dot = |p: Point, d: Point| p[0] * d[0] + p[1] * d[1];

    let p0 = points[0];
    let far = points
        .iter()
        .copied()
        .max_by(|a, b| dist(*a, p0).total_cmp(&dist(*b, p0)))
        .unwrap_or(p0);
    let len = dist(far, p0);
    if len <= 0.0 {
        return vec![[0.5, 0.0]; n];
    }
    let line = [(far[0] - p0[0]) / len, (far[1] - p0[1]) / len];
    let normal = [-line[1], line[0]];
    let straight = points
        .iter()
        .all(|&p| dot([p[0] - p0[0], p[1] - p0[1]], normal).abs() <= 1e-9 * len);
    if straight {
        let flip = dot(line, along) < -1e-9;
        let u = normalised(points.iter().map(|&p| dot(p, line)));
        return u
            .into_iter()
            .map(|u| [if flip { 1.0 - u } else { u }, 0.0])
            .collect();
    }
    let u = normalised(points.iter().map(|&p| dot(p, along)));
    let v = normalised(points.iter().map(|&p| dot(p, across)));
    u.into_iter().zip(v).map(|(u, v)| [u, v]).collect()
}

fn dist(a: Point, b: Point) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Rescale to 0..1; a constant becomes 0.
fn normalised(vals: impl Iterator<Item = f64> + Clone) -> Vec<f64> {
    let (lo, hi) = vals
        .clone()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    let span = hi - lo;
    vals.map(|v| {
        if span > 0.0 {
            ((v - lo) / span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use freergb_mock::devices;

    fn close(a: &[Point], b: &[Point]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(p, q)| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9)
    }

    fn strip(n: usize) -> Vec<Point> {
        (0..n).map(|i| [linear(i, n), 0.5]).collect()
    }

    #[test]
    fn strip_runs_from_zero_to_one_at_mid_height() {
        let d = devices::big_300();
        let p = local_points(&d);
        assert_eq!(p.len(), d.colors.len());
        assert_eq!(p[0], [0.0, 0.5]);
        assert_eq!(*p.last().unwrap(), [1.0, 0.5]);
        assert!(p.windows(2).all(|w| w[0][0] <= w[1][0]));
    }

    #[test]
    fn matrix_zone_uses_columns_and_rows_not_wiring_order() {
        let d = devices::keyboard();
        let p = local_points(&d);
        let z = d.zones.iter().find(|z| z.matrix_map.is_some()).unwrap();
        let m = z.matrix_map.as_ref().unwrap();
        let w = m.width as usize;
        let at = |cell: usize| p[m.data[cell] as usize];
        assert!(m.data[0] != u32::MAX && m.data[w] != u32::MAX);
        // Same column, next row: same x, lower down.
        assert_eq!(at(0)[0], at(w)[0]);
        assert!(at(0)[1] < at(w)[1]);
    }

    #[test]
    fn unplaced_devices_tile_slots_like_the_old_one_dimensional_canvas() {
        let pieces = vec![(strip(5), None), (strip(5), None)];
        let desk = arrange(&pieces);
        let all: Vec<Point> = desk.concat();
        let uv = canvas(&all, 0.0);
        let u: Vec<f64> = uv.iter().map(|p| p[0]).collect();
        assert_eq!(u[..5], [0.0, 0.125, 0.25, 0.375, 0.5]);
        assert_eq!(u[5..], [0.5, 0.625, 0.75, 0.875, 1.0]);
        // A strip ignores a perpendicular angle (rain's default) and keeps its own order.
        assert!(close(&canvas(&all, 90.0), &uv));
    }

    #[test]
    fn the_angle_picks_the_end_a_line_starts_from() {
        let line = strip(3);
        let u = |a: f64| canvas(&line, a).iter().map(|p| p[0]).collect::<Vec<_>>();
        assert_eq!(u(0.0), vec![0.0, 0.5, 1.0]);
        assert_eq!(u(180.0), vec![1.0, 0.5, 0.0]);
        // A vertical strip, LED 0 at the top: falling (90) starts at the top, rising at the bottom.
        let up = on_desk(
            &line,
            &Placement {
                x: 0.0,
                y: 0.0,
                w: 2.0,
                h: 1.0,
                rot: 90.0,
            },
        );
        let fall: Vec<f64> = canvas(&up, 90.0).iter().map(|p| p[0]).collect();
        assert!((fall[0] - 0.0).abs() < 1e-9 && (fall[2] - 1.0).abs() < 1e-9);
        let rise: Vec<f64> = canvas(&up, 270.0).iter().map(|p| p[0]).collect();
        assert!((rise[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_grid_gets_u_along_the_angle_and_v_across_it() {
        let grid = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        assert!(close(
            &canvas(&grid, 0.0),
            &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
        ));
        // Falling: u grows downwards, v runs right to left (across, clockwise from along).
        assert!(close(
            &canvas(&grid, 90.0),
            &[[0.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]]
        ));
    }

    #[test]
    fn placement_moves_scales_and_rotates_the_unit_square() {
        let at = Placement {
            x: 10.0,
            y: 5.0,
            w: 4.0,
            h: 2.0,
            rot: 90.0,
        };
        let p = on_desk(&[[0.0, 0.0], [1.0, 0.5]], &at);
        assert!(close(&p, &[[11.0, 3.0], [10.0, 7.0]]));
    }

    #[test]
    fn unplaced_devices_go_right_of_the_placed_ones() {
        let placed = Placement {
            x: 0.0,
            y: 0.0,
            w: 4.0,
            h: 2.0,
            rot: 0.0,
        };
        let desk = arrange(&[
            (strip(2), None),
            (vec![[0.0, 0.0], [1.0, 1.0]], Some(placed)),
        ]);
        // Placed box: x -2..2, y -1..1; the slot is 2 high and starts at x = 2.
        assert!(close(&desk[1], &[[-2.0, -1.0], [2.0, 1.0]]));
        assert!(close(&desk[0], &[[2.0, 0.0], [4.0, 0.0]]));
    }

    fn entry(points: Option<Vec<Point>>, segment: Option<SegmentKey>) -> Placed {
        Placed {
            device: freergb_sdk::DeviceKey {
                name: "D".into(),
                location: String::new(),
                serial: String::new(),
                nth: 0,
            },
            at: None,
            points,
            shape: None,
            length: None,
            segment,
        }
    }

    #[test]
    fn drawn_spots_are_used_only_while_they_fit_the_led_count() {
        let mut d = devices::ram_stick(0);
        let n = d.colors.len();
        let drawn: Vec<Point> = (0..n).map(|i| [0.5, linear(i, n)]).collect();
        let e = entry(Some(drawn.clone()), None);
        let local = |d: &ControllerData, es: &[&Placed]| placed_pieces(d, es)[0].1.clone();
        assert_eq!(local(&d, &[&e]), drawn);
        d.colors.pop();
        assert_eq!(local(&d, &[&e]), local_points(&d));
        assert_eq!(local(&d, &[]), local_points(&d));
    }

    fn segment(name: &str, start: u32, n: u32) -> freergb_sdk::Segment {
        freergb_sdk::Segment {
            name: name.into(),
            start_idx: start,
            leds_count: n,
            ..Default::default()
        }
    }

    #[test]
    fn segments_become_pieces_and_the_rest_stays_with_the_device() {
        let mut d = devices::big_300();
        assert_eq!(pieces(&d).len(), 1);
        assert_eq!(pieces(&d)[0].leds.len(), 300);
        // Overlapping and overlong runs are skipped, as an empty one is.
        d.zones[0].segments = vec![
            segment("Desk", 10, 20),
            segment("Overlap", 25, 10),
            segment("Empty", 40, 0),
            segment("Shelf", 280, 20),
            segment("Past", 299, 5),
        ];
        let p = pieces(&d);
        let keys: Vec<_> = p.iter().map(|p| p.segment.map(|s| s.start)).collect();
        assert_eq!(keys, [None, Some(10), Some(280)]);
        assert_eq!(p[0].leds.len(), 260);
        assert_eq!(p[0].leds[..11], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 30]);
        assert_eq!(p[1].leds, (10..30).collect::<Vec<_>>());
        // A segment's piece is a straight run unless drawn; the rest keeps the device's spots.
        let placed = placed_pieces(&d, &[]);
        assert_eq!(placed[1].1[0], [0.0, 0.5]);
        assert_eq!(placed[1].1[19], [1.0, 0.5]);
        assert_eq!(placed[0].1[10], local_points(&d)[30]);
        let drawn: Vec<Point> = (0..20).map(|i| [0.5, linear(i, 20)]).collect();
        let e = entry(
            Some(drawn.clone()),
            Some(SegmentKey {
                zone: 0,
                start: 280,
            }),
        );
        assert_eq!(placed_pieces(&d, &[&e])[2].1, drawn);
        // Fully split: no piece is left for the device itself.
        d.zones[0].segments = vec![segment("A", 0, 150), segment("B", 150, 150)];
        assert!(pieces(&d).iter().all(|p| p.segment.is_some()));
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        assert!(canvas(&[], 0.0).is_empty());
        assert_eq!(canvas(&[[3.0, 3.0]], 45.0), vec![[0.5, 0.0]]);
        assert_eq!(canvas(&[[1.0, 1.0], [1.0, 1.0]], 0.0), vec![[0.5, 0.0]; 2]);
    }
}
