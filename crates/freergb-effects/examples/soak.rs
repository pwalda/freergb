// SPDX-License-Identifier: GPL-2.0-or-later
//! Leak check and tray-cost numbers for the backend's hot path: the SDK client and the effects
//! engine driving mock devices, cycling through looks and idle spells the way a tray-resident
//! app does. The mock runs in a child process (it keeps per-connection state of its own), so
//! the numbers are the client side only.
//!
//! ```text
//! cargo run --release -p freergb-effects --example soak -- [--secs 120] [--fps 30]
//!     [--warmup 20] [--max-growth-kb 2048]
//! ```
//!
//! Exits 1 when resident memory grows by more than `--max-growth-kb` between the first and the
//! last quarter of the run (after warm-up). Memory and CPU are read from /proc, so the numbers
//! and the check are Linux-only; elsewhere it just runs. Tray numbers of the real app on
//! Windows come from `tools/tray-numbers.ps1`.

use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Rgb, Scene, Targets};
use freergb_effects::{Engine, EngineConfig};
use freergb_mock::{devices, MockServer};
use freergb_sdk::{Client, ConnectOptions, RgbBackend};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Args {
    secs: u64,
    fps: u32,
    warmup: u64,
    max_growth_kb: u64,
}

fn args() -> Args {
    let mut a = Args {
        secs: 120,
        fps: 30,
        warmup: 20,
        max_growth_kb: 2048,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut num = || -> u64 {
            it.next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{flag} needs a number"))
        };
        match flag.as_str() {
            "--secs" => a.secs = num(),
            "--fps" => a.fps = num() as u32,
            "--warmup" => a.warmup = num(),
            "--max-growth-kb" => a.max_growth_kb = num(),
            other => panic!("unknown argument {other}"),
        }
    }
    assert!(
        a.secs > a.warmup + 8,
        "--secs must leave time after --warmup"
    );
    a
}

/// The child: a mock rig on a free port, printed on stdout, until stdin closes.
fn serve() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(async {
        MockServer::start(vec![
            devices::motherboard(),
            devices::keyboard(),
            devices::ram_stick(0),
            devices::ram_stick(1),
            devices::resizable_strip(),
            devices::big_300(),
        ])
        .await
        .unwrap()
    });
    server.set_recording(false);
    println!("{}", server.addr().port());
    let _ = std::io::stdin().read_to_end(&mut vec![]);
}

fn look(kind: EffectKind) -> Vec<Scene> {
    let colors = match kind {
        EffectKind::Rainbow | EffectKind::SpectrumCycle => vec![],
        _ => vec![
            Rgb {
                r: 255,
                g: 40,
                b: 0,
            },
            Rgb {
                r: 0,
                g: 80,
                b: 255,
            },
        ],
    };
    vec![Scene {
        effect: EffectSpec {
            kind,
            colors,
            speed: 0.5,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: None,
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        },
        targets: Targets::All,
        canvas: Canvas::Shared,
        group: None,
    }]
}

/// Resident set in kB and CPU time in seconds of this process.
#[cfg(target_os = "linux")]
fn sample() -> Option<(u64, f64)> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let rss = status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the parenthesised name; utime and stime are the 12th and 13th of those.
    let rest: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let ticks: f64 = rest.get(11)?.parse::<f64>().ok()? + rest.get(12)?.parse::<f64>().ok()?;
    Some((rss, ticks / 100.0)) // USER_HZ is 100 on every Linux we target
}
#[cfg(not(target_os = "linux"))]
fn sample() -> Option<(u64, f64)> {
    None
}

fn mean(v: &[u64]) -> f64 {
    v.iter().sum::<u64>() as f64 / v.len().max(1) as f64
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--serve") {
        return serve();
    }
    let a = args();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start the mock server");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let port: u16 = line.trim().parse().expect("mock server port");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let ok = rt.block_on(run(&a, port));
    let _ = child.kill();
    let _ = child.wait();
    std::process::exit(if ok { 0 } else { 1 });
}

async fn run(a: &Args, port: u16) -> bool {
    let client = Client::connect(
        std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        ConnectOptions::default(),
    )
    .await
    .expect("connect to the mock");
    let leds: usize = client.devices().iter().map(|(_, d)| d.colors.len()).sum();
    let engine = Engine::start(
        Arc::new(client),
        EngineConfig {
            fps: a.fps,
            ..EngineConfig::default()
        },
    );

    // Five seconds each; None is an idle spell (no effect: the loop must sleep).
    let cycle = [
        Some(EffectKind::Rainbow),
        Some(EffectKind::Wave),
        Some(EffectKind::Breathing),
        None,
        Some(EffectKind::Gradient),
        Some(EffectKind::SpectrumCycle),
        Some(EffectKind::Static),
        None,
        Some(EffectKind::Rain),
        Some(EffectKind::Fire),
        Some(EffectKind::Starfield),
        Some(EffectKind::Comet),
        Some(EffectKind::Marquee),
        Some(EffectKind::BouncingBeam),
    ];
    let start = Instant::now();
    let mut rss = vec![];
    let (mut busy_cpu, mut busy_secs, mut idle_cpu, mut idle_secs) =
        (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut last = sample();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    for s in 0..a.secs {
        let phase = cycle[(s / 5) as usize % cycle.len()];
        if s % 5 == 0 {
            engine
                .set_scenes(phase.map(look).unwrap_or_default())
                .unwrap();
        }
        tick.tick().await;
        let now = sample();
        // Skip the second after a switch: it carries the mode change, not the steady state.
        if let (Some((_, c0)), Some((kb, c1))) = (last, now) {
            if s >= a.warmup && s % 5 != 0 {
                if phase.is_some() {
                    (busy_cpu, busy_secs) = (busy_cpu + c1 - c0, busy_secs + 1.0);
                } else {
                    (idle_cpu, idle_secs) = (idle_cpu + c1 - c0, idle_secs + 1.0);
                }
            }
            if s >= a.warmup {
                rss.push(kb);
            }
        }
        last = now;
    }
    let stats = engine.stats();
    engine.stop().await;
    let secs = start.elapsed().as_secs_f64();

    println!(
        "soak: {secs:.0} s, {leds} LEDs on 6 mock devices, {} fps target",
        a.fps
    );
    println!(
        "device frames: {} sent ({:.1}/s overall, idle spells included), {} dropped",
        stats.sent,
        stats.sent as f64 / secs,
        stats.dropped
    );
    if rss.len() < 8 {
        println!("memory and CPU: not measured on this platform");
        return true;
    }
    let q = rss.len() / 4;
    let (first, last_q) = (mean(&rss[..q]), mean(&rss[rss.len() - q..]));
    let growth = last_q - first;
    println!(
        "CPU: {:.2}% of one core with an effect, {:.2}% idle",
        100.0 * busy_cpu / busy_secs.max(1.0),
        100.0 * idle_cpu / idle_secs.max(1.0)
    );
    println!(
        "RSS after warm-up: {:.1} MB first quarter, {:.1} MB last quarter, {:.1} MB peak ({growth:+.0} kB)",
        first / 1024.0,
        last_q / 1024.0,
        *rss.iter().max().unwrap() as f64 / 1024.0
    );
    let ok = growth <= a.max_growth_kb as f64;
    println!(
        "{}: memory growth {growth:+.0} kB (limit {} kB)",
        if ok { "PASS" } else { "FAIL" },
        a.max_growth_kb
    );
    ok
}
