// SPDX-License-Identifier: GPL-2.0-or-later
//! Scriptable mock OpenRGB SDK server. Speaks the real wire protocol over TCP
//! and reproduces behaviour observed on a real OpenRGB 1.1 server
//! (docs/UPSTREAM-FACTS.md): reply-then-ACK ordering, SIGNAL_UPDATE broadcasts,
//! silent no-op resize of unflagged zones, local-client gating, the 159
//! handshake on profile load, status 5 for wrong LED counts.
//!
//! Scenario controls: hotplug (`add_device`/`remove_device`), `crash`/`restart`,
//! `set_latency`, `set_max_protocol`, `inject_raw` (malformed packets),
//! `set_fragmenting`, `set_remote_mode`.

pub mod devices;

use freergb_protocol::diag::{self, LogEntry};
use freergb_protocol::ids::*;
use freergb_protocol::messages::*;
use freergb_protocol::{Color, ControllerData, Packet};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

enum Msg {
    Bytes(Vec<u8>),
    Close,
}

struct Client {
    id: u64,
    tx: mpsc::UnboundedSender<Msg>,
    proto: u32,
    flags: u32,
    local: bool,
    about: Arc<Notify>,
}

impl Client {
    fn send(&self, p: &Packet) {
        let _ = self.tx.send(Msg::Bytes(p.encode()));
    }
    fn wants_profiles(&self) -> bool {
        self.flags & client_flag::SUPPORTS_PROFILEMANAGER != 0
    }
}

#[derive(Clone)]
struct Dev {
    id: u32,
    data: ControllerData,
}

type Snapshot = Vec<(String, String, i32, Vec<freergb_protocol::Mode>, Vec<Color>)>;

struct State {
    devices: Vec<Dev>,
    next_id: u32,
    profiles: BTreeMap<String, Snapshot>,
    active_profile: String,
    settings: BTreeMap<String, serde_json::Value>,
    /// Times a settings write made this server abort, as OpenRGB 1.1 does.
    aborts: u32,
    clients: Vec<Client>,
    max_protocol: u32,
    latency: Duration,
    remote_mode: bool,
    received: Vec<Packet>,
    /// Off for long runs (the soak harness): the log would grow without bound.
    recording: bool,
    accept: Option<JoinHandle<()>>,
    /// Like OpenRGB, the log is kept only while `LogManager.log_console` was on at startup.
    log_on: bool,
    log: Vec<LogEntry>,
    started: std::time::Instant,
}

struct Inner {
    state: Mutex<State>,
    addr: SocketAddr,
    fragmenting: AtomicBool,
    next_client: AtomicU64,
}

#[derive(Clone)]
pub struct MockServer {
    inner: Arc<Inner>,
}

/// Mirror of upstream `SettingsManager::FilterSettingsAgainstSchema` for the mock schema.
/// Returns JSON null when nothing survives, exactly like the C++ (`json filtered;`).
fn filter_settings(key: &str, v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    let Some(obj) = v.as_object() else {
        return Value::Null;
    };
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let Some(props) = schema[key]["properties"].as_object() else {
        return v.clone(); // unregistered key: stored as given
    };
    let mut out = Value::Null;
    for (k, val) in obj {
        let Some(prop) = props.get(k) else { continue };
        let ok = match prop["type"].as_str() {
            Some("bool") => val.is_boolean(),
            Some("integer") => val.is_i64() || val.is_u64(),
            Some("number") => val.is_number(),
            Some("string") | Some("language") => val.is_string(),
            Some("array") => val.is_array(),
            Some("object") | Some("profile") => val.is_object(),
            Some(_) => false,
            None => true,
        };
        if ok {
            out[k] = val.clone();
        }
    }
    out
}

/// What the mock's machine has: one SMBus, a receiver and a cooler on USB, one serial port.
fn fake_hardware(id: u32) -> Vec<u8> {
    use diag::*;
    let r = match id {
        pkt::GET_I2C_BUS_INFO => encode_i2c_buses(&[I2cBus {
            name: "SMBus I801 adapter at efa0".into(),
            port_id: 0,
            pci_device: 0x7a23,
            pci_vendor: 0x8086,
            pci_subsystem_device: 0x7d25,
            pci_subsystem_vendor: 0x1462,
            bus_id: 0,
        }]),
        pkt::GET_HID_DEVICE_INFO => encode_hid_devices(&[
            HidDevice {
                vendor_id: 0x1b1c,
                product_id: 0x0c1a,
                release: 0x0100,
                usage_page: 0xff42,
                usage: 1,
                interface: 0,
                serial: "".into(),
                manufacturer: "Corsair".into(),
                product: "iCUE H150i ELITE".into(),
                path: "/dev/hidraw3".into(),
            },
            HidDevice {
                vendor_id: 0x046d,
                product_id: 0xc52b,
                release: 0x1211,
                usage_page: 0x0001,
                usage: 6,
                interface: 0,
                serial: "".into(),
                manufacturer: "Logitech".into(),
                product: "USB Receiver".into(),
                path: "/dev/hidraw0".into(),
            },
        ]),
        pkt::GET_USB_DEVICE_INFO => encode_usb_devices(&[UsbDevice {
            vendor_id: 0x1b1c,
            product_id: 0x0c1a,
            serial: "".into(),
            manufacturer: "Corsair".into(),
            product: "iCUE H150i ELITE".into(),
        }]),
        pkt::GET_SERIAL_PORTS => encode_serial_ports(&["/dev/ttyUSB0".into()]),
        _ => encode_usb_serial_ports(&[UsbSerialPort {
            vendor_id: 0x1a86,
            product_id: 0x7523,
            port: "/dev/ttyUSB0".into(),
            usb_path: "1-4".into(),
        }]),
    };
    r.unwrap_or_default()
}

const SCHEMA: &str = r#"{"LogManager":{"title":"Log Manager","type":"object","properties":{"log_console":{"type":"bool","title":"Enable Log Console"},"log_file":{"type":"bool","title":"Enable Log File"},"loglevel":{"type":"integer","title":"Log Level"}}},"ProfileManager":{"title":"Profile Manager","type":"object","local_only":false,"properties":{"open_profile":{"type":"profile","title":"Load Profile on Open"},"exit_profile":{"type":"profile","title":"Load Profile on Exit"},"suspend_profile":{"type":"profile","title":"Load Profile on Suspend"},"resume_profile":{"type":"profile","title":"Load Profile on Resume"},"service_startup_profile":{"type":"profile","title":"Load Profile on Service Startup"},"service_shutdown_profile":{"type":"profile","title":"Load Profile on Service Shutdown"}}}}"#;

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn signal(reason: u32, body: &[u8]) -> Vec<u8> {
    let mut p = ((8 + body.len()) as u32).to_le_bytes().to_vec();
    p.extend_from_slice(&reason.to_le_bytes());
    p.extend_from_slice(body);
    p
}

impl State {
    fn client(&self, id: u64) -> Option<&Client> {
        self.clients.iter().find(|c| c.id == id)
    }

    fn dev_mut(&mut self, id: u32) -> Option<&mut Dev> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    fn broadcast(&self, p: &Packet) {
        self.clients.iter().for_each(|c| c.send(p));
    }

    /// Append a log line and push it to local clients, if the log is on.
    fn log(&mut self, level: u32, text: &str) {
        if !self.log_on {
            return;
        }
        let e = LogEntry {
            level,
            line: 1,
            time_ms: self.started.elapsed().as_millis() as u32,
            file: "freergb-mock".into(),
            text: text.into(),
        };
        if let Ok(b) = diag::encode_log_entry(&e) {
            let p = Packet::new(0, pkt::LOG_LOGGED_ENTRY, b);
            self.clients
                .iter()
                .filter(|c| c.local)
                .for_each(|c| c.send(&p));
        }
        self.log.push(e);
    }

    /// Read the log setting as a starting OpenRGB does, and start a fresh log.
    fn boot_log(&mut self) {
        self.log_on = self
            .settings
            .get("LogManager")
            .and_then(|v| v["log_console"].as_bool())
            == Some(true);
        self.log.clear();
        self.started = std::time::Instant::now();
        self.log(3, "[NetworkServer] Network server started");
    }

    fn broadcast_profiles(&self, p: &Packet) {
        self.clients
            .iter()
            .filter(|c| c.wants_profiles())
            .for_each(|c| c.send(p));
    }

    fn signal_block(&self, dev: u32, reason: u32) {
        if let Some(d) = self.devices.iter().find(|d| d.id == dev) {
            if let Ok(block) = d.data.encode_block(6) {
                self.broadcast(&Packet::new(
                    dev,
                    pkt::SIGNAL_UPDATE,
                    signal(reason, &block),
                ));
            }
        }
    }

    fn signal_colors(&self, dev: u32) {
        if let Some(d) = self.devices.iter().find(|d| d.id == dev) {
            if let Ok(c) = freergb_protocol::device::encode_color_list(&d.data.colors) {
                self.broadcast(&Packet::new(
                    dev,
                    pkt::SIGNAL_UPDATE,
                    signal(update_reason::UPDATE_LEDS, &c),
                ));
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        self.devices
            .iter()
            .map(|d| {
                (
                    d.data.name.clone(),
                    d.data.serial.clone(),
                    d.data.active_mode,
                    d.data.modes.clone(),
                    d.data.colors.clone(),
                )
            })
            .collect()
    }
}

impl MockServer {
    /// Start on an ephemeral loopback port.
    pub async fn start(devices: Vec<ControllerData>) -> std::io::Result<Self> {
        Self::start_on(devices, 0).await
    }

    pub async fn start_on(devices: Vec<ControllerData>, port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        let addr = listener.local_addr()?;
        let mut next_id = 0;
        let devs = devices
            .into_iter()
            .map(|data| {
                let id = next_id;
                next_id += 1;
                Dev { id, data }
            })
            .collect();
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                devices: devs,
                next_id,
                profiles: BTreeMap::new(),
                active_profile: String::new(),
                settings: BTreeMap::new(),
                aborts: 0,
                clients: Vec::new(),
                max_protocol: MAX_PROTOCOL_VERSION,
                latency: Duration::ZERO,
                remote_mode: false,
                received: Vec::new(),
                recording: true,
                accept: None,
                log_on: false,
                log: Vec::new(),
                started: std::time::Instant::now(),
            }),
            addr,
            fragmenting: AtomicBool::new(false),
            next_client: AtomicU64::new(1),
        });
        let server = MockServer { inner };
        server.spawn_accept(listener);
        Ok(server)
    }

    fn spawn_accept(&self, listener: TcpListener) {
        let me = self.clone();
        let h = tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                // Without this, Nagle + the client's delayed ACK add ~40 ms per reply.
                let _ = sock.set_nodelay(true);
                let me = me.clone();
                tokio::spawn(async move { me.serve(sock).await });
            }
        });
        lock(&self.inner.state).accept = Some(h);
    }

    pub fn addr(&self) -> SocketAddr {
        self.inner.addr
    }

    // ---- scenario controls ------------------------------------------------

    /// Hotplug: add a device and broadcast DEVICE_LIST_UPDATED. Returns its stable ID.
    pub fn add_device(&self, data: ControllerData) -> u32 {
        let mut s = lock(&self.inner.state);
        let id = s.next_id;
        s.next_id += 1;
        s.devices.push(Dev { id, data });
        s.broadcast(&Packet::new(0, pkt::DEVICE_LIST_UPDATED, vec![]));
        id
    }

    /// Hotplug: remove a device and broadcast DEVICE_LIST_UPDATED. IDs are never reused.
    pub fn remove_device(&self, id: u32) -> bool {
        let mut s = lock(&self.inner.state);
        let before = s.devices.len();
        s.devices.retain(|d| d.id != id);
        let removed = s.devices.len() != before;
        if removed {
            s.broadcast(&Packet::new(0, pkt::DEVICE_LIST_UPDATED, vec![]));
        }
        removed
    }

    pub fn device_ids(&self) -> Vec<u32> {
        lock(&self.inner.state)
            .devices
            .iter()
            .map(|d| d.id)
            .collect()
    }

    pub fn device(&self, id: u32) -> Option<ControllerData> {
        lock(&self.inner.state)
            .devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.data.clone())
    }

    /// Simulate a server crash: stop listening and drop every connection.
    /// Device and profile state survives (as it would on disk) until `restart`.
    pub fn crash(&self) {
        let mut s = lock(&self.inner.state);
        if let Some(h) = s.accept.take() {
            h.abort();
        }
        for c in s.clients.drain(..) {
            let _ = c.tx.send(Msg::Close);
        }
    }

    /// Rebind the same port after `crash`.
    pub async fn restart(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.inner.addr).await?;
        lock(&self.inner.state).boot_log();
        self.spawn_accept(listener);
        Ok(())
    }

    /// Add a line to the server log (kept only while the log is on, see `restart`).
    pub fn log(&self, level: u32, text: &str) {
        lock(&self.inner.state).log(level, text);
    }

    /// Start with `LogManager.log_console` on, as if it was saved before this server started.
    pub fn enable_log(&self) {
        let mut s = lock(&self.inner.state);
        s.settings.insert(
            "LogManager".into(),
            serde_json::json!({ "log_console": true }),
        );
        s.boot_log();
    }

    /// The server's settings for `key`, as saved by clients (null when unset).
    pub fn settings(&self, key: &str) -> serde_json::Value {
        lock(&self.inner.state)
            .settings
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    /// How many times a settings write made this server abort (see `SETTINGS_MODIFY`
    /// handling). A correct client keeps this at 0.
    pub fn settings_aborts(&self) -> u32 {
        lock(&self.inner.state).aborts
    }

    pub fn set_latency(&self, d: Duration) {
        lock(&self.inner.state).latency = d;
    }

    /// Cap the protocol version the server will negotiate (version-mismatch scenario).
    pub fn set_max_protocol(&self, v: u32) {
        lock(&self.inner.state).max_protocol = v;
    }

    /// Do not grant local-client status (as for a remote, non-loopback client).
    pub fn set_remote_mode(&self, remote: bool) {
        lock(&self.inner.state).remote_mode = remote;
    }

    /// Write every message in tiny chunks to exercise the client's stream parser.
    pub fn set_fragmenting(&self, on: bool) {
        self.inner.fragmenting.store(on, Ordering::Relaxed);
    }

    /// Send raw bytes to every client (malformed-packet scenarios).
    pub fn inject_raw(&self, bytes: &[u8]) {
        for c in &lock(&self.inner.state).clients {
            let _ = c.tx.send(Msg::Bytes(bytes.to_vec()));
        }
    }

    /// Stop (or resume) keeping every received packet for `received`.
    pub fn set_recording(&self, on: bool) {
        lock(&self.inner.state).recording = on;
    }

    pub fn received(&self) -> Vec<Packet> {
        lock(&self.inner.state).received.clone()
    }

    pub fn profile_names(&self) -> Vec<String> {
        lock(&self.inner.state).profiles.keys().cloned().collect()
    }

    pub fn client_count(&self) -> usize {
        lock(&self.inner.state).clients.len()
    }

    // ---- connection handling ----------------------------------------------

    async fn serve(self, sock: TcpStream) {
        let (mut rd, mut wr) = sock.into_split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
        let id = self.inner.next_client.fetch_add(1, Ordering::Relaxed);
        let about = Arc::new(Notify::new());
        lock(&self.inner.state).clients.push(Client {
            id,
            tx,
            proto: 0,
            flags: 0,
            local: false,
            about: about.clone(),
        });
        let me = self.clone();
        let writer = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Msg::Close => break,
                    Msg::Bytes(b) => {
                        if me.inner.fragmenting.load(Ordering::Relaxed) {
                            for chunk in b.chunks(3) {
                                if wr.write_all(chunk).await.is_err() {
                                    return;
                                }
                                let _ = wr.flush().await;
                                tokio::task::yield_now().await;
                            }
                        } else if wr.write_all(&b).await.is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = wr.shutdown().await;
        });
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        'outer: loop {
            match rd.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
            loop {
                match Packet::decode(&buf) {
                    Ok(Some((p, used))) => {
                        buf.drain(..used);
                        let lat = lock(&self.inner.state).latency;
                        if !lat.is_zero() {
                            tokio::time::sleep(lat).await;
                        }
                        self.handle(id, p);
                    }
                    Ok(None) => break,
                    Err(_) => break 'outer, // bad magic / oversize: drop the connection like a real server would
                }
            }
        }
        lock(&self.inner.state).clients.retain(|c| c.id != id);
        writer.abort();
    }

    fn handle(&self, cid: u64, p: Packet) {
        let mut s = lock(&self.inner.state);
        if s.recording {
            s.received.push(p.clone());
        }
        let Some(c) = s.client(cid) else { return };
        let proto = c.proto;
        let local = c.local;
        let ack = |s: &State, status: u32| {
            if let Some(c) = s.client(cid) {
                if c.proto >= 6 {
                    c.send(&Packet::new(
                        p.dev_id,
                        pkt::ACK,
                        encode_ack(p.pkt_id, status),
                    ));
                }
            }
        };
        let reply = |s: &State, id: u32, payload: Vec<u8>| {
            if let Some(c) = s.client(cid) {
                c.send(&Packet::new(p.dev_id, id, payload));
            }
        };
        let gated = matches!(
            p.pkt_id,
            pkt::PROFILE_SAVE
                | pkt::PROFILE_DELETE
                | pkt::PROFILE_UPLOAD
                | pkt::SETTINGS_MODIFY
                | pkt::SETTINGS_SET
                | pkt::SETTINGS_SAVE
                | pkt::LOG_CLEAR_BUFFER
                | pkt::LOG_GET_BUFFER
                | pkt::LOG_GET_LEVEL
                | pkt::LOG_SET_LEVEL
                | pkt::GET_I2C_BUS_INFO
                | pkt::GET_HID_DEVICE_INFO
                | pkt::GET_USB_DEVICE_INFO
                | pkt::GET_SERIAL_PORTS
                | pkt::GET_USB_SERIAL_PORTS
        );
        if gated && !local {
            return ack(&s, status::ERROR_NOT_ALLOWED);
        }
        let empty_list = vec![6, 0, 0, 0, 0, 0];
        match p.pkt_id {
            pkt::REQUEST_PROTOCOL_VERSION => {
                let Ok(cv) = decode_protocol_version(&p.payload) else {
                    return ack(&s, status::ERROR_INVALID_DATA);
                };
                let negotiated = cv.min(s.max_protocol);
                let maxp = s.max_protocol;
                if let Some(c) = s.clients.iter_mut().find(|c| c.id == cid) {
                    c.proto = negotiated;
                }
                reply(
                    &s,
                    pkt::REQUEST_PROTOCOL_VERSION,
                    encode_protocol_version(maxp),
                );
                if negotiated >= 6 {
                    reply(&s, pkt::SET_SERVER_NAME, encode_name("OpenRGB 1.1 (mock)"));
                    ack(&s, status::OK);
                }
            }
            pkt::SET_CLIENT_NAME => ack(&s, status::OK),
            pkt::SET_CLIENT_FLAGS => {
                let Ok(f) = decode_flags(&p.payload) else {
                    return ack(&s, status::ERROR_INVALID_DATA);
                };
                let granted = f & client_flag::REQUEST_LOCAL_CLIENT != 0 && !s.remote_mode;
                if let Some(c) = s.clients.iter_mut().find(|c| c.id == cid) {
                    c.flags = f;
                    c.local = granted;
                }
                if proto >= 6 {
                    let mut sf = server_flag::SUPPORTS_RGBCONTROLLER
                        | server_flag::SUPPORTS_LOGMANAGER
                        | server_flag::SUPPORTS_PROFILEMANAGER
                        | server_flag::SUPPORTS_PLUGINMANAGER
                        | server_flag::SUPPORTS_SETTINGSMANAGER
                        | server_flag::SUPPORTS_DETECTION
                        | server_flag::SUPPORTS_DEVICE_INFO;
                    if granted {
                        sf |= server_flag::LOCAL_CLIENT;
                    }
                    reply(&s, pkt::SET_SERVER_FLAGS, encode_flags(sf));
                    ack(&s, status::OK);
                }
            }
            pkt::REQUEST_CONTROLLER_COUNT => {
                let ids: Vec<u32> = s.devices.iter().map(|d| d.id).collect();
                reply(
                    &s,
                    pkt::REQUEST_CONTROLLER_COUNT,
                    encode_controller_list(&ids, proto),
                );
                ack(&s, status::OK);
            }
            pkt::REQUEST_CONTROLLER_DATA => {
                match s
                    .devices
                    .iter()
                    .find(|d| d.id == p.dev_id)
                    .and_then(|d| d.data.encode_packet(proto.max(1)).ok())
                {
                    Some(bytes) => {
                        reply(&s, pkt::REQUEST_CONTROLLER_DATA, bytes);
                        ack(&s, status::OK);
                    }
                    None => ack(&s, status::ERROR_INVALID_ID),
                }
            }
            pkt::REQUEST_RESCAN_DEVICES => {
                ack(&s, status::OK);
                s.log(3, "[ResourceManager] Rescanning devices");
                s.broadcast(&Packet::new(0, pkt::DETECTION_STARTED, vec![]));
                for (percent, label) in [(0, "Detecting I2C interfaces"), (50, "Mock devices")] {
                    if let Ok(body) = encode_detection_progress(percent, label) {
                        s.broadcast(&Packet::new(0, pkt::DETECTION_PROGRESS_CHANGED, body));
                    }
                }
                s.broadcast(&Packet::new(0, pkt::DETECTION_COMPLETE, vec![]));
            }
            pkt::UPDATE_LEDS => {
                let st = match decode_update_leds(&p.payload) {
                    Ok(cols) => match s.dev_mut(p.dev_id) {
                        // Real rule: fewer colors than LEDs is a partial update from LED 0;
                        // more colors than LEDs is invalid data.
                        Some(d) if cols.len() <= d.data.colors.len() => {
                            d.data.colors[..cols.len()].copy_from_slice(&cols);
                            status::OK
                        }
                        Some(_) => status::ERROR_INVALID_DATA,
                        None => status::ERROR_INVALID_ID,
                    },
                    Err(_) => status::ERROR_INVALID_DATA,
                };
                ack(&s, st);
                if st == status::OK {
                    s.signal_colors(p.dev_id);
                }
            }
            pkt::UPDATE_ZONE_LEDS => {
                let st = match decode_update_zone_leds(&p.payload) {
                    Ok((zi, cols)) => match s.dev_mut(p.dev_id) {
                        Some(d) => {
                            let zi = zi as usize;
                            let start: usize = d
                                .data
                                .zones
                                .iter()
                                .take(zi)
                                .map(|z| z.leds_count as usize)
                                .sum();
                            match d.data.zones.get(zi) {
                                Some(z) if cols.len() <= z.leds_count as usize => {
                                    d.data.colors[start..start + cols.len()].copy_from_slice(&cols);
                                    status::OK
                                }
                                _ => status::ERROR_INVALID_DATA,
                            }
                        }
                        None => status::ERROR_INVALID_ID,
                    },
                    Err(_) => status::ERROR_INVALID_DATA,
                };
                ack(&s, st);
                if st == status::OK {
                    s.signal_colors(p.dev_id);
                }
            }
            pkt::UPDATE_SINGLE_LED => {
                let st = match decode_update_single_led(&p.payload) {
                    Ok((i, col)) => match s.dev_mut(p.dev_id) {
                        Some(d) if i >= 0 && (i as usize) < d.data.colors.len() => {
                            d.data.colors[i as usize] = col;
                            status::OK
                        }
                        Some(_) => status::ERROR_INVALID_DATA,
                        None => status::ERROR_INVALID_ID,
                    },
                    Err(_) => status::ERROR_INVALID_DATA,
                };
                ack(&s, st);
                if st == status::OK {
                    s.signal_colors(p.dev_id);
                }
            }
            pkt::SET_CUSTOM_MODE => {
                let st = match s.dev_mut(p.dev_id) {
                    Some(d) => match d
                        .data
                        .modes
                        .iter()
                        .position(|m| m.flags & mode_flag::HAS_PER_LED_COLOR != 0)
                    {
                        Some(i) => {
                            d.data.active_mode = i as i32;
                            status::OK
                        }
                        None => status::ERROR_UNSUPPORTED,
                    },
                    None => status::ERROR_INVALID_ID,
                };
                ack(&s, st);
                if st == status::OK {
                    s.signal_block(p.dev_id, update_reason::UPDATE_MODE);
                }
            }
            pkt::UPDATE_MODE | pkt::SAVE_MODE => {
                let st = match decode_update_mode(&p.payload, proto.max(3)) {
                    Ok((i, mode)) => match s.dev_mut(p.dev_id) {
                        Some(d) if i >= 0 && (i as usize) < d.data.modes.len() => {
                            d.data.modes[i as usize] = mode;
                            d.data.active_mode = i;
                            status::OK
                        }
                        Some(_) => status::ERROR_INVALID_DATA,
                        None => status::ERROR_INVALID_ID,
                    },
                    Err(_) => status::ERROR_INVALID_DATA,
                };
                ack(&s, st);
                if st == status::OK {
                    let reason = if p.pkt_id == pkt::SAVE_MODE {
                        update_reason::SAVE_MODE
                    } else {
                        update_reason::UPDATE_MODE
                    };
                    s.signal_block(p.dev_id, reason);
                }
            }
            pkt::UPDATE_ZONE_MODE => {
                let st = match decode_update_zone_mode(&p.payload, proto.max(6)) {
                    Ok((zi, mi, mode)) => match s.dev_mut(p.dev_id) {
                        Some(d) => match (d.data.zones.get_mut(zi.max(0) as usize), mode) {
                            (Some(z), Some(mode)) if (mi as usize) < z.modes.len() => {
                                z.modes[mi as usize] = mode;
                                z.active_mode = mi;
                                status::OK
                            }
                            (Some(z), None) if mi == -1 => {
                                z.active_mode = -1;
                                status::OK
                            }
                            _ => status::ERROR_INVALID_DATA,
                        },
                        None => status::ERROR_INVALID_ID,
                    },
                    Err(_) => status::ERROR_INVALID_DATA,
                };
                ack(&s, st);
                if st == status::OK {
                    s.signal_block(p.dev_id, update_reason::UPDATE_MODE);
                }
            }
            pkt::RESIZE_ZONE => {
                // Returns (status, whether the zone actually changed).
                let (st, changed) = match decode_resize_zone(&p.payload) {
                    Ok((zi, size)) => match s.dev_mut(p.dev_id) {
                        Some(d) => {
                            // Real behaviour: unflagged zones silently ignore the resize but still ACK OK.
                            let mut changed = false;
                            if let Some(z) = d.data.zones.get_mut(zi.max(0) as usize) {
                                let resizable = z.flags
                                    & (zone_flag::CONFIGURABLE_SIZE | zone_flag::SIZE_EFFECTS_ONLY)
                                    != 0;
                                if resizable
                                    && size >= z.leds_min as i32
                                    && size <= z.leds_max as i32
                                {
                                    z.leds_count = size as u32;
                                    changed = true;
                                }
                            }
                            if changed {
                                devices::rebuild_leds(&mut d.data);
                            }
                            (status::OK, changed)
                        }
                        None => (status::ERROR_INVALID_ID, false),
                    },
                    Err(_) => (status::ERROR_INVALID_DATA, false),
                };
                ack(&s, st);
                if changed {
                    s.signal_block(p.dev_id, update_reason::CONFIGURE_ZONE);
                }
            }
            // Like the 1.1 server: any zone takes segments, nothing is checked against its size.
            pkt::CLEAR_SEGMENTS | pkt::ADD_SEGMENT => {
                let st = match s.dev_mut(p.dev_id) {
                    None => status::ERROR_INVALID_ID,
                    Some(d) => {
                        let parsed = if p.pkt_id == pkt::CLEAR_SEGMENTS {
                            decode_clear_segments(&p.payload).map(|z| (z, None))
                        } else {
                            decode_add_segment(&p.payload, proto.max(6)).map(|(z, s)| (z, Some(s)))
                        };
                        match parsed {
                            Ok((zi, seg)) => match d.data.zones.get_mut(zi.max(0) as usize) {
                                Some(z) => {
                                    match seg {
                                        Some(seg) => z.segments.push(seg),
                                        None => z.segments.clear(),
                                    }
                                    status::OK
                                }
                                None => status::ERROR_INVALID_DATA,
                            },
                            Err(_) => status::ERROR_INVALID_DATA,
                        }
                    }
                };
                ack(&s, st);
                if st == status::OK {
                    let why = if p.pkt_id == pkt::CLEAR_SEGMENTS {
                        update_reason::CLEAR_SEGMENTS
                    } else {
                        update_reason::ADD_SEGMENT
                    };
                    s.signal_block(p.dev_id, why);
                }
            }
            pkt::SET_HIDDEN => {
                let st = match (decode_set_hidden(&p.payload), s.dev_mut(p.dev_id)) {
                    (Ok(h), Some(d)) => {
                        if h {
                            d.data.flags |= controller_flag::HIDDEN;
                        } else {
                            d.data.flags &= !controller_flag::HIDDEN;
                        }
                        status::OK
                    }
                    (_, None) => status::ERROR_INVALID_ID,
                    _ => status::ERROR_INVALID_DATA,
                };
                ack(&s, st);
                if st == status::OK {
                    s.signal_block(p.dev_id, update_reason::HIDDEN);
                }
            }
            pkt::PROFILE_GET_LIST => {
                let names: Vec<String> = s.profiles.keys().cloned().collect();
                if let Ok(b) = encode_profile_list(&names) {
                    reply(&s, pkt::PROFILE_GET_LIST, b);
                }
                ack(&s, status::OK);
            }
            pkt::PROFILE_SAVE => match decode_cstring(&p.payload) {
                Ok(name) if !name.is_empty() => {
                    let snap = s.snapshot();
                    s.profiles.insert(name, snap);
                    ack(&s, status::OK);
                    let names: Vec<String> = s.profiles.keys().cloned().collect();
                    if let Ok(b) = encode_profile_list(&names) {
                        s.broadcast_profiles(&Packet::new(0, pkt::PROFILE_LIST_UPDATED, b));
                    }
                }
                _ => ack(&s, status::ERROR_INVALID_DATA),
            },
            pkt::PROFILE_DELETE => match decode_cstring(&p.payload) {
                Ok(name) => {
                    s.profiles.remove(&name);
                    ack(&s, status::OK);
                    let names: Vec<String> = s.profiles.keys().cloned().collect();
                    if let Ok(b) = encode_profile_list(&names) {
                        s.broadcast_profiles(&Packet::new(0, pkt::PROFILE_LIST_UPDATED, b));
                    }
                }
                Err(_) => ack(&s, status::ERROR_INVALID_DATA),
            },
            pkt::PROFILE_LOAD => match decode_cstring(&p.payload) {
                Ok(name) if s.profiles.contains_key(&name) => {
                    // Real server asks every ProfileManager client to answer 159 first and
                    // waits (about 1 s) for the echo, so load runs off the read loop.
                    let waiters: Vec<Arc<Notify>> = s
                        .clients
                        .iter()
                        .filter(|c| c.wants_profiles())
                        .map(|c| c.about.clone())
                        .collect();
                    s.broadcast_profiles(&Packet::new(0, pkt::PROFILE_ABOUT_TO_LOAD, vec![]));
                    let me = self.clone();
                    let dev_id = p.dev_id;
                    drop(s);
                    tokio::spawn(async move {
                        for w in waiters {
                            let _ =
                                tokio::time::timeout(Duration::from_secs(1), w.notified()).await;
                        }
                        me.finish_load(cid, dev_id, &name);
                    });
                }
                Ok(_) => ack(&s, status::ERROR_INVALID_DATA),
                Err(_) => ack(&s, status::ERROR_INVALID_DATA),
            },
            pkt::PROFILE_ABOUT_TO_LOAD => {
                if let Some(c) = s.client(cid) {
                    c.about.notify_one();
                }
            }
            pkt::PROFILE_DOWNLOAD => match decode_cstring(&p.payload) {
                Ok(name) if s.profiles.contains_key(&name) => {
                    reply(
                        &s,
                        pkt::PROFILE_DOWNLOAD,
                        encode_cstring(&format!("{{\"profile_name\":\"{name}\"}}")),
                    );
                    ack(&s, status::OK);
                }
                _ => ack(&s, status::ERROR_INVALID_DATA),
            },
            pkt::PROFILE_UPLOAD => ack(&s, status::OK),
            pkt::PROFILE_GET_ACTIVE => {
                reply(
                    &s,
                    pkt::PROFILE_GET_ACTIVE,
                    encode_cstring(&s.active_profile.clone()),
                );
                ack(&s, status::OK);
            }
            pkt::PROFILE_CLEAR_ACTIVE => {
                s.active_profile.clear();
                ack(&s, status::OK);
                s.broadcast_profiles(&Packet::new(
                    0,
                    pkt::PROFILE_ACTIVE_CHANGED,
                    encode_cstring(""),
                ));
            }
            pkt::SETTINGS_GET => {
                let key = decode_cstring(&p.payload).unwrap_or_default();
                let v = s
                    .settings
                    .get(&key)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "null".into());
                reply(&s, pkt::SETTINGS_GET, encode_cstring(&v));
                ack(&s, status::OK);
            }
            pkt::SETTINGS_GET_SCHEMA => {
                let key = decode_cstring(&p.payload).unwrap_or_default();
                let v = if key.is_empty() {
                    SCHEMA.to_string()
                } else {
                    "null".into()
                };
                reply(&s, pkt::SETTINGS_GET_SCHEMA, encode_cstring(&v));
                ack(&s, status::OK);
            }
            pkt::SETTINGS_SAVE => ack(&s, status::OK),
            pkt::SETTINGS_MODIFY | pkt::SETTINGS_SET => {
                let body = decode_cstring(&p.payload).unwrap_or_default();
                let parsed: serde_json::Value =
                    serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
                let modify = p.pkt_id == pkt::SETTINGS_MODIFY;
                let mut abort = false;
                if let Some(obj) = parsed.as_object() {
                    for (key, val) in obj {
                        let filtered = filter_settings(key, val);
                        if modify {
                            // upstream: settings_data[key].update(filtered) -- throws
                            // (uncaught, process aborts) when `filtered` is null.
                            let Some(f) = filtered.as_object() else {
                                abort = true;
                                break;
                            };
                            let e = s
                                .settings
                                .entry(key.clone())
                                .or_insert_with(|| serde_json::json!({}));
                            if !e.is_object() {
                                *e = serde_json::json!({});
                            }
                            for (k, v) in f {
                                e[k] = v.clone();
                            }
                        } else {
                            s.settings.insert(key.clone(), filtered);
                        }
                    }
                }
                if abort {
                    s.aborts += 1;
                    if let Some(h) = s.accept.take() {
                        h.abort();
                    }
                    for c in s.clients.drain(..) {
                        let _ = c.tx.send(Msg::Close);
                    }
                } else {
                    ack(&s, status::OK);
                }
            }
            pkt::LOG_GET_LEVEL => {
                reply(&s, pkt::LOG_GET_LEVEL, 5u32.to_le_bytes().to_vec());
                ack(&s, status::OK);
            }
            pkt::LOG_SET_LEVEL => ack(&s, status::OK),
            pkt::LOG_CLEAR_BUFFER => {
                s.log.clear();
                ack(&s, status::OK);
            }
            pkt::LOG_GET_BUFFER => {
                for e in &s.log {
                    if let Ok(b) = diag::encode_log_entry(e) {
                        reply(&s, pkt::LOG_LOGGED_ENTRY, b);
                    }
                }
                ack(&s, status::OK);
            }
            pkt::GET_I2C_BUS_INFO
            | pkt::GET_HID_DEVICE_INFO
            | pkt::GET_USB_DEVICE_INFO
            | pkt::GET_SERIAL_PORTS
            | pkt::GET_USB_SERIAL_PORTS => {
                reply(&s, p.pkt_id, fake_hardware(p.pkt_id));
                ack(&s, status::OK);
            }
            pkt::PLUGIN_GET_LIST => {
                reply(&s, pkt::PLUGIN_GET_LIST, empty_list);
                ack(&s, status::OK);
            }
            _ => ack(&s, status::ERROR_UNSUPPORTED),
        }
    }

    fn finish_load(&self, cid: u64, dev_id: u32, name: &str) {
        let mut s = lock(&self.inner.state);
        let Some(snap) = s.profiles.get(name).cloned() else {
            return;
        };
        let mut changed = Vec::new();
        for (n, serial, active, modes, colors) in snap {
            if let Some(d) = s
                .devices
                .iter_mut()
                .find(|d| d.data.name == n && d.data.serial == serial)
            {
                d.data.active_mode = active;
                d.data.modes = modes;
                if colors.len() == d.data.colors.len() {
                    d.data.colors = colors;
                }
                changed.push(d.id);
            }
        }
        s.active_profile = name.to_string();
        let json = encode_cstring(&format!("{{\"profile_name\":\"{name}\"}}"));
        for c in s.clients.iter().filter(|c| c.wants_profiles() && c.local) {
            c.send(&Packet::new(0, pkt::PROFILE_LOADED, json.clone()));
        }
        s.broadcast_profiles(&Packet::new(
            0,
            pkt::PROFILE_ACTIVE_CHANGED,
            encode_cstring(name),
        ));
        if let Some(c) = s.client(cid) {
            if c.proto >= 6 {
                c.send(&Packet::new(
                    dev_id,
                    pkt::ACK,
                    encode_ack(pkt::PROFILE_LOAD, status::OK),
                ));
            }
        }
        for id in changed {
            s.signal_block(id, update_reason::UPDATE_MODE);
        }
    }
}
