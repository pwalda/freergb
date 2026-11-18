// SPDX-License-Identifier: GPL-2.0-or-later
//! The temperatures and loads this machine can give, found once at startup without opening
//! anything slow: a source opens its sensor only when a look follows it.
//!
//! * CPU load: everywhere, through `sysinfo`.
//! * NVIDIA card (temperature, load): through NVML, the driver's own library, loaded at
//!   runtime, so freeRGB runs the same on machines without one.
//! * Linux: the CPU temperature and an AMD card's from `/sys/class/hwmon`.
//! * Windows: the CPU temperature from LibreHardwareMonitor, offered even while it is not
//!   running (the window says what the reading needs).
//! * The sound and the screen (see [`crate::audio`], [`crate::screen`]).

use crate::{Blocking, Source};
use freergb_effects::react::SourceId;
use nvml_wrapper::Nvml;
use std::path::PathBuf;
use std::time::Duration;

/// Loads change fast and are cheap to read; temperatures change slowly.
const LOAD_EVERY: Duration = Duration::from_millis(500);
const TEMP_EVERY: Duration = Duration::from_secs(1);

/// Every source this machine offers. When two give the same reading, the first counts.
/// `screen_token` is the file that keeps the desktop's answer to "which screen?" (Wayland).
pub fn local(screen_token: Option<PathBuf>) -> Vec<Box<dyn Source>> {
    let mut found: Vec<Box<dyn Source>> = vec![cpu_load()];
    #[cfg(windows)]
    found.push(crate::lhm::source());
    #[cfg(target_os = "linux")]
    if let Some(p) = crate::hwmon::cpu_temp(std::path::Path::new(crate::hwmon::ROOT)) {
        found.push(polled(SourceId::CpuTemp, TEMP_EVERY, move || {
            crate::hwmon::read_temp(&p)
        }));
    }
    if nvidia_driver() {
        found.push(nvidia(SourceId::GpuTemp, TEMP_EVERY, gpu_temp));
        found.push(nvidia(SourceId::GpuLoad, LOAD_EVERY, gpu_load));
    }
    #[cfg(target_os = "linux")]
    if let Some(gpu) = crate::hwmon::amd_gpu(std::path::Path::new(crate::hwmon::ROOT)) {
        let temp = gpu.temp;
        found.push(polled(SourceId::GpuTemp, TEMP_EVERY, move || {
            crate::hwmon::read_temp(&temp)
        }));
        if let Some(load) = gpu.load {
            found.push(polled(SourceId::GpuLoad, LOAD_EVERY, move || {
                crate::hwmon::read_percent(&load)
            }));
        }
    }
    found.extend(crate::audio::sources());
    found.extend(crate::screen::source(screen_token));
    found
}

/// The whole CPU's load in percent. The first read after opening needs a second sample, which
/// the next tick gives.
fn cpu_load() -> Box<dyn Source> {
    Box::new(Blocking {
        id: SourceId::CpuLoad,
        period: LOAD_EVERY,
        needs: None,
        open: || {
            let mut sys = sysinfo::System::new();
            sys.refresh_cpu_usage();
            Some(sys)
        },
        read: |sys: &mut sysinfo::System| {
            sys.refresh_cpu_usage();
            Some(f64::from(sys.global_cpu_usage()))
        },
    })
}

/// Whether NVIDIA's driver is installed, without loading its library (that can take a
/// moment, and startup should not wait on it).
fn nvidia_driver() -> bool {
    #[cfg(target_os = "linux")]
    return std::path::Path::new("/proc/driver/nvidia/version").exists();
    #[cfg(windows)]
    return std::env::var_os("SystemRoot")
        .map(|r| std::path::Path::new(&r).join("System32").join("nvml.dll"))
        .is_some_and(|p| p.exists());
    #[allow(unreachable_code)]
    false
}

/// A reading of the first NVIDIA card; each running reading opens the library itself (NVML
/// counts its users, so two readings share one driver session).
fn nvidia(id: SourceId, period: Duration, read: fn(&mut Nvml) -> Option<f64>) -> Box<dyn Source> {
    Box::new(Blocking {
        id,
        period,
        needs: None,
        open: || Nvml::init().ok(),
        read,
    })
}

fn gpu_temp(n: &mut Nvml) -> Option<f64> {
    use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
    let t = n
        .device_by_index(0)
        .ok()?
        .temperature(TemperatureSensor::Gpu);
    t.ok().map(f64::from)
}

fn gpu_load(n: &mut Nvml) -> Option<f64> {
    let u = n.device_by_index(0).ok()?.utilization_rates().ok()?;
    Some(f64::from(u.gpu))
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn polled(
    id: SourceId,
    period: Duration,
    read: impl Fn() -> Option<f64> + Send + Sync + 'static,
) -> Box<dyn Source> {
    Box::new(crate::Polled {
        id,
        period,
        read: std::sync::Arc::new(read),
    })
}
