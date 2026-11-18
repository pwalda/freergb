// SPDX-License-Identifier: GPL-2.0-or-later
//! What the sensor service and the app share: the pipe, the message, and turning the CPU's
//! registers into degrees. Windows gives a normal program no CPU temperature, and PawnIO (the
//! driver that can read it) opens only for administrators, so a small service running as
//! SYSTEM reads it and the app asks for it.

use serde::{Deserialize, Serialize};

/// Read-only for everyone on this PC (the default for a pipe a SYSTEM service makes), and
/// closed to the network.
pub const PIPE: &str = r"\\.\pipe\freergb-sensors";
pub const SERVICE: &str = "freeRGB-Sensors";

/// One answer per connection, as a line of JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// Degrees Celsius.
    pub cpu_temp: Option<f64>,
    /// Why there is no temperature, in words for the window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

impl Reading {
    pub fn parse(line: &str) -> Option<Reading> {
        serde_json::from_str(line.trim()).ok()
    }
    pub fn line(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap_or_default();
        s.push('\n');
        s
    }
}

/// Intel: `IA32_TEMPERATURE_TARGET` (0x1A2) holds TjMax in bits 23:16; a thermal status MSR
/// (package 0x1B1, or the core's 0x19C) holds how far below it the CPU is in bits 22:16,
/// valid when bit 31 is set.
pub fn intel_temp(target: u64, status: u64) -> Option<f64> {
    if status & (1 << 31) == 0 {
        return None;
    }
    let tjmax = match (target >> 16) & 0xFF {
        0 => 100,
        t => t,
    };
    let below = (status >> 16) & 0x7F;
    Some(tjmax.saturating_sub(below) as f64)
}

/// AMD Zen (families 17h to 1Ah): SMN 0x59800 holds Tctl in eighths of a degree in bits
/// 31:21; with bit 19 set the range starts at -49 °C (as Linux's k10temp reads it).
pub fn amd_tctl(reg: u32) -> f64 {
    let t = f64::from((reg >> 21) & 0x7FF) / 8.0;
    if reg & (1 << 19) != 0 {
        t - 49.0
    } else {
        t
    }
}

/// Tctl runs ahead of the real temperature on some early Ryzen models; k10temp's table.
pub fn amd_offset(brand: &str) -> f64 {
    const TABLE: [(&str, f64); 6] = [
        ("AMD Ryzen 5 1600X", 20.0),
        ("AMD Ryzen 7 1700X", 20.0),
        ("AMD Ryzen 7 1800X", 20.0),
        ("AMD Ryzen 7 2700X", 10.0),
        ("AMD Ryzen Threadripper 19", 27.0),
        ("AMD Ryzen Threadripper 29", 27.0),
    ];
    TABLE
        .iter()
        .find(|(name, _)| brand.contains(name))
        .map_or(0.0, |(_, o)| *o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intel_reads_degrees_below_tjmax() {
        // TjMax 100, 46 below: 54 °C.
        let target = 100 << 16;
        let status = (1 << 31) | (46 << 16);
        assert_eq!(intel_temp(target, status), Some(54.0));
        assert_eq!(intel_temp(target, 46 << 16), None, "not valid");
        assert_eq!(intel_temp(0, status), Some(54.0), "no TjMax: 100");
        assert_eq!(intel_temp(90 << 16, (1 << 31) | (100 << 16)), Some(0.0));
    }

    #[test]
    fn amd_reads_tctl_in_eighths() {
        assert_eq!(amd_tctl(((55 * 8 + 2) as u32) << 21), 55.25);
        assert_eq!(amd_tctl((((100 * 8) as u32) << 21) | (1 << 19)), 51.0);
        assert_eq!(amd_offset("AMD Ryzen 7 1800X Eight-Core Processor"), 20.0);
        assert_eq!(
            amd_offset("AMD Ryzen Threadripper 2950X 16-Core Processor"),
            27.0
        );
        assert_eq!(amd_offset("AMD Ryzen 9 7950X 16-Core Processor"), 0.0);
    }

    #[test]
    fn a_reading_is_one_json_line() {
        let r = Reading {
            cpu_temp: Some(48.5),
            problem: None,
        };
        assert_eq!(r.line(), "{\"cpu_temp\":48.5}\n");
        assert_eq!(Reading::parse(&r.line()), Some(r));
        let p = Reading::parse(r#"{"cpu_temp":null,"problem":"PawnIO is not installed"}"#).unwrap();
        assert_eq!(p.problem.as_deref(), Some("PawnIO is not installed"));
        assert_eq!(Reading::parse("garbage"), None);
    }
}
