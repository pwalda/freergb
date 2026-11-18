// SPDX-License-Identifier: GPL-2.0-or-later
//! Golden vectors for the effect maths. `vectors.json` is the contract with the webview
//! preview: the JS renderer must reproduce every sample (within 1 per channel, since `sin`
//! and `cos` may differ in the last ulp between V8 and Rust). `layout_vectors.json` does the
//! same for where LEDs land on the canvas (desk map, slots, angle), and `react_vectors.json`
//! for looks that follow a reading.
//!
//! Intentional change to the maths: `FREERGB_UPDATE_VECTORS=1 cargo test -p freergb-effects
//! --test vectors`, then update the JS side to match.

use freergb_effects::layout::{arrange, canvas, Placement, Point};
use freergb_effects::react::{render_led, Readings};
use freergb_effects::render::render_at;
use freergb_effects::EffectSpec;
use serde_json::{json, Value};

const SPECS: &[&str] = &[
    r##"{"type":"static","colors":["#336699"]}"##,
    r##"{"type":"static","colors":["#ffffff"],"brightness":0.4}"##,
    r##"{"type":"gradient","colors":["#ff0000","#0000ff"]}"##,
    r##"{"type":"gradient","colors":["#ff0000","#00ff00","#0000ff","#ffffff"],"direction":"reverse"}"##,
    r#"{"type":"spectrum_cycle","speed":0.3}"#,
    r#"{"type":"spectrum_cycle","speed":0.3,"direction":"reverse","brightness":0.7}"#,
    r#"{"type":"rainbow","speed":0.25}"#,
    r#"{"type":"rainbow","speed":1.5,"density":2.5,"direction":"reverse"}"#,
    r##"{"type":"breathing","colors":["#ff8000"],"speed":0.5}"##,
    r##"{"type":"breathing","colors":["#ff0000","#00ff00","#0000ff"],"speed":1.25}"##,
    r##"{"type":"wave","colors":["#00ff00"],"speed":0.5}"##,
    r##"{"type":"wave","colors":["#000000","#00aaff","#ffffff"],"speed":0.75,"density":3,"brightness":0.9}"##,
    r##"{"type":"wave","colors":["#ff0000","#0000ff"],"speed":2,"density":1.5,"direction":"reverse"}"##,
    r##"{"type":"comet","colors":["#ffffff"],"speed":0.5}"##,
    r##"{"type":"comet","colors":["#ff8000","#2000ff"],"speed":1.5,"density":2.5,"direction":"reverse","brightness":0.8}"##,
    r##"{"type":"marquee","colors":["#ff0000","#00ff00","#0000ff"],"speed":0.4}"##,
    r##"{"type":"marquee","colors":["#ffcc00"],"speed":2,"density":0.75,"direction":"reverse"}"##,
    r##"{"type":"bouncing_beam","colors":["#ff0000"],"speed":0.5}"##,
    r##"{"type":"bouncing_beam","colors":["#ff0000","#0000ff"],"speed":1.25,"density":2,"direction":"reverse"}"##,
    r##"{"type":"starfield","colors":["#ffffff"],"speed":0.5}"##,
    r##"{"type":"starfield","colors":["#ffffff","#ffaa00","#00aaff"],"speed":1.75,"density":3}"##,
    r##"{"type":"fire","colors":["#ff0000","#ff8000","#ffff80"],"speed":0.3}"##,
    r##"{"type":"fire","colors":["#2000ff","#00ffff"],"speed":1,"density":2.5,"direction":"reverse"}"##,
    r##"{"type":"rain","colors":["#00aaff"],"speed":0.5}"##,
    r##"{"type":"rain","colors":["#ffffff","#00ff80"],"speed":2.5,"density":3,"direction":"reverse","brightness":0.6}"##,
];
const TIMES: &[f64] = &[0.0, 0.1, 0.5, 1.3, 2.345, 7.77, 31.6];
// Irregular positions as well as the round ones, so the random-looking effects are sampled
// where they are lit and not only on cell boundaries.
const XS: &[f64] = &[
    0.0, 0.03, 0.1, 0.17, 0.25, 0.31, 0.38, 0.444, 0.5, 0.57, 0.63, 0.7, 0.77, 0.82, 0.9, 0.97, 1.0,
];
/// Across the canvas: a strip (0), a lane boundary region and the far edge.
const YS: &[f64] = &[0.0, 0.37, 1.0];

fn generate() -> Value {
    let cases: Vec<Value> = SPECS
        .iter()
        .map(|s| {
            let spec: EffectSpec = serde_json::from_str(s).unwrap();
            spec.validate().unwrap();
            let samples: Vec<Value> = TIMES
                .iter()
                .flat_map(|&t| XS.iter().map(move |&x| (t, x)))
                .flat_map(|(t, x)| YS.iter().map(move |&y| (t, x, y)))
                .map(|(t, x, y)| {
                    let c = render_at(&spec, t, x, y);
                    json!({"t": t, "x": x, "y": y, "rgb": [c.r, c.g, c.b]})
                })
                .collect();
            json!({"spec": spec, "samples": samples})
        })
        .collect();
    json!({"cases": cases})
}

fn strip(n: usize) -> Vec<Point> {
    (0..n)
        .map(|i| {
            [
                if n > 1 {
                    i as f64 / (n - 1) as f64
                } else {
                    0.5
                },
                0.5,
            ]
        })
        .collect()
}

/// A 4 x 3 grid with one cell missing, like a keyboard matrix.
fn grid() -> Vec<Point> {
    (0..12)
        .filter(|&c| c != 5)
        .map(|c| [(c % 4) as f64 / 3.0, (c / 4) as f64 / 2.0])
        .collect()
}

fn at(x: f64, y: f64, w: f64, h: f64, rot: f64) -> Option<Placement> {
    Some(Placement { x, y, w, h, rot })
}

/// A device's local points and its place on the desk, if it has one.
type Piece = (Vec<Point>, Option<Placement>);

fn layout_cases() -> Value {
    let cases: Vec<(Vec<Piece>, f64)> = vec![
        (vec![(strip(5), None), (strip(3), None)], 0.0),
        (vec![(strip(5), None), (strip(3), None)], 90.0),
        (vec![(grid(), None), (strip(4), None)], 90.0),
        (vec![(strip(6), at(0.0, 0.0, 30.0, 2.0, 90.0))], 270.0),
        (
            vec![
                (strip(4), at(0.0, 0.0, 10.0, 1.0, 0.0)),
                (strip(4), at(12.0, 0.0, 10.0, 1.0, 180.0)),
            ],
            45.0,
        ),
        (
            vec![
                (grid(), at(20.0, 10.0, 44.0, 14.0, 0.0)),
                (strip(8), at(0.0, 12.0, 30.0, 2.0, 90.0)),
                (strip(5), None),
            ],
            37.0,
        ),
        (vec![(strip(1), None)], 0.0),
    ];
    let out: Vec<Value> = cases
        .into_iter()
        .map(|(pieces, angle)| {
            let desk = arrange(&pieces);
            let uv = canvas(&desk.concat(), angle);
            let pieces: Vec<Value> = pieces
                .iter()
                .map(|(local, at)| json!({"local": local, "at": at}))
                .collect();
            json!({"pieces": pieces, "angle": angle, "uv": uv})
        })
        .collect();
    json!({"cases": out})
}

fn check(file: &str, now: Value) {
    let path = format!("{}/tests/{file}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FREERGB_UPDATE_VECTORS").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&now).unwrap() + "\n").unwrap();
        return;
    }
    let committed: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("vectors file missing"))
            .unwrap();
    assert!(
        same(&now, &committed),
        "{file} changed; if intended, regenerate with FREERGB_UPDATE_VECTORS=1 and update the JS preview"
    );
}

/// Equal, numbers within 1e-12: serde_json's default float parsing may be off by an ulp.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() <= 1e-12
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

/// Looks that follow a reading, each with the readings it is sampled under.
const REACTS: &[(&str, &str)] = &[
    (
        r##"{"type":"wave","colors":["#ff0000","#0000ff"],"speed":0.5,"react":{"source":"sound","drives":"brightness"}}"##,
        r#"{"sound":0.37}"#,
    ),
    (
        r##"{"type":"rainbow","speed":0.3,"density":2,"react":{"source":"cpu_load","drives":"amount","low":10,"high":90}}"##,
        r#"{"cpu_load":64}"#,
    ),
    (
        r##"{"type":"breathing","colors":["#00ff00","#ff00ff","#ffffff"],"speed":0.4,"react":{"source":"cpu_temp","drives":"color"}}"##,
        r#"{"cpu_temp":71.5}"#,
    ),
    (
        r##"{"type":"comet","colors":["#ffffff"],"speed":0.7,"react":{"source":"gpu_temp","drives":"brightness"}}"##,
        r#"{}"#,
    ),
    (
        r##"{"type":"static","colors":["#ffffff"],"react":{"source":"spectrum","drives":"brightness"}}"##,
        r#"{"spectrum":[0.9,0.1,0.55,0.0,1.0]}"#,
    ),
    (
        r##"{"type":"marquee","colors":["#ffffff","#808080"],"speed":0.5,"react":{"source":"screen","drives":"color"}}"##,
        r##"{"screen":{"width":3,"height":2,"colors":["#ff0000","#00ff00","#0000ff","#ffff00","#00ffff","#ff00ff"]}}"##,
    ),
    (
        r##"{"type":"gradient","colors":["#ff8000","#0080ff"],"react":{"source":"beat","drives":"color","low":0.2}}"##,
        r#"{"beat":0.8}"#,
    ),
    (
        r##"{"type":"wave","colors":["#ff0000","#0000ff"],"speed":0.5,"react":{"source":"beat","drives":"brightness","sensitivity":0.8,"strength":0.5}}"##,
        r#"{"beat":0.3}"#,
    ),
    (
        r##"{"type":"static","colors":["#ffffff"],"react":{"source":"spectrum","drives":"brightness","sensitivity":0.25,"strength":0.7}}"##,
        r#"{"spectrum":[0.9,0.1,0.55,0.0,1.0]}"#,
    ),
    (
        r##"{"type":"gradient","colors":["#ff8000","#0080ff"],"react":{"source":"sound","drives":"color","strength":0.6}}"##,
        r#"{"sound":0.9}"#,
    ),
];
const DESKS: &[[f64; 2]] = &[[0.0, 0.0], [0.4, 0.2], [0.75, 0.9], [1.0, 1.0]];

fn react_cases() -> Value {
    let cases: Vec<Value> = REACTS
        .iter()
        .map(|(s, r)| {
            let spec: EffectSpec = serde_json::from_str(s).unwrap();
            spec.validate().unwrap();
            let readings: Readings = serde_json::from_str(r).unwrap();
            let mut samples = vec![];
            for &t in &TIMES[..4] {
                for &x in XS {
                    for &desk in DESKS {
                        let c = render_led(&spec, &readings, t, x, 0.0, desk);
                        samples.push(json!({"t": t, "x": x, "desk": desk, "rgb": [c.r, c.g, c.b]}));
                    }
                }
            }
            json!({"spec": spec, "readings": readings, "samples": samples})
        })
        .collect();
    json!({"cases": cases})
}

#[test]
fn looks_that_follow_a_reading_match_the_committed_vectors() {
    check("react_vectors.json", react_cases());
}

#[test]
fn the_layout_matches_the_committed_vectors() {
    check("layout_vectors.json", layout_cases());
}

#[test]
fn the_maths_matches_the_committed_vectors() {
    check("vectors.json", generate());
}
