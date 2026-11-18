// SPDX-License-Identifier: GPL-2.0-or-later
//! Device identity that survives an OpenRGB restart.
//!
//! Controller IDs are a per-process counter in the server (`controller_next_idx`,
//! `NetworkServer.cpp`): stable across hotplug and rescans within one server process, but
//! assigned in detection order afresh after a restart. Verified live: restarting with one
//! device removed shifted the IDs of every device after it. Anything persisted (profiles,
//! effect targets, per-device settings) must therefore be keyed by [`DeviceKey`].

use freergb_protocol::ControllerData;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What identifies a physical device across server restarts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceKey {
    pub name: String,
    pub location: String,
    pub serial: String,
    /// Disambiguates devices whose other fields are identical (drivers that leave
    /// location and serial empty), by ascending controller ID. Fragile only for
    /// genuinely indistinguishable devices, where nothing better exists.
    pub nth: u32,
}

impl DeviceKey {
    fn base(d: &ControllerData) -> (String, String, String) {
        (d.name.clone(), d.location.clone(), d.serial.clone())
    }
}

/// Keys for a device map, in ascending controller-ID order.
pub fn keys_for(devices: &BTreeMap<u32, ControllerData>) -> Vec<(u32, DeviceKey)> {
    keys_of(devices.iter().map(|(id, d)| (*id, d)))
}

/// Like [`keys_for`] for any `(id, data)` sequence, which must be in ascending ID order
/// (the `nth` tie-break depends on it). `RgbBackend::devices()` already is.
pub fn keys_of<'a>(
    devices: impl IntoIterator<Item = (u32, &'a ControllerData)>,
) -> Vec<(u32, DeviceKey)> {
    let mut seen: BTreeMap<(String, String, String), u32> = BTreeMap::new();
    devices
        .into_iter()
        .map(|(id, d)| {
            let base = DeviceKey::base(d);
            let n = seen.entry(base.clone()).or_insert(0);
            let key = DeviceKey {
                name: base.0,
                location: base.1,
                serial: base.2,
                nth: *n,
            };
            *n += 1;
            (id, key)
        })
        .collect()
}

/// Match persisted identities to the devices present now, one-to-one, in tiers:
/// exact key; then (name, serial) when the serial is non-empty; then (name, nth among the
/// still-unmatched devices of that name). Needed because `location` is not reliably stable for
/// real hardware: upstream builds it from the hidapi path for HID devices (`"HID: " + path`,
/// a `/dev/hidrawN` node on Linux), which can change across replugs and reboots. UNVERIFIED on
/// real hardware. Returns, for each saved key, the current controller ID or `None`.
pub fn resolve(saved: &[DeviceKey], current: &[(u32, DeviceKey)]) -> Vec<Option<u32>> {
    let mut out = vec![None; saved.len()];
    let mut taken = vec![false; current.len()];
    let mut tier = |pred: &dyn Fn(&DeviceKey, &DeviceKey) -> bool, out: &mut Vec<Option<u32>>| {
        for (i, want) in saved.iter().enumerate() {
            if out[i].is_some() {
                continue;
            }
            if let Some(j) = (0..current.len()).find(|&j| !taken[j] && pred(want, &current[j].1)) {
                taken[j] = true;
                out[i] = Some(current[j].0);
            }
        }
    };
    tier(&|a, b| a == b, &mut out);
    tier(
        &|a, b| !a.serial.is_empty() && a.name == b.name && a.serial == b.serial,
        &mut out,
    );
    // Same name, in the order the saved entries were recorded.
    tier(&|a, b| a.name == b.name, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, loc: &str, serial: &str) -> ControllerData {
        ControllerData {
            name: name.into(),
            location: loc.into(),
            serial: serial.into(),
            ..Default::default()
        }
    }

    #[test]
    fn keys_survive_id_renumbering() {
        let before: BTreeMap<_, _> =
            [(5, dev("Strip", "a", "1")), (6, dev("Pad", "b", "2"))].into();
        let after: BTreeMap<_, _> = [(5, dev("Pad", "b", "2"))].into(); // strip gone, ids shifted
        let kb = keys_for(&before);
        let ka = keys_for(&after);
        assert_eq!(kb[1].1, ka[0].1);
        assert_ne!(kb[1].0, ka[0].0, "the raw id did change");
    }

    #[test]
    fn identical_devices_get_distinct_keys() {
        let m: BTreeMap<_, _> = [
            (1, dev("X", "", "")),
            (2, dev("X", "", "")),
            (3, dev("Y", "", "")),
        ]
        .into();
        let k = keys_for(&m);
        assert_eq!(k[0].1.nth, 0);
        assert_eq!(k[1].1.nth, 1);
        assert_eq!(k[2].1.nth, 0);
        assert_ne!(k[0].1, k[1].1);
    }

    fn key(name: &str, loc: &str, serial: &str, nth: u32) -> DeviceKey {
        DeviceKey {
            name: name.into(),
            location: loc.into(),
            serial: serial.into(),
            nth,
        }
    }

    #[test]
    fn resolve_prefers_exact_then_serial_then_name() {
        let saved = vec![
            key("Mouse", "HID: /dev/hidraw3", "S1", 0), // replugged: new hidraw node, same serial
            key("Kbd", "HID: /dev/hidraw5", "", 0),     // no serial: matches by name
            key("Strip", "usb:1", "A", 0),              // unplugged
            key("RAM", "i2c:0x58", "", 0),              // exact
        ];
        let current = vec![
            (0, key("RAM", "i2c:0x58", "", 0)),
            (1, key("Mouse", "HID: /dev/hidraw7", "S1", 0)),
            (2, key("Kbd", "HID: /dev/hidraw9", "", 0)),
        ];
        assert_eq!(
            resolve(&saved, &current),
            vec![Some(1), Some(2), None, Some(0)]
        );
    }

    #[test]
    fn resolve_is_one_to_one() {
        // two saved identical-name devices, only one present: exactly one match, not two
        let saved = vec![key("Pad", "a", "", 0), key("Pad", "b", "", 1)];
        let current = vec![(4, key("Pad", "b", "", 0))];
        let r = resolve(&saved, &current);
        assert_eq!(r.iter().flatten().count(), 1);
        // and an exact match is not stolen by an earlier looser one
        let saved = vec![key("Pad", "zzz", "", 0), key("Pad", "b", "", 0)];
        let r = resolve(&saved, &current);
        assert_eq!(r, vec![None, Some(4)]);
    }
}
