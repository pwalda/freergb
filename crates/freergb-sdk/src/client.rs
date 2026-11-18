// SPDX-License-Identifier: GPL-2.0-or-later

use crate::backend::RgbBackend;
use crate::error::{Result, SdkError};
use freergb_protocol::device::decode_color_list;
use freergb_protocol::diag;
use freergb_protocol::ids::*;
use freergb_protocol::messages::*;
use freergb_protocol::wire::Reader;
use freergb_protocol::{Color, ControllerData, Mode, Packet, Segment};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub client_name: String,
    pub request_timeout: Duration,
    /// Limit for the TCP connect alone, kept apart from `request_timeout` because a closed
    /// loopback port can take ~2 s to refuse on Windows, which must not look like a hang.
    pub connect_timeout: Duration,
    /// Max LED updates sent without an ACK yet (flow control for the effects path).
    pub max_unacked: usize,
    /// After loading a profile, re-send each device's active mode. Default **off**: the SDK doc
    /// says clients must, but upstream's server-side load already calls UpdateMode/UpdateLEDs/
    /// UpdateZoneMode itself (`ProfileManager.cpp` 1475-1558), so this would double hardware writes.
    pub apply_loaded_profile: bool,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            client_name: "freergb".into(),
            request_timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(5),
            max_unacked: 64,
            apply_loaded_profile: false,
        }
    }
}

/// Server-pushed happenings. Device state is already updated in the store
/// when the corresponding event is delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    DeviceListChanged {
        added: Vec<u32>,
        removed: Vec<u32>,
    },
    /// A device changed (`reason` is an `update_reason` code; colors-only changes use UPDATE_LEDS).
    DeviceUpdated {
        id: u32,
        reason: u32,
    },
    ProfileListUpdated(Vec<String>),
    ActiveProfileChanged(String),
    ProfileLoaded,
    DetectionStarted,
    /// `percent` is 0..100; `label` names the detector running (may be empty).
    DetectionProgress {
        percent: u32,
        label: String,
    },
    DetectionComplete,
    /// New server log lines are in [`Client::log_since`] (local clients only).
    LogUpdated,
    Disconnected,
}

struct PendingAck {
    dev: u32,
    pkt: u32,
    tx: Option<oneshot::Sender<u32>>,
    _permit: Option<OwnedSemaphorePermit>,
}

struct PendingReply {
    dev: u32,
    pkt: u32,
    tx: oneshot::Sender<Packet>,
}

#[derive(Default)]
struct Store {
    devices: BTreeMap<u32, ControllerData>,
    acks: Vec<PendingAck>,
    replies: Vec<PendingReply>,
    closed: bool,
    log: Log,
}

/// Most server log lines kept; older ones are dropped.
pub const LOG_CAP: usize = 5000;

/// The server's log as this connection received it, each line numbered from 1.
#[derive(Default)]
struct Log {
    lines: std::collections::VecDeque<(u64, diag::LogEntry)>,
    last: u64,
}

impl Log {
    /// Keep a line, unless it is one just kept: a line logged while the server sends its
    /// buffer can arrive both live and in the buffer.
    fn push(&mut self, e: diag::LogEntry) -> bool {
        let recent = self.lines.iter().rev().take(64);
        if recent
            .take_while(|(_, l)| l.time_ms >= e.time_ms)
            .any(|(_, l)| *l == e)
        {
            return false;
        }
        self.last += 1;
        self.lines.push_back((self.last, e));
        if self.lines.len() > LOG_CAP {
            self.lines.pop_front();
        }
        true
    }
}

struct Shared {
    store: Mutex<Store>,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    events: broadcast::Sender<Event>,
    flow: Arc<Semaphore>,
    resync_lock: tokio::sync::Mutex<()>,
    opts: ConnectOptions,
    local: bool,
    server_name: String,
}

fn lock(m: &Mutex<Store>) -> MutexGuard<'_, Store> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
}

async fn read_packet(sock: &mut TcpStream, buf: &mut Vec<u8>, timeout: Duration) -> Result<Packet> {
    let fut = async {
        loop {
            if let Some((p, used)) = Packet::decode(buf)? {
                buf.drain(..used);
                return Ok(p);
            }
            let mut chunk = [0u8; 8192];
            let n = sock.read(&mut chunk).await?;
            if n == 0 {
                return Err(SdkError::Disconnected);
            }
            buf.extend_from_slice(&chunk[..n]);
        }
    };
    tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| SdkError::Timeout("handshake"))?
}

impl Client {
    pub async fn connect(addr: SocketAddr, opts: ConnectOptions) -> Result<Client> {
        let mut sock = tokio::time::timeout(opts.connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| SdkError::Timeout("connect"))??;
        sock.set_nodelay(true)?;
        let mut buf = Vec::new();
        let t = opts.request_timeout;

        // 1. Version. A v6 server answers: version, server name, ACK. Older servers: version only.
        sock.write_all(
            &Packet::new(
                0,
                pkt::REQUEST_PROTOCOL_VERSION,
                encode_protocol_version(MAX_PROTOCOL_VERSION),
            )
            .encode(),
        )
        .await?;
        let reply = match read_packet(&mut sock, &mut buf, t).await {
            Ok(p) => p,
            // Silence (OpenRGB < 0.5 never answers a version request, and neither does
            // most other software) or a reply that is not an "ORGB" frame.
            Err(SdkError::Timeout(_) | SdkError::Protocol(_)) => return Err(SdkError::NotOpenRgb),
            Err(e) => return Err(e),
        };
        if reply.pkt_id != pkt::REQUEST_PROTOCOL_VERSION {
            return Err(SdkError::BadReply("expected protocol version reply"));
        }
        let server_version = decode_protocol_version(&reply.payload)?;
        if server_version < 6 {
            return Err(SdkError::UnsupportedProtocol {
                server: server_version,
            });
        }
        let mut server_name = String::new();
        loop {
            let p = read_packet(&mut sock, &mut buf, t).await?;
            match p.pkt_id {
                pkt::SET_SERVER_NAME => server_name = decode_name(&p.payload)?,
                pkt::ACK => break,
                _ => {}
            }
        }

        // 2. Name (ACKed), 3. flags (server flags, then ACK).
        sock.write_all(
            &Packet::new(0, pkt::SET_CLIENT_NAME, encode_name(&opts.client_name)).encode(),
        )
        .await?;
        wait_ack(&mut sock, &mut buf, t).await?;
        let flags = client_flag::SUPPORTS_RGBCONTROLLER
            | client_flag::SUPPORTS_PROFILEMANAGER
            | client_flag::SUPPORTS_SETTINGSMANAGER
            | client_flag::SUPPORTS_LOGMANAGER
            | client_flag::REQUEST_LOCAL_CLIENT;
        sock.write_all(&Packet::new(0, pkt::SET_CLIENT_FLAGS, encode_flags(flags)).encode())
            .await?;
        let mut server_flags = 0;
        loop {
            let p = read_packet(&mut sock, &mut buf, t).await?;
            match p.pkt_id {
                pkt::SET_SERVER_FLAGS => server_flags = decode_flags(&p.payload)?,
                pkt::ACK => break,
                _ => {}
            }
        }

        let (rd, mut wr) = sock.into_split();
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (events, _) = broadcast::channel(256);
        let shared = Arc::new(Shared {
            store: Mutex::new(Store::default()),
            write_tx,
            events,
            flow: Arc::new(Semaphore::new(opts.max_unacked.max(1))),
            resync_lock: tokio::sync::Mutex::new(()),
            local: server_flags & server_flag::LOCAL_CLIENT != 0,
            server_name,
            opts,
        });

        tokio::spawn(async move {
            while let Some(b) = write_rx.recv().await {
                if wr.write_all(&b).await.is_err() {
                    break;
                }
            }
        });
        let reader_shared = shared.clone();
        tokio::spawn(async move { reader_loop(reader_shared, rd, buf).await });

        let client = Client { shared };
        client.resync().await?;
        Ok(client)
    }

    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.shared.events.subscribe()
    }

    pub fn is_local_client(&self) -> bool {
        self.shared.local
    }

    pub fn server_name(&self) -> &str {
        &self.shared.server_name
    }

    pub fn is_connected(&self) -> bool {
        !lock(&self.shared.store).closed
    }

    /// Restart-proof identity of every current device (see [`DeviceKey`]).
    pub fn device_keys(&self) -> Vec<(u32, crate::DeviceKey)> {
        crate::identity::keys_for(&lock(&self.shared.store).devices)
    }

    /// Current controller ID for a persisted identity, if that device is present.
    pub fn find_device(&self, key: &crate::DeviceKey) -> Option<u32> {
        self.device_keys()
            .into_iter()
            .find(|(_, k)| k == key)
            .map(|(id, _)| id)
    }

    /// Match persisted identities to current devices, tolerating a changed `location`
    /// (see [`crate::identity::resolve`]). Aligned with `saved`.
    pub fn resolve_devices(&self, saved: &[crate::DeviceKey]) -> Vec<Option<u32>> {
        crate::identity::resolve(saved, &self.device_keys())
    }

    pub fn device(&self, id: u32) -> Option<ControllerData> {
        lock(&self.shared.store).devices.get(&id).cloned()
    }

    // ---- request plumbing --------------------------------------------------

    /// Send a request, wait for its ACK (and the reply packet if `reply_id` is set).
    ///
    /// Waiters are registered and the bytes queued under one lock hold, so registration
    /// order equals wire order. Replies are matched on `(dev_id, pkt_id)`; the real server
    /// echoes the request's dev_id on every reply (checked against the golden fixtures).
    async fn request(
        &self,
        dev: u32,
        id: u32,
        payload: Vec<u8>,
        reply_id: Option<u32>,
    ) -> Result<Option<Packet>> {
        let (ack_tx, ack_rx) = oneshot::channel();
        let bytes = Packet::new(dev, id, payload).encode();
        let reply_rx = {
            let mut st = lock(&self.shared.store);
            if st.closed {
                return Err(SdkError::Disconnected);
            }
            st.acks.push(PendingAck {
                dev,
                pkt: id,
                tx: Some(ack_tx),
                _permit: None,
            });
            let rx = reply_id.map(|rid| {
                let (tx, rx) = oneshot::channel();
                st.replies.push(PendingReply { dev, pkt: rid, tx });
                rx
            });
            self.shared
                .write_tx
                .send(bytes)
                .map_err(|_| SdkError::Disconnected)?;
            rx
        };
        let t = self.shared.opts.request_timeout;
        let status = tokio::time::timeout(t, ack_rx)
            .await
            .map_err(|_| SdkError::Timeout("ACK"))?
            .map_err(|_| SdkError::Disconnected)?;
        if status != status::OK {
            return Err(SdkError::Rejected { pkt_id: id, status });
        }
        match reply_rx {
            None => Ok(None),
            Some(rx) => {
                let p = tokio::time::timeout(t, rx)
                    .await
                    .map_err(|_| SdkError::Timeout("reply"))?
                    .map_err(|_| SdkError::Disconnected)?;
                Ok(Some(p))
            }
        }
    }

    async fn command(&self, dev: u32, id: u32, payload: Vec<u8>) -> Result<()> {
        self.request(dev, id, payload, None).await.map(|_| ())
    }

    async fn query(&self, dev: u32, id: u32, payload: Vec<u8>) -> Result<Packet> {
        self.request(dev, id, payload, Some(id))
            .await?
            .ok_or(SdkError::BadReply("missing reply"))
    }

    fn require_local(&self) -> Result<()> {
        if self.shared.local {
            Ok(())
        } else {
            Err(SdkError::NotLocalClient)
        }
    }

    fn require_device(&self, id: u32) -> Result<ControllerData> {
        self.device(id).ok_or(SdkError::UnknownDevice(id))
    }

    // ---- device sync -------------------------------------------------------

    /// Re-list controllers and (re)fetch any we do not have. Keeps stable IDs: devices
    /// that are still listed are not re-requested.
    async fn resync(&self) -> Result<()> {
        let _g = self.shared.resync_lock.lock().await;
        let p = self.query(0, pkt::REQUEST_CONTROLLER_COUNT, vec![]).await?;
        let list = decode_controller_list(&p.payload, 6)?;
        let wanted: BTreeSet<u32> = list.ids.iter().copied().collect();
        let (removed, missing): (Vec<u32>, Vec<u32>) = {
            let st = lock(&self.shared.store);
            let have: BTreeSet<u32> = st.devices.keys().copied().collect();
            (
                have.difference(&wanted).copied().collect(),
                wanted.difference(&have).copied().collect(),
            )
        };
        {
            let mut st = lock(&self.shared.store);
            for id in &removed {
                st.devices.remove(id);
            }
        }
        let mut added = Vec::new();
        for id in missing {
            match self.fetch_device(id).await {
                Ok(()) => added.push(id),
                // Vanished between listing and fetching: the next DEVICE_LIST_UPDATED fixes it.
                Err(SdkError::Rejected { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        if !added.is_empty() || !removed.is_empty() {
            let _ = self
                .shared
                .events
                .send(Event::DeviceListChanged { added, removed });
        }
        Ok(())
    }

    async fn fetch_device(&self, id: u32) -> Result<()> {
        let p = self
            .query(id, pkt::REQUEST_CONTROLLER_DATA, encode_protocol_version(6))
            .await?;
        let data = ControllerData::decode_packet(&p.payload, 6)?;
        lock(&self.shared.store).devices.insert(id, data);
        Ok(())
    }

    async fn refresh_device(&self, id: u32) -> Result<()> {
        self.fetch_device(id).await
    }

    pub async fn rescan(&self) -> Result<()> {
        self.command(0, pkt::REQUEST_RESCAN_DEVICES, vec![]).await
    }

    // ---- settings / misc ---------------------------------------------------

    /// JSON string for a settings key (`"null"` when unset).
    pub async fn get_settings(&self, key: &str) -> Result<String> {
        let p = self
            .query(0, pkt::SETTINGS_GET, encode_cstring(key))
            .await?;
        Ok(decode_cstring(&p.payload)?)
    }

    /// Settings schema JSON; an empty key returns every registered schema.
    pub async fn get_settings_schema(&self, key: &str) -> Result<String> {
        let p = self
            .query(0, pkt::SETTINGS_GET_SCHEMA, encode_cstring(key))
            .await?;
        Ok(decode_cstring(&p.payload)?)
    }

    /// Settings for a key, parsed (`Value::Null` when unset).
    pub async fn get_settings_json(&self, key: &str) -> Result<serde_json::Value> {
        let s = self.get_settings(key).await?;
        serde_json::from_str(&s).map_err(|_| SdkError::BadReply("settings reply is not JSON"))
    }

    /// Merge `value` into the server's settings for `key` (nlohmann `update`: object
    /// members are added or overwritten, others kept). Local clients only. In memory until
    /// [`Client::save_settings`].
    ///
    /// The value is validated against the server's schema first
    /// ([`SdkError::InvalidSettings`]): a value with no valid member makes a real
    /// OpenRGB 1.1 server abort, so it is never sent.
    pub async fn modify_settings(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        self.write_settings(pkt::SETTINGS_MODIFY, key, value).await
    }

    /// Replace the server's settings for `key` wholesale. Local clients only. Validated
    /// like [`Client::modify_settings`].
    pub async fn set_settings(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        self.write_settings(pkt::SETTINGS_SET, key, value).await
    }

    /// Persist the server's settings to its config file. Local clients only.
    pub async fn save_settings(&self) -> Result<()> {
        self.require_local()?;
        self.command(0, pkt::SETTINGS_SAVE, vec![]).await
    }

    async fn write_settings(&self, id: u32, key: &str, value: &serde_json::Value) -> Result<()> {
        self.require_local()?;
        let schema: serde_json::Value = serde_json::from_str(&self.get_settings_schema("").await?)
            .map_err(|_| SdkError::BadReply("settings schema is not JSON"))?;
        crate::settings::validate(&schema, key, value)?;
        let body = serde_json::json!({ key: value }).to_string();
        self.command(0, id, encode_cstring(&body)).await
    }

    // ---- diagnostics (local clients only) ----------------------------------

    pub async fn i2c_buses(&self) -> Result<Vec<diag::I2cBus>> {
        Ok(diag::decode_i2c_buses(
            &self.local_query(pkt::GET_I2C_BUS_INFO).await?,
        )?)
    }

    pub async fn hid_devices(&self) -> Result<Vec<diag::HidDevice>> {
        Ok(diag::decode_hid_devices(
            &self.local_query(pkt::GET_HID_DEVICE_INFO).await?,
        )?)
    }

    pub async fn usb_devices(&self) -> Result<Vec<diag::UsbDevice>> {
        Ok(diag::decode_usb_devices(
            &self.local_query(pkt::GET_USB_DEVICE_INFO).await?,
        )?)
    }

    pub async fn serial_ports(&self) -> Result<Vec<String>> {
        Ok(diag::decode_serial_ports(
            &self.local_query(pkt::GET_SERIAL_PORTS).await?,
        )?)
    }

    pub async fn usb_serial_ports(&self) -> Result<Vec<diag::UsbSerialPort>> {
        Ok(diag::decode_usb_serial_ports(
            &self.local_query(pkt::GET_USB_SERIAL_PORTS).await?,
        )?)
    }

    async fn local_query(&self, id: u32) -> Result<Vec<u8>> {
        self.require_local()?;
        Ok(self.query(0, id, vec![]).await?.payload)
    }

    /// Fetch the server's buffered log into [`Client::log_since`]; it is there when this
    /// returns (the server sends the lines, then the ACK). New lines follow on their own. The
    /// server keeps lines only while its `LogManager.log_console` setting is on, which it
    /// reads at startup.
    pub async fn request_log(&self) -> Result<()> {
        self.require_local()?;
        // The buffer replaces what is kept, so asking twice does not repeat lines.
        lock(&self.shared.store).log.lines.clear();
        self.command(0, pkt::LOG_GET_BUFFER, vec![]).await
    }

    /// Empty the server's log buffer and the lines kept here.
    pub async fn clear_log(&self) -> Result<()> {
        self.require_local()?;
        self.command(0, pkt::LOG_CLEAR_BUFFER, vec![]).await?;
        lock(&self.shared.store).log.lines.clear();
        Ok(())
    }

    /// Log lines numbered after `after` (0 for all kept), oldest first.
    pub fn log_since(&self, after: u64) -> Vec<(u64, diag::LogEntry)> {
        let st = lock(&self.shared.store);
        let skip = st.log.lines.partition_point(|(n, _)| *n <= after);
        st.log.lines.range(skip..).cloned().collect()
    }

    pub async fn active_profile(&self) -> Result<String> {
        let p = self.query(0, pkt::PROFILE_GET_ACTIVE, vec![]).await?;
        Ok(decode_cstring(&p.payload)?)
    }

    pub async fn set_hidden(&self, device: u32, hidden: bool) -> Result<()> {
        self.require_device(device)?;
        self.command(device, pkt::SET_HIDDEN, encode_set_hidden(hidden))
            .await
    }

    /// Write `mode` to the device and have it keep it in its own memory (OpenRGB's `SaveMode`).
    /// Only modes flagged `MANUAL_SAVE` do anything with it; the caller checks.
    pub async fn save_mode(&self, device: u32, mode_idx: u32, mode: &Mode) -> Result<()> {
        self.require_device(device)?;
        let payload = encode_update_mode(mode_idx as i32, mode, 6)?;
        self.command(device, pkt::SAVE_MODE, payload).await?;
        self.refresh_device(device).await
    }

    /// Replace a zone's segments (ClearSegments, then one AddSegment each; the server saves
    /// its configuration after each). Segments run in order without overlapping and fit the
    /// zone; a keyboard's key grid is not split.
    pub async fn set_segments(&self, device: u32, zone: u32, segments: &[Segment]) -> Result<()> {
        let dev = self.require_device(device)?;
        let z = dev
            .zones
            .get(zone as usize)
            .ok_or(SdkError::UnknownZone { device, zone })?;
        let mut end = 0u32;
        for s in segments {
            let fits = s.start_idx >= end
                && s.leds_count > 0
                && s.start_idx
                    .checked_add(s.leds_count)
                    .is_some_and(|e| e <= z.leds_count);
            if !fits
                || s.name.trim().is_empty()
                || z.matrix_map.as_ref().is_some_and(|m| m.width > 0)
            {
                return Err(SdkError::BadSegments);
            }
            end = s.start_idx + s.leds_count;
        }
        let payloads = segments
            .iter()
            .map(|s| encode_add_segment(zone as i32, s, 6))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        self.command(
            device,
            pkt::CLEAR_SEGMENTS,
            encode_clear_segments(zone as i32),
        )
        .await?;
        for p in payloads {
            self.command(device, pkt::ADD_SEGMENT, p).await?;
        }
        self.refresh_device(device).await
    }

    pub async fn update_zone_leds(&self, device: u32, zone: u32, colors: &[Color]) -> Result<()> {
        self.require_device(device)?;
        self.command(
            device,
            pkt::UPDATE_ZONE_LEDS,
            encode_update_zone_leds(zone, colors)?,
        )
        .await
    }

    pub async fn update_single_led(&self, device: u32, led: u32, color: Color) -> Result<()> {
        self.require_device(device)?;
        self.command(
            device,
            pkt::UPDATE_SINGLE_LED,
            encode_update_single_led(led as i32, color),
        )
        .await
    }
}

async fn wait_ack(sock: &mut TcpStream, buf: &mut Vec<u8>, t: Duration) -> Result<()> {
    loop {
        if read_packet(sock, buf, t).await?.pkt_id == pkt::ACK {
            return Ok(());
        }
    }
}

async fn reader_loop(
    shared: Arc<Shared>,
    mut rd: tokio::net::tcp::OwnedReadHalf,
    mut buf: Vec<u8>,
) {
    let mut chunk = vec![0u8; 16 * 1024];
    'outer: loop {
        loop {
            match Packet::decode(&buf) {
                Ok(Some((p, used))) => {
                    buf.drain(..used);
                    dispatch(&shared, p);
                }
                Ok(None) => break,
                Err(_) => break 'outer, // malformed stream: cannot resynchronise, drop the connection
            }
        }
        match rd.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let mut st = lock(&shared.store);
    st.closed = true;
    st.acks.clear(); // dropping the senders wakes waiters with Disconnected
    st.replies.clear();
    drop(st);
    let _ = shared.events.send(Event::Disconnected);
}

fn dispatch(shared: &Arc<Shared>, p: Packet) {
    let emit = |e: Event| {
        let _ = shared.events.send(e);
    };
    match p.pkt_id {
        pkt::ACK => {
            let Ok((acked, status)) = decode_ack(&p.payload) else {
                return;
            };
            let mut st = lock(&shared.store);
            if let Some(i) = st
                .acks
                .iter()
                .position(|a| a.dev == p.dev_id && a.pkt == acked)
            {
                let a = st.acks.remove(i);
                if let Some(tx) = a.tx {
                    let _ = tx.send(status);
                }
            }
        }
        pkt::PROFILE_ABOUT_TO_LOAD => {
            // The real server waits for this echo (an ACK does not satisfy it).
            let _ = shared
                .write_tx
                .send(Packet::new(0, pkt::PROFILE_ABOUT_TO_LOAD, vec![]).encode());
        }
        pkt::DEVICE_LIST_UPDATED => {
            let c = Client {
                shared: shared.clone(),
            };
            tokio::spawn(async move {
                let _ = c.resync().await;
            });
        }
        pkt::SIGNAL_UPDATE => {
            let mut r = Reader::new(&p.payload);
            let (Ok(_size), Ok(reason)) = (r.u32(), r.u32()) else {
                return;
            };
            let mut st = lock(&shared.store);
            if reason == update_reason::UPDATE_LEDS {
                if let (Ok(colors), Some(d)) =
                    (decode_color_list(&mut r), st.devices.get_mut(&p.dev_id))
                {
                    d.colors = colors;
                } else {
                    return;
                }
            } else if let (Ok(data), Some(slot)) = (
                ControllerData::decode_block(&mut r, 6),
                st.devices.get_mut(&p.dev_id),
            ) {
                *slot = data;
            } else {
                return;
            }
            drop(st);
            emit(Event::DeviceUpdated {
                id: p.dev_id,
                reason,
            });
        }
        pkt::PROFILE_LIST_UPDATED => {
            if let Ok(names) = decode_profile_list(&p.payload) {
                emit(Event::ProfileListUpdated(names));
            }
        }
        pkt::PROFILE_ACTIVE_CHANGED => {
            if let Ok(n) = decode_cstring(&p.payload) {
                emit(Event::ActiveProfileChanged(n));
            }
        }
        pkt::PROFILE_LOADED => emit(Event::ProfileLoaded),
        pkt::DETECTION_STARTED => emit(Event::DetectionStarted),
        pkt::DETECTION_PROGRESS_CHANGED => {
            if let Ok((percent, label)) = decode_detection_progress(&p.payload) {
                emit(Event::DetectionProgress { percent, label });
            }
        }
        pkt::DETECTION_COMPLETE => emit(Event::DetectionComplete),
        pkt::LOG_LOGGED_ENTRY => {
            if let Ok(e) = diag::decode_log_entry(&p.payload) {
                if lock(&shared.store).log.push(e) {
                    emit(Event::LogUpdated);
                }
            }
        }
        id => {
            // Reply to a pending query?
            let mut st = lock(&shared.store);
            if let Some(i) = st
                .replies
                .iter()
                .position(|r| r.pkt == id && r.dev == p.dev_id)
            {
                let r = st.replies.remove(i);
                let _ = r.tx.send(p);
            }
        }
    }
}

impl RgbBackend for Client {
    fn devices(&self) -> Vec<(u32, ControllerData)> {
        lock(&self.shared.store)
            .devices
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    fn update_leds_nowait(
        &self,
        device: u32,
        colors: &[Color],
    ) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let payload = encode_update_leds(colors);
        async move {
            let payload = payload?;
            let permit = tokio::time::timeout(
                this.shared.opts.request_timeout,
                this.shared.flow.clone().acquire_owned(),
            )
            .await
            .map_err(|_| SdkError::Timeout("flow control: server stopped acknowledging"))?
            .map_err(|_| SdkError::Disconnected)?;
            let bytes = Packet::new(device, pkt::UPDATE_LEDS, payload).encode();
            // Register the ACK slot and queue the bytes under one lock hold (slot order == wire order).
            let mut st = lock(&this.shared.store);
            if st.closed {
                return Err(SdkError::Disconnected);
            }
            st.acks.push(PendingAck {
                dev: device,
                pkt: pkt::UPDATE_LEDS,
                tx: None,
                _permit: Some(permit),
            });
            this.shared
                .write_tx
                .send(bytes)
                .map_err(|_| SdkError::Disconnected)
        }
    }

    fn update_leds(
        &self,
        device: u32,
        colors: &[Color],
    ) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let payload = encode_update_leds(colors);
        async move {
            this.require_device(device)?;
            this.command(device, pkt::UPDATE_LEDS, payload?).await
        }
    }

    fn set_custom_mode(&self, device: u32) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        async move {
            this.require_device(device)?;
            this.command(device, pkt::SET_CUSTOM_MODE, vec![]).await?;
            this.refresh_device(device).await
        }
    }

    fn update_mode(
        &self,
        device: u32,
        mode_idx: u32,
        mode: &Mode,
    ) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let payload = encode_update_mode(mode_idx as i32, mode, 6);
        async move {
            this.require_device(device)?;
            this.command(device, pkt::UPDATE_MODE, payload?).await?;
            this.refresh_device(device).await
        }
    }

    fn update_zone_mode(
        &self,
        device: u32,
        zone: u32,
        mode_idx: i32,
        mode: Option<&Mode>,
    ) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let payload = encode_update_zone_mode(zone as i32, mode_idx, mode, 6);
        async move {
            let dev = this.require_device(device)?;
            let z = dev
                .zones
                .get(zone as usize)
                .ok_or(SdkError::UnknownZone { device, zone })?;
            if mode_idx < -1 || mode_idx >= z.modes.len() as i32 {
                return Err(SdkError::UnknownZoneMode {
                    zone,
                    mode: mode_idx,
                });
            }
            this.command(device, pkt::UPDATE_ZONE_MODE, payload?)
                .await?;
            this.refresh_device(device).await
        }
    }

    fn resize_zone(
        &self,
        device: u32,
        zone: u32,
        size: u32,
    ) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        async move {
            let dev = this.require_device(device)?;
            let z = dev
                .zones
                .get(zone as usize)
                .ok_or(SdkError::UnknownZone { device, zone })?;
            // The real server ACKs OK but silently ignores resizes of unflagged zones.
            if z.flags & (zone_flag::CONFIGURABLE_SIZE | zone_flag::SIZE_EFFECTS_ONLY) == 0 {
                return Err(SdkError::ZoneNotResizable);
            }
            if size < z.leds_min || size > z.leds_max {
                return Err(SdkError::SizeOutOfRange {
                    size,
                    min: z.leds_min,
                    max: z.leds_max,
                });
            }
            this.command(
                device,
                pkt::RESIZE_ZONE,
                encode_resize_zone(zone as i32, size as i32),
            )
            .await?;
            this.refresh_device(device).await
        }
    }

    fn list_profiles(&self) -> impl Future<Output = Result<Vec<String>>> + Send {
        let this = self.clone();
        async move {
            let p = this.query(0, pkt::PROFILE_GET_LIST, vec![]).await?;
            Ok(decode_profile_list(&p.payload)?)
        }
    }

    fn save_profile(&self, name: &str) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let name = name.to_string();
        async move {
            this.require_local()?;
            this.command(0, pkt::PROFILE_SAVE, encode_cstring(&name))
                .await
        }
    }

    fn load_profile(&self, name: &str) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let name = name.to_string();
        async move {
            this.command(0, pkt::PROFILE_LOAD, encode_cstring(&name))
                .await?;
            // The server pushes per-device updates, but re-read to be deterministic.
            let ids: Vec<u32> = lock(&this.shared.store).devices.keys().copied().collect();
            for id in &ids {
                this.refresh_device(*id).await?;
            }
            if this.shared.opts.apply_loaded_profile {
                for id in ids {
                    if let Some(d) = this.device(id) {
                        if let Some(m) = d.modes.get(d.active_mode.max(0) as usize) {
                            this.command(
                                id,
                                pkt::UPDATE_MODE,
                                encode_update_mode(d.active_mode, m, 6)?,
                            )
                            .await?;
                        }
                    }
                }
            }
            Ok(())
        }
    }

    fn delete_profile(&self, name: &str) -> impl Future<Output = Result<()>> + Send {
        let this = self.clone();
        let name = name.to_string();
        async move {
            this.require_local()?;
            this.command(0, pkt::PROFILE_DELETE, encode_cstring(&name))
                .await
        }
    }
}
