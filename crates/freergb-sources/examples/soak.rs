// SPDX-License-Identifier: GPL-2.0-or-later
//! Soak with readings: the engine plays looks that follow each source this machine offers,
//! with idle spells between, the way the app wires them (the hub runs only what a look
//! follows; every new reading goes to the engine). Prints the CPU each source costs on top of
//! the look, whether it delivered readings, and checks memory does not grow.
//!
//! ```text
//! cargo run --release -p freergb-sources --example soak -- [--secs 600] [--phase 10]
//!     [--max-growth-kb 4096]
//! ```
//!
//! Memory and CPU come from /proc (Linux only); elsewhere it just runs. The mock rig runs in
//! a child process, so the numbers are freeRGB's side only.

use freergb_effects::react::{Drive, React, Readings, SourceId};
use freergb_effects::spec::{Canvas, EffectKind, EffectSpec, Rgb, Scene, Targets};
use freergb_effects::{Engine, EngineConfig};
use freergb_mock::{devices, MockServer};
use freergb_sdk::{Client, ConnectOptions};
use freergb_sources::{demand, machine, Hub};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Args {
    secs: u64,
    phase: u64,
    max_growth_kb: u64,
}

fn args() -> Args {
    let mut a = Args {
        secs: 600,
        phase: 10,
        max_growth_kb: 4096,
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
            "--phase" => a.phase = num(),
            "--max-growth-kb" => a.max_growth_kb = num(),
            other => panic!("unknown argument {other}"),
        }
    }
    assert!(a.phase >= 4, "--phase must be at least 4 s");
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

/// A wave across the whole rig that follows `source` (or none).
fn look(source: Option<SourceId>) -> Vec<Scene> {
    let drives = match source {
        Some(SourceId::Screen) => Drive::Color,
        _ => Drive::Brightness,
    };
    let c = |r, g, b| Rgb { r, g, b };
    vec![Scene {
        effect: EffectSpec {
            kind: EffectKind::Wave,
            colors: vec![c(255, 40, 0), c(0, 80, 255)],
            speed: 0.5,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: None,
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: source.map(|source| React {
                source,
                drives,
                low: None,
                high: None,
                sensitivity: None,
                strength: None,
            }),
        },
        targets: Targets::All,
        canvas: Canvas::Shared,
        group: None,
    }]
}

fn has(r: &Readings, s: SourceId) -> bool {
    match s {
        SourceId::Spectrum => r.spectrum.is_some(),
        SourceId::Screen => r.screen.is_some(),
        _ => r.value(s).is_some(),
    }
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
    let rest: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let ticks: f64 = rest.get(11)?.parse::<f64>().ok()? + rest.get(12)?.parse::<f64>().ok()?;
    Some((rss, ticks / 100.0))
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

#[derive(Default)]
struct Phase {
    cpu: f64,
    secs: f64,
    delivered: bool,
}

async fn run(a: &Args, port: u16) -> bool {
    let client = Client::connect(
        std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        ConnectOptions::default(),
    )
    .await
    .expect("connect to the mock");
    let engine = Arc::new(Engine::start(Arc::new(client), EngineConfig::default()));
    let token = std::env::temp_dir().join("freergb-soak-screen-token");
    let hub = Hub::new(
        machine::local(Some(token)),
        tokio::runtime::Handle::current(),
    );
    let offered: Vec<SourceId> = hub.available().iter().map(|o| o.id).collect();

    // As the app does: every new reading goes to the engine.
    let mut readings = hub.readings();
    let to_engine = engine.clone();
    let forward = tokio::spawn(async move {
        while readings.changed().await.is_ok() {
            let r = readings.borrow_and_update().clone();
            to_engine.set_readings(r);
        }
    });

    // Idle, a look on its own, then each source, then idle again.
    let mut cycle: Vec<Option<Option<SourceId>>> = vec![None, Some(None)];
    cycle.extend(offered.iter().map(|s| Some(Some(*s))));
    let label = |p: Option<Option<SourceId>>| match p {
        None => "idle".to_string(),
        Some(None) => "look alone".to_string(),
        Some(Some(s)) => format!("{s:?}"),
    };

    let start = Instant::now();
    // The first round opens each source for the first time (libraries, audio and screen
    // sessions); memory is judged from the second round on.
    let round = a.phase * cycle.len() as u64;
    assert!(
        a.secs >= 3 * round,
        "--secs must cover at least three rounds ({round} s each)"
    );
    let warmup = round;
    let mut ends = vec![];
    let mut rss = vec![];
    let mut phases: BTreeMap<String, Phase> = BTreeMap::new();
    let mut last = sample();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    for s in 0..a.secs {
        let p = cycle[(s / a.phase) as usize % cycle.len()];
        let into = s % a.phase;
        if into == 0 {
            let scenes = p.map(look).unwrap_or_default();
            hub.want(&demand(scenes.iter().map(|s| &s.effect)));
            engine.set_scenes(scenes).unwrap();
        }
        tick.tick().await;
        let now = sample();
        let entry = phases.entry(label(p)).or_default();
        if let Some(Some(src)) = p {
            entry.delivered |= has(&hub.readings().borrow(), src);
        }
        // Skip the first two seconds of a phase: a source opening is not its steady cost.
        if let (Some((_, c0)), Some((kb, c1))) = (last, now) {
            if into >= 2 {
                entry.cpu += c1 - c0;
                entry.secs += 1.0;
            }
            if s >= warmup {
                rss.push(kb);
            }
            if (s + 1) % round == 0 {
                ends.push(kb);
            }
        }
        last = now;
    }
    hub.want(&Default::default());
    forward.abort();
    let stats = engine.stats();
    let secs = start.elapsed().as_secs_f64();

    println!(
        "soak with readings: {secs:.0} s, {}-s phases, offered: {offered:?}",
        a.phase
    );
    println!(
        "device frames: {} sent, {} dropped",
        stats.sent, stats.dropped
    );
    if rss.len() < 8 {
        println!("memory and CPU: not measured on this platform");
        return true;
    }
    for name in cycle.iter().map(|p| label(*p)) {
        let Some(ph) = phases.get(&name) else {
            continue;
        };
        let note = match name.as_str() {
            "idle" | "look alone" => "",
            _ if ph.delivered => "  (readings arrived)",
            _ => "  (NO readings on this machine)",
        };
        println!(
            "  {name:<12} {:>6.2}% of one core{note}",
            100.0 * ph.cpu / ph.secs.max(1.0)
        );
    }
    let ends: Vec<String> = ends
        .iter()
        .map(|kb| format!("{:.1}", *kb as f64 / 1024.0))
        .collect();
    println!("RSS at the end of each round (MB): {}", ends.join(", "));
    let q = rss.len() / 4;
    let (first, last_q) = (mean(&rss[..q]), mean(&rss[rss.len() - q..]));
    let growth = last_q - first;
    println!(
        "RSS: {:.1} MB first quarter, {:.1} MB last quarter, {:.1} MB peak",
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
