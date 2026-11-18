// SPDX-License-Identifier: GPL-2.0-or-later

use freergb_protocol::ids::*;
use freergb_protocol::messages::*;
use freergb_protocol::*;

fn sample() -> ControllerData {
    let mode = |name: &str| Mode {
        name: name.into(),
        value: 7,
        flags: mode_flag::HAS_SPEED | mode_flag::HAS_BRIGHTNESS | mode_flag::HAS_PER_LED_COLOR,
        speed_min: 1,
        speed_max: 100,
        brightness_min: 0,
        brightness_max: 255,
        colors_min: 1,
        colors_max: 2,
        speed: 50,
        brightness: 200,
        direction: direction::LEFT,
        color_mode: color_mode::MODE_SPECIFIC,
        colors: vec![Color::new(255, 0, 0), Color::new(0, 0, 255)],
    };
    ControllerData {
        device_type: device_type::MOTHERBOARD,
        name: "Test Board".into(),
        vendor: "ACME".into(),
        description: "Fake motherboard".into(),
        version: "1.2".into(),
        serial: "SN123".into(),
        location: "I2C: 0x29".into(),
        active_mode: 0,
        modes: vec![mode("Direct"), mode("Rainbow")],
        zones: vec![
            Zone {
                name: "ARGB 1".into(),
                zone_type: zone_type::LINEAR,
                leds_min: 1,
                leds_max: 300,
                leds_count: 3,
                flags: zone_flag::CONFIGURABLE_SIZE,
                segments: vec![Segment {
                    name: "first".into(),
                    segment_type: zone_type::LINEAR,
                    start_idx: 0,
                    leds_count: 2,
                    matrix_map: Some(MatrixMap {
                        height: 1,
                        width: 2,
                        data: vec![0, 1],
                    }),
                    flags: 1,
                }],
                modes: vec![mode("Zone Direct")],
                display_name: "Header One".into(),
                ..Default::default()
            },
            Zone {
                name: "Grid".into(),
                zone_type: zone_type::MATRIX,
                leds_count: 4,
                matrix_map: Some(MatrixMap {
                    height: 2,
                    width: 2,
                    data: vec![0, 1, 2, u32::MAX],
                }),
                ..Default::default()
            },
        ],
        leds: (0..3)
            .map(|i| Led {
                name: format!("LED {i}"),
                value: i,
            })
            .collect(),
        colors: vec![Color::new(1, 2, 3); 3],
        led_display_names: vec!["a".into(), "b".into(), "c".into()],
        flags: controller_flag::LOCAL,
        display_name: "My Board".into(),
        configuration: "{\"k\":1}".into(),
    }
}

/// Fields not carried on the wire at protocol `v` come back as defaults.
fn degrade(mut c: ControllerData, v: u32) -> ControllerData {
    let fix_mode = |m: &mut Mode| {
        if v < 6 { /* value kept */
        } else {
            m.value = 0
        }
        if v < 3 {
            m.brightness_min = 0;
            m.brightness_max = 0;
            m.brightness = 0;
        }
    };
    if v < 1 {
        c.vendor.clear();
    }
    c.modes.iter_mut().for_each(fix_mode);
    for z in &mut c.zones {
        z.modes.iter_mut().for_each(fix_mode);
        if v < 4 {
            z.segments.clear();
        }
        if v < 5 {
            z.flags = 0;
        }
        if v < 6 {
            z.modes.clear();
            z.active_mode = 0;
            z.display_name.clear();
        }
        for s in &mut z.segments {
            if v < 6 {
                s.matrix_map = None;
                s.flags = 0;
            }
        }
    }
    if v >= 6 {
        c.leds.iter_mut().for_each(|l| l.value = 0);
    }
    if v < 5 {
        c.led_display_names.clear();
        c.flags = 0;
    }
    if v < 6 {
        c.display_name.clear();
        c.configuration.clear();
    }
    c
}

#[test]
fn controller_roundtrips_at_every_protocol_version() {
    for v in 0..=MAX_PROTOCOL_VERSION {
        let want = degrade(sample(), v);
        let bytes = sample().encode_packet(v).unwrap();
        let got = ControllerData::decode_packet(&bytes, v).unwrap();
        assert_eq!(got, want, "protocol {v}");
    }
}

#[test]
fn data_size_includes_itself() {
    let bytes = sample().encode_packet(6).unwrap();
    assert_eq!(
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize,
        bytes.len()
    );
}

#[test]
fn every_truncated_block_is_an_error_not_a_panic() {
    // Exercise the inner bounds checks directly: `decode_packet` would reject
    // most cuts on the outer data_size check without ever parsing the block.
    for v in 0..=MAX_PROTOCOL_VERSION {
        let block = sample().encode_block(v).unwrap();
        for cut in 0..block.len() {
            let mut r = freergb_protocol::wire::Reader::new(&block[..cut]);
            assert!(
                ControllerData::decode_block(&mut r, v).is_err(),
                "protocol {v}, cut at {cut}"
            );
        }
    }
}

#[test]
fn single_byte_corruption_never_panics() {
    // Deterministic mutation sweep. Result may be Ok or Err; it must return,
    // and must not allocate absurd amounts (counts are bounds-checked).
    for v in [0, 3, 5, 6] {
        let block = sample().encode_block(v).unwrap();
        for i in 0..block.len() {
            for &b in &[0x00u8, 0x01, 0x7f, 0x80, 0xfe, 0xff] {
                let mut m = block.clone();
                m[i] = b;
                let mut r = freergb_protocol::wire::Reader::new(&m);
                let _ = ControllerData::decode_block(&mut r, v);
            }
        }
    }
}

#[test]
fn empty_matrix_encodes_as_length_zero_like_upstream() {
    let mut c = sample();
    c.zones[1].matrix_map = Some(MatrixMap {
        height: 0,
        width: 0,
        data: vec![],
    });
    let got = ControllerData::decode_packet(&c.encode_packet(6).unwrap(), 6).unwrap();
    assert_eq!(got.zones[1].matrix_map, None);
}

#[test]
fn inconsistent_matrix_is_rejected_on_encode_and_decode() {
    let mut c = sample();
    c.zones[1].matrix_map = Some(MatrixMap {
        height: 2,
        width: 2,
        data: vec![0],
    });
    assert!(c.encode_block(6).is_err());
    // Decode: matrix length that does not match height*width.
    let mut w = freergb_protocol::wire::Writer::new();
    w.string16("z").unwrap();
    w.i32(2);
    w.u32(0);
    w.u32(0);
    w.u32(0);
    w.u16(12); // claims 12 bytes: 8 header + 4 data
    w.u32(2);
    w.u32(2); // but 2x2 needs 16 bytes of data
    w.u32(0);
    let bytes = w.into_vec();
    let mut r = freergb_protocol::wire::Reader::new(&bytes);
    assert!(freergb_protocol::Zone::read(&mut r, 3).is_err());
}

#[test]
fn hostile_counts_do_not_allocate() {
    // device_type, then a name claiming 65535 bytes in a tiny buffer.
    let mut b = 0i32.to_le_bytes().to_vec();
    b.extend_from_slice(&u16::MAX.to_le_bytes());
    b.extend_from_slice(b"short\0");
    assert!(ControllerData::decode_packet(
        &[&(b.len() as u32 + 4).to_le_bytes()[..], &b].concat(),
        6
    )
    .is_err());
}

#[test]
fn large_device_fits() {
    let mut c = sample();
    c.leds = (0..300)
        .map(|i| Led {
            name: format!("LED {i}"),
            value: 0,
        })
        .collect();
    c.colors = vec![Color::default(); 300];
    let bytes = c.encode_packet(6).unwrap();
    assert_eq!(
        ControllerData::decode_packet(&bytes, 6).unwrap().leds.len(),
        300
    );
}

#[test]
fn packet_framing_streams() {
    let p = Packet::new(42, pkt::UPDATE_LEDS, vec![1, 2, 3]);
    let bytes = p.encode();
    for cut in 0..bytes.len() {
        assert_eq!(Packet::decode(&bytes[..cut]).unwrap(), None, "cut {cut}");
    }
    let (got, used) = Packet::decode(&bytes).unwrap().unwrap();
    assert_eq!((got, used), (p, bytes.len()));
}

#[test]
fn bad_magic_and_oversize_are_errors() {
    let mut bytes = Packet::new(0, 0, vec![]).encode();
    bytes[0] = b'X';
    assert!(matches!(
        Packet::decode(&bytes),
        Err(ProtocolError::BadMagic(_))
    ));
    let mut big = Packet::new(0, 0, vec![]).encode();
    big[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Packet::decode(&big),
        Err(ProtocolError::PacketTooLarge(..))
    ));
}

#[test]
fn header_layout_is_documented_layout() {
    let bytes = Packet::new(0x0102_0304, pkt::ACK, vec![9]).encode();
    assert_eq!(&bytes[0..4], b"ORGB");
    assert_eq!(&bytes[4..8], &[4, 3, 2, 1]);
    assert_eq!(&bytes[8..12], &10u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &1u32.to_le_bytes());
}

#[test]
fn controller_list_v6_has_ids_v5_is_count_only() {
    let v6 = encode_controller_list(&[7, 9], 6);
    assert_eq!(
        decode_controller_list(&v6, 6).unwrap(),
        ControllerList {
            count: 2,
            ids: vec![7, 9]
        }
    );
    let v5 = encode_controller_list(&[7, 9], 5);
    assert_eq!(
        decode_controller_list(&v5, 5).unwrap(),
        ControllerList {
            count: 2,
            ids: vec![]
        }
    );
}

#[test]
fn command_packets_roundtrip() {
    let colors = vec![Color::new(1, 2, 3), Color::new(4, 5, 6)];
    assert_eq!(
        decode_update_leds(&encode_update_leds(&colors).unwrap()).unwrap(),
        colors
    );
    assert_eq!(
        decode_update_zone_leds(&encode_update_zone_leds(3, &colors).unwrap()).unwrap(),
        (3, colors)
    );
    assert_eq!(
        decode_resize_zone(&encode_resize_zone(1, 60)).unwrap(),
        (1, 60)
    );
    assert_eq!(
        decode_ack(&encode_ack(pkt::UPDATE_LEDS, status::OK)).unwrap(),
        (pkt::UPDATE_LEDS, 0)
    );
    let m = &sample().modes[0];
    assert_eq!(
        decode_update_mode(&encode_update_mode(2, m, 6).unwrap(), 6)
            .unwrap()
            .0,
        2
    );
    let (z, i, got) =
        decode_update_zone_mode(&encode_update_zone_mode(1, 0, Some(m), 6).unwrap(), 6).unwrap();
    assert_eq!((z, i), (1, 0));
    assert_eq!(got.unwrap().name, m.name);
    // -1 = follow the device's mode: no mode data, so the size field counts 12 bytes.
    let follow = encode_update_zone_mode(2, -1, Some(m), 6).unwrap();
    assert_eq!(follow, [12, 0, 0, 0, 2, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(decode_update_zone_mode(&follow, 6).unwrap(), (2, -1, None));
}

#[test]
fn segment_packets_match_the_upstream_layout() {
    // ProcessRequest_RGBController_ClearSegments wants exactly the 4-byte zone index.
    assert_eq!(encode_clear_segments(3), [3, 0, 0, 0]);
    assert_eq!(decode_clear_segments(&[3, 0, 0, 0]).unwrap(), 3);
    let s = Segment {
        name: "Desk".into(),
        segment_type: zone_type::LINEAR,
        start_idx: 10,
        leds_count: 20,
        matrix_map: None,
        flags: 0,
    };
    let p = encode_add_segment(1, &s, 6).unwrap();
    // size, zone, then name (u16 length with NUL), type, start, count, matrix size 0, flags.
    let size = 4 + 4 + 2 + 5 + 4 + 4 + 4 + 2 + 4;
    assert_eq!(p.len(), size);
    assert_eq!(p[..8], [size as u8, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(decode_add_segment(&p, 6).unwrap(), (1, s));
}

#[test]
fn detection_progress_matches_the_upstream_layout() {
    // NetworkServer::SendRequest_DetectionProgress: size, percent, u16 length with NUL, text.
    let p = encode_detection_progress(42, "Corsair").unwrap();
    let mut want = 18u32.to_le_bytes().to_vec();
    want.extend_from_slice(&42u32.to_le_bytes());
    want.extend_from_slice(&8u16.to_le_bytes());
    want.extend_from_slice(b"Corsair\0");
    assert_eq!(p, want);
    assert_eq!(
        decode_detection_progress(&p).unwrap(),
        (42, "Corsair".into())
    );
    assert!(decode_detection_progress(&p[..p.len() - 1]).is_err());
}

#[test]
fn bus_info_is_the_raw_struct_with_a_fixed_name() {
    use freergb_protocol::diag::*;
    // ProcessRequest_GetI2CBusInfo memcpys i2c_smbus_info: char[512] name, six i32.
    let bus = I2cBus {
        name: "NVidia NV3 on port 0".into(),
        port_id: 1,
        pci_device: 0x2204,
        pci_vendor: 0x10de,
        pci_subsystem_device: 0x3881,
        pci_subsystem_vendor: 0x1462,
        bus_id: -1,
    };
    let p = encode_i2c_buses(std::slice::from_ref(&bus)).unwrap();
    assert_eq!(p.len(), 4 + 2 + 512 + 24);
    assert_eq!(p[..6], [(4 + 2 + 536) as u8, 2, 0, 0, 1, 0]);
    assert_eq!(&p[6..10], b"NVid");
    assert_eq!(p[6 + 512..6 + 516], 1i32.to_le_bytes());
    assert_eq!(decode_i2c_buses(&p).unwrap(), [bus]);
    assert!(decode_i2c_buses(&p[..p.len() - 1]).is_err());
}

#[test]
fn device_lists_follow_the_upstream_field_order() {
    use freergb_protocol::diag::*;
    let hid = HidDevice {
        vendor_id: 0x1b1c,
        product_id: 0x0c1a,
        release: 0x0100,
        usage_page: 0xff42,
        usage: 1,
        interface: 0,
        serial: "".into(),
        manufacturer: "Corsair".into(),
        product: "iCUE H150i".into(),
        path: "/dev/hidraw3".into(),
    };
    let p = encode_hid_devices(std::slice::from_ref(&hid)).unwrap();
    // size, count, five u16, i32 interface, then serial "" as length 1 + NUL.
    assert_eq!(p[6..8], 0x1b1cu16.to_le_bytes());
    assert_eq!(p[6 + 10..6 + 14], 0i32.to_le_bytes());
    assert_eq!(p[6 + 14..6 + 17], [1, 0, 0]);
    assert_eq!(decode_hid_devices(&p).unwrap(), [hid]);

    let usb = UsbDevice {
        vendor_id: 0x046d,
        product_id: 0xc52b,
        serial: "".into(),
        manufacturer: "Logitech".into(),
        product: "USB Receiver".into(),
    };
    let p = encode_usb_devices(std::slice::from_ref(&usb)).unwrap();
    assert_eq!(decode_usb_devices(&p).unwrap(), [usb]);

    let ports = vec!["COM3".to_string(), "/dev/ttyUSB0".into()];
    let p = encode_serial_ports(&ports).unwrap();
    assert_eq!(p[6..8], 5u16.to_le_bytes());
    assert_eq!(decode_serial_ports(&p).unwrap(), ports);

    let us = UsbSerialPort {
        vendor_id: 0x1a86,
        product_id: 0x7523,
        port: "/dev/ttyUSB0".into(),
        usb_path: "1-4".into(),
    };
    let p = encode_usb_serial_ports(std::slice::from_ref(&us)).unwrap();
    assert_eq!(decode_usb_serial_ports(&p).unwrap(), [us]);

    // A count that cannot fit is refused before allocating.
    assert!(decode_hid_devices(&[10, 0, 0, 0, 0xff, 0xff]).is_err());
}

#[test]
fn a_log_entry_matches_the_upstream_layout() {
    use freergb_protocol::diag::*;
    let e = LogEntry {
        level: 3,
        line: 1236,
        time_ms: 81,
        file: "NetworkServer.cpp".into(),
        text: "Network server started".into(),
    };
    let p = encode_log_entry(&e).unwrap();
    let mut want = Vec::new();
    for v in [3u32, 1236, 81] {
        want.extend_from_slice(&v.to_le_bytes());
    }
    want.extend_from_slice(&18u16.to_le_bytes());
    want.extend_from_slice(b"NetworkServer.cpp\0");
    want.extend_from_slice(&23u16.to_le_bytes());
    want.extend_from_slice(b"Network server started\0");
    assert_eq!(p[4..], want);
    assert_eq!(p[..4], ((want.len() + 4) as u32).to_le_bytes());
    assert_eq!(decode_log_entry(&p).unwrap(), e);
}
