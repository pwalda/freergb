// SPDX-License-Identifier: GPL-2.0-or-later
//! Walks the steps the sound readings take to capture what the PC plays and prints each one's
//! result, so a machine where they stay at "No reading yet" shows which step fails. Play
//! some music while it runs.
//!
//! ```text
//! cargo run -p freergb-sources --example sound_probe
//! ```

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() {
    // On a thread of its own, as in the app.
    std::thread::spawn(probe).join().unwrap();
}

fn name(d: &cpal::Device) -> String {
    d.description()
        .map(|x| x.name().to_string())
        .unwrap_or_else(|e| format!("(no name: {e})"))
}

fn id(d: &cpal::Device) -> String {
    d.id()
        .map(|i| i.id().to_string())
        .unwrap_or_else(|e| format!("(no id: {e})"))
}

fn probe() {
    #[cfg(target_os = "linux")]
    let host = match cpal::host_from_id(cpal::HostId::PulseAudio) {
        Ok(h) => h,
        Err(e) => return println!("PulseAudio host: {e}"),
    };
    #[cfg(not(target_os = "linux"))]
    let host = cpal::default_host();
    println!("host: {:?}", host.id());

    let Some(default) = host.default_output_device() else {
        return println!("default output: none");
    };
    let of = id(&default);
    println!("default output: {} [{of}]", name(&default));

    match host.output_devices() {
        Ok(all) => {
            for d in all {
                let i = id(&d);
                let mark = if i == of { "  <- default" } else { "" };
                println!("  output: {} [{i}]{mark}", name(&d));
            }
        }
        Err(e) => println!("output devices: {e}"),
    }

    #[cfg(target_os = "linux")]
    let endpoint = host
        .input_devices()
        .ok()
        .and_then(|mut all| all.find(|d| id(d) == format!("{of}.monitor")));
    #[cfg(not(target_os = "linux"))]
    let endpoint = host
        .output_devices()
        .ok()
        .and_then(|mut all| all.find(|d| id(d) == of));
    match endpoint {
        Some(d) => try_capture("the default's endpoint", &d),
        None => println!("the default's endpoint: not found among the devices"),
    }
    #[cfg(windows)]
    try_capture("the default output as cpal hands it out", &default);
}

fn try_capture(what: &str, device: &cpal::Device) {
    println!("\n{what}: {}", name(device));
    #[cfg(target_os = "linux")]
    let config = device.default_input_config();
    #[cfg(not(target_os = "linux"))]
    let config = device.default_output_config();
    let config = match config {
        Ok(c) => {
            println!(
                "  format: {:?}, {} Hz, {} channels",
                c.sample_format(),
                c.sample_rate(),
                c.channels()
            );
            c
        }
        Err(e) => return println!("  format: FAILED {e}"),
    };
    let samples = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(Mutex::new(0f32));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let (n, p, errs) = (samples.clone(), peak.clone(), errors.clone());
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
            config.config(),
            move |data: &[f32], _| {
                n.fetch_add(data.len(), Ordering::Relaxed);
                let m = data.iter().fold(0f32, |m, s| m.max(s.abs()));
                let mut p = p.lock().unwrap();
                *p = p.max(m);
            },
            move |e| errs.lock().unwrap().push(e.to_string()),
            Some(Duration::from_secs(2)),
        ),
        other => device.build_input_stream_raw(
            config.config(),
            other,
            move |data: &cpal::Data, _| {
                n.fetch_add(data.len(), Ordering::Relaxed);
            },
            move |e| errs.lock().unwrap().push(e.to_string()),
            Some(Duration::from_secs(2)),
        ),
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => return println!("  open: FAILED {e}"),
    };
    println!("  open: ok");
    if let Err(e) = stream.play() {
        return println!("  play: FAILED {e}");
    }
    println!("  play: ok; listening 5 s...");
    std::thread::sleep(Duration::from_secs(5));
    drop(stream);
    println!(
        "  samples: {}, loudest: {:.3}",
        samples.load(Ordering::Relaxed),
        *peak.lock().unwrap()
    );
    for e in errors.lock().unwrap().iter() {
        println!("  stream error: {e}");
    }
}
