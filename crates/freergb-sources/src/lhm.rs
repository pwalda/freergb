// SPDX-License-Identifier: GPL-2.0-or-later
//! The CPU temperature on Windows: Windows itself gives no CPU temperature to a normal
//! program. freeRGB's sensor service (freergb-sensord, through PawnIO) answers first; else
//! LibreHardwareMonitor, which publishes its sensors over WMI (namespace
//! `ROOT\LibreHardwareMonitor`, class `Sensor`) while it runs.

use serde::Deserialize;

/// What a look needs for this reading, shown while there is none.
pub const NEEDS: &str =
    "Needs PawnIO (the freeRGB installer offers it) or LibreHardwareMonitor running";

/// One temperature sensor as LibreHardwareMonitor lists it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Sensor", rename_all = "PascalCase")]
pub struct Sensor {
    pub name: String,
    /// The hardware's identifier, such as `/amdcpu/0` or `/intelcpu/0`.
    pub parent: String,
    pub value: f32,
}

/// The CPU sensors' names for the whole package, best first (Intel, then AMD's).
const PACKAGE: [&str; 7] = [
    "CPU Package",
    "Core (Tctl/Tdie)",
    "Core (Tdie)",
    "Core (Tctl)",
    "Package",
    "Core Max",
    "Core Average",
];

/// The CPU temperature among `sensors`: the package sensor, else the hottest CPU sensor that
/// is a temperature (not a distance to the limit).
pub fn cpu_temp(sensors: &[Sensor]) -> Option<f64> {
    let cpu: Vec<&Sensor> = sensors
        .iter()
        .filter(|s| s.parent.starts_with("/intelcpu") || s.parent.starts_with("/amdcpu"))
        .filter(|s| s.value.is_finite() && s.value > 0.0)
        .collect();
    PACKAGE
        .iter()
        .find_map(|want| cpu.iter().find(|s| s.name == *want))
        .or_else(|| {
            cpu.iter()
                .filter(|s| !s.name.contains("Distance"))
                .max_by(|a, b| a.value.total_cmp(&b.value))
        })
        .map(|s| f64::from(s.value))
}

#[cfg(windows)]
pub use wmi_source::source;

#[cfg(windows)]
mod wmi_source {
    use super::{cpu_temp, Sensor, NEEDS};
    use crate::{Blocking, Source};
    use freergb_effects::react::SourceId;
    use std::io::{BufRead, BufReader, Read};
    use std::time::Duration;
    use wmi::WMIConnection;

    enum Via {
        Service,
        Lhm(WMIConnection),
    }

    /// One line from the sensor service, which answers each connection once. Read up to the
    /// newline, not to the end: the service closing its end is an error on a pipe, not an EOF.
    fn ask_service() -> Option<f64> {
        let pipe = std::fs::File::open(freergb_sensord::PIPE).ok()?;
        let mut line = String::new();
        BufReader::new(pipe.take(4096)).read_line(&mut line).ok()?;
        freergb_sensord::Reading::parse(&line)?.cpu_temp
    }

    fn lhm(c: &mut WMIConnection) -> Option<f64> {
        let rows: Vec<Sensor> = c
            .raw_query("SELECT Name, Parent, Value FROM Sensor WHERE SensorType = 'Temperature'")
            .ok()?;
        cpu_temp(&rows)
    }

    /// Read every second. A failed read drops the choice, so the next tick looks again,
    /// service first: either may start (or stop) while the look runs.
    pub fn source() -> Box<dyn Source> {
        Box::new(Blocking {
            id: SourceId::CpuTemp,
            period: Duration::from_secs(1),
            needs: Some(NEEDS),
            open: || {
                if ask_service().is_some() {
                    return Some(Via::Service);
                }
                WMIConnection::with_namespace_path("ROOT\\LibreHardwareMonitor")
                    .ok()
                    .map(Via::Lhm)
            },
            read: |via: &mut Via| match via {
                Via::Service => ask_service(),
                Via::Lhm(c) => lhm(c),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(parent: &str, name: &str, value: f32) -> Sensor {
        Sensor {
            name: name.into(),
            parent: parent.into(),
            value,
        }
    }

    #[test]
    fn the_cpu_temperature_is_the_package_sensor_of_the_cpu() {
        let intel = [
            s("/gpu-nvidia/0", "GPU Core", 58.0),
            s("/intelcpu/0", "CPU Core #1", 49.0),
            s("/intelcpu/0", "Core Distance to TjMax #1", 51.0),
            s("/intelcpu/0", "CPU Package", 54.0),
        ];
        assert_eq!(cpu_temp(&intel), Some(54.0));

        let amd = [
            s("/amdcpu/0", "CCD1 (Tdie)", 61.5),
            s("/amdcpu/0", "Core (Tctl/Tdie)", 63.25),
        ];
        assert_eq!(cpu_temp(&amd), Some(63.25));

        // No package sensor: the hottest CPU temperature, never a distance to the limit.
        let other = [
            s("/amdcpu/0", "CCD1 (Tdie)", 61.0),
            s("/amdcpu/0", "CCD2 (Tdie)", 66.0),
            s("/amdcpu/0", "Core Distance to TjMax", 90.0),
        ];
        assert_eq!(cpu_temp(&other), Some(66.0));

        assert_eq!(cpu_temp(&[s("/gpu-amd/0", "GPU Core", 50.0)]), None);
        assert_eq!(cpu_temp(&[]), None);
    }

    #[test]
    fn a_sensor_row_reads_from_wmi_property_names() {
        let row: Sensor =
            serde_json::from_str(r#"{"Name":"CPU Package","Parent":"/intelcpu/0","Value":51.5}"#)
                .unwrap();
        assert_eq!(cpu_temp(&[row]), Some(51.5));
    }
}
