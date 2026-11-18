// SPDX-License-Identifier: GPL-2.0-or-later
//! Diagnostics for the window: OpenRGB's log, and what OpenRGB sees on this machine (SMBus
//! buses, USB and HID devices, serial ports). Read-only; both need a local connection.

use freergb_sdk::diag as wire;
use serde::Serialize;

/// Whether OpenRGB is keeping a log freeRGB can show.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogState {
    /// Not connected, or connected from another computer.
    Unavailable,
    /// OpenRGB's "Enable Log Console" setting is off.
    Off,
    /// The setting is on now, but OpenRGB reads it only when it starts.
    Waiting,
    On,
}

impl LogState {
    /// On a new connection: the saved setting is taken as what the running OpenRGB started
    /// with (it is read only at startup).
    pub fn at_connect(local: bool, log_console: Option<bool>) -> Self {
        match (local, log_console) {
            (false, _) => LogState::Unavailable,
            (true, Some(true)) => LogState::On,
            _ => LogState::Off,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub seq: u64,
    /// OpenRGB's level: 0 fatal .. 6 trace, 7 dialog.
    pub level: u32,
    /// Since OpenRGB started.
    pub time_ms: u32,
    pub file: String,
    pub line: u32,
    pub text: String,
}

impl LogLine {
    pub fn new(seq: u64, e: wire::LogEntry) -> Self {
        LogLine {
            seq,
            level: e.level,
            time_ms: e.time_ms,
            file: e.file,
            line: e.line,
            text: e.text,
        }
    }
}

/// The window's view of the log. `epoch` changes when the lines start over (a new
/// connection, or the log was cleared); a window on another epoch takes all lines again.
#[derive(Serialize, Clone, Debug)]
pub struct LogView {
    pub epoch: u64,
    pub state: LogState,
    /// freeRGB started this OpenRGB, so it can restart it.
    pub owned: bool,
    pub lines: Vec<LogLine>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Bus {
    pub name: String,
    pub bus: i32,
    pub port: i32,
    pub pci_vendor: i32,
    pub pci_device: i32,
    pub sub_vendor: i32,
    pub sub_device: i32,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Usb {
    pub vendor: u16,
    pub product: u16,
    pub manufacturer: String,
    pub name: String,
    pub serial: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Hid {
    pub vendor: u16,
    pub product: u16,
    pub manufacturer: String,
    pub name: String,
    pub serial: String,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
    pub path: String,
}

/// A serial port, with its USB adapter's ids when it is one.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Serial {
    pub port: String,
    pub vendor: Option<u16>,
    pub product: Option<u16>,
    pub usb_path: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Hardware {
    pub buses: Vec<Bus>,
    pub usb: Vec<Usb>,
    pub hid: Vec<Hid>,
    pub serial: Vec<Serial>,
}

impl Hardware {
    pub fn new(
        buses: Vec<wire::I2cBus>,
        hid: Vec<wire::HidDevice>,
        usb: Vec<wire::UsbDevice>,
        ports: Vec<String>,
        usb_ports: Vec<wire::UsbSerialPort>,
    ) -> Self {
        let mut serial: Vec<Serial> = ports
            .into_iter()
            .map(|port| Serial {
                port,
                vendor: None,
                product: None,
                usb_path: None,
            })
            .collect();
        // The USB list names the same ports; merge, and keep any the plain list missed.
        for u in usb_ports {
            let at = match serial.iter().position(|s| s.port == u.port) {
                Some(i) => i,
                None => {
                    serial.push(Serial {
                        port: u.port.clone(),
                        vendor: None,
                        product: None,
                        usb_path: None,
                    });
                    serial.len() - 1
                }
            };
            let s = &mut serial[at];
            s.vendor = Some(u.vendor_id);
            s.product = Some(u.product_id);
            s.usb_path = Some(u.usb_path);
        }
        Hardware {
            buses: buses
                .into_iter()
                .map(|b| Bus {
                    name: b.name,
                    bus: b.bus_id,
                    port: b.port_id,
                    pci_vendor: b.pci_vendor,
                    pci_device: b.pci_device,
                    sub_vendor: b.pci_subsystem_vendor,
                    sub_device: b.pci_subsystem_device,
                })
                .collect(),
            usb: usb
                .into_iter()
                .map(|u| Usb {
                    vendor: u.vendor_id,
                    product: u.product_id,
                    manufacturer: u.manufacturer,
                    name: u.product,
                    serial: u.serial,
                })
                .collect(),
            hid: hid
                .into_iter()
                .map(|h| Hid {
                    vendor: h.vendor_id,
                    product: h.product_id,
                    manufacturer: h.manufacturer,
                    name: h.product,
                    serial: h.serial,
                    usage_page: h.usage_page,
                    usage: h.usage,
                    interface: h.interface,
                    path: h.path,
                })
                .collect(),
            serial,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_state_follows_the_saved_setting_on_a_local_connection() {
        assert_eq!(
            LogState::at_connect(false, Some(true)),
            LogState::Unavailable
        );
        assert_eq!(LogState::at_connect(true, Some(true)), LogState::On);
        assert_eq!(LogState::at_connect(true, Some(false)), LogState::Off);
        assert_eq!(LogState::at_connect(true, None), LogState::Off, "unset");
        assert_eq!(
            serde_json::to_string(&LogState::Waiting).unwrap(),
            r#""waiting""#
        );
    }

    #[test]
    fn serial_ports_take_their_usb_adapter_and_none_is_lost() {
        let usb = |port: &str| wire::UsbSerialPort {
            vendor_id: 0x1a86,
            product_id: 0x7523,
            port: port.into(),
            usb_path: "1-4".into(),
        };
        let h = Hardware::new(
            vec![],
            vec![],
            vec![],
            vec!["COM1".into(), "COM3".into()],
            vec![usb("COM3"), usb("COM9")],
        );
        let ports: Vec<_> = h
            .serial
            .iter()
            .map(|s| (s.port.as_str(), s.vendor))
            .collect();
        assert_eq!(
            ports,
            [
                ("COM1", None),
                ("COM3", Some(0x1a86)),
                ("COM9", Some(0x1a86))
            ]
        );
    }
}
