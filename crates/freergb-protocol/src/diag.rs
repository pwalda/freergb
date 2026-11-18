// SPDX-License-Identifier: GPL-2.0-or-later
//! Diagnostics payloads: what the server sees on the machine (120-124) and its log (300-304).
//! Layouts follow OpenRGB release_1.1 `NetworkServer.cpp`; every list starts with
//! `data_size` (u32) and a `u16` count. All of these are local-client only on the server.

use crate::error::{ProtocolError, Result};
use crate::wire::{Reader, Writer};

/// `i2c_smbus_info.device_name` is a fixed `char[512]`, sent as raw struct bytes.
const BUS_NAME_LEN: usize = 512;
const BUS_LEN: usize = BUS_NAME_LEN + 6 * 4;

/// One SMBus/I2C bus (GET_I2C_BUS_INFO, 120). PCI ids are -1 or 0 where the bus has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I2cBus {
    pub name: String,
    pub port_id: i32,
    pub pci_device: i32,
    pub pci_vendor: i32,
    pub pci_subsystem_device: i32,
    pub pci_subsystem_vendor: i32,
    pub bus_id: i32,
}

/// One HID interface (GET_HID_DEVICE_INFO, 121), as hidapi enumerates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    pub release: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
    pub serial: String,
    pub manufacturer: String,
    pub product: String,
    pub path: String,
}

/// One USB device (GET_USB_DEVICE_INFO, 122), as libusb lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: String,
    pub manufacturer: String,
    pub product: String,
}

/// One USB serial adapter (GET_USB_SERIAL_PORTS, 124).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbSerialPort {
    pub vendor_id: u16,
    pub product_id: u16,
    pub port: String,
    pub usb_path: String,
}

/// One server log line (LOG_LOGGED_ENTRY, 304). `level` is OpenRGB's: 0 fatal, 1 error,
/// 2 warning, 3 info, 4 verbose, 5 debug, 6 trace, 7 dialog. `time_ms` counts from the
/// server's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub level: u32,
    pub line: u32,
    pub time_ms: u32,
    pub file: String,
    pub text: String,
}

fn count(n: usize, what: &'static str) -> Result<u16> {
    u16::try_from(n).map_err(|_| ProtocolError::TooLargeToEncode(what))
}

fn sized(w: Writer) -> Vec<u8> {
    let body = w.into_vec();
    let mut out = ((body.len() + 4) as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&body);
    out
}

/// Decode `data_size`, `u16` count, then `count` items of at least `min` bytes each.
fn list<T>(p: &[u8], min: usize, mut item: impl FnMut(&mut Reader) -> Result<T>) -> Result<Vec<T>> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let n = r.u16()? as usize;
    r.check_count(n, min)?;
    let out = (0..n).map(|_| item(&mut r)).collect::<Result<Vec<_>>>()?;
    r.finish()?;
    Ok(out)
}

pub fn encode_i2c_buses(buses: &[I2cBus]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u16(count(buses.len(), "bus list")?);
    for b in buses {
        let mut name = [0u8; BUS_NAME_LEN];
        let s = b.name.as_bytes();
        let n = s.len().min(BUS_NAME_LEN - 1);
        name[..n].copy_from_slice(&s[..n]);
        w.bytes(&name);
        for v in [
            b.port_id,
            b.pci_device,
            b.pci_vendor,
            b.pci_subsystem_device,
            b.pci_subsystem_vendor,
            b.bus_id,
        ] {
            w.i32(v);
        }
    }
    Ok(sized(w))
}

pub fn decode_i2c_buses(p: &[u8]) -> Result<Vec<I2cBus>> {
    list(p, BUS_LEN, |r| {
        let raw = r.bytes(BUS_NAME_LEN)?;
        let end = raw.iter().position(|&b| b == 0).unwrap_or(BUS_NAME_LEN);
        Ok(I2cBus {
            name: String::from_utf8_lossy(&raw[..end]).into_owned(),
            port_id: r.i32()?,
            pci_device: r.i32()?,
            pci_vendor: r.i32()?,
            pci_subsystem_device: r.i32()?,
            pci_subsystem_vendor: r.i32()?,
            bus_id: r.i32()?,
        })
    })
}

pub fn encode_hid_devices(devices: &[HidDevice]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u16(count(devices.len(), "HID list")?);
    for d in devices {
        for v in [d.vendor_id, d.product_id, d.release, d.usage_page, d.usage] {
            w.u16(v);
        }
        w.i32(d.interface);
        for s in [&d.serial, &d.manufacturer, &d.product, &d.path] {
            w.string16(s)?;
        }
    }
    Ok(sized(w))
}

pub fn decode_hid_devices(p: &[u8]) -> Result<Vec<HidDevice>> {
    list(p, 5 * 2 + 4 + 4 * 3, |r| {
        Ok(HidDevice {
            vendor_id: r.u16()?,
            product_id: r.u16()?,
            release: r.u16()?,
            usage_page: r.u16()?,
            usage: r.u16()?,
            interface: r.i32()?,
            serial: r.string16()?,
            manufacturer: r.string16()?,
            product: r.string16()?,
            path: r.string16()?,
        })
    })
}

pub fn encode_usb_devices(devices: &[UsbDevice]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u16(count(devices.len(), "USB list")?);
    for d in devices {
        w.u16(d.vendor_id);
        w.u16(d.product_id);
        for s in [&d.serial, &d.manufacturer, &d.product] {
            w.string16(s)?;
        }
    }
    Ok(sized(w))
}

pub fn decode_usb_devices(p: &[u8]) -> Result<Vec<UsbDevice>> {
    list(p, 2 * 2 + 3 * 3, |r| {
        Ok(UsbDevice {
            vendor_id: r.u16()?,
            product_id: r.u16()?,
            serial: r.string16()?,
            manufacturer: r.string16()?,
            product: r.string16()?,
        })
    })
}

/// GET_SERIAL_PORTS (123): port names (`COM3`, `/dev/ttyUSB0`).
pub fn encode_serial_ports(ports: &[String]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u16(count(ports.len(), "serial port list")?);
    for s in ports {
        w.string16(s)?;
    }
    Ok(sized(w))
}

pub fn decode_serial_ports(p: &[u8]) -> Result<Vec<String>> {
    list(p, 3, |r| r.string16())
}

pub fn encode_usb_serial_ports(ports: &[UsbSerialPort]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u16(count(ports.len(), "USB serial list")?);
    for d in ports {
        w.u16(d.vendor_id);
        w.u16(d.product_id);
        w.string16(&d.port)?;
        w.string16(&d.usb_path)?;
    }
    Ok(sized(w))
}

pub fn decode_usb_serial_ports(p: &[u8]) -> Result<Vec<UsbSerialPort>> {
    list(p, 2 * 2 + 2 * 3, |r| {
        Ok(UsbSerialPort {
            vendor_id: r.u16()?,
            product_id: r.u16()?,
            port: r.string16()?,
            usb_path: r.string16()?,
        })
    })
}

/// LOG_LOGGED_ENTRY (304): `data_size`, level, line, timestamp (u32 each), then the source
/// file name and the text as `string16`. Sent for each buffered line after LOG_GET_BUFFER
/// (301) and live for each new one.
pub fn encode_log_entry(e: &LogEntry) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u32(e.level);
    w.u32(e.line);
    w.u32(e.time_ms);
    w.string16(&e.file)?;
    w.string16(&e.text)?;
    Ok(sized(w))
}

pub fn decode_log_entry(p: &[u8]) -> Result<LogEntry> {
    let mut r = Reader::new(p);
    let _size = r.u32()?;
    let e = LogEntry {
        level: r.u32()?,
        line: r.u32()?,
        time_ms: r.u32()?,
        file: r.string16()?,
        text: r.string16()?,
    };
    r.finish()?;
    Ok(e)
}
