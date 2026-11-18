// SPDX-License-Identifier: GPL-2.0-or-later

use freergb_protocol::ProtocolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("OpenRGB server speaks protocol {server}, but this app needs protocol 6 or newer (OpenRGB 1.0+). Update OpenRGB.")]
    UnsupportedProtocol { server: u32 },
    #[error("the program on that port did not answer like OpenRGB 1.0 or newer (an old OpenRGB, or a different program)")]
    NotOpenRgb,
    #[error("connection to the OpenRGB server was lost")]
    Disconnected,
    #[error("timed out waiting for the OpenRGB server ({0})")]
    Timeout(&'static str),
    #[error("server rejected request {pkt_id} with status {status}")]
    Rejected { pkt_id: u32, status: u32 },
    #[error("this action needs a local connection to OpenRGB; the server did not grant local-client status")]
    NotLocalClient,
    #[error("no such device (id {0}); it may have been unplugged")]
    UnknownDevice(u32),
    #[error("device {device} has no zone {zone}")]
    UnknownZone { device: u32, zone: u32 },
    #[error("zone {zone} has no mode {mode} of its own")]
    UnknownZoneMode { zone: u32, mode: i32 },
    #[error("segments must be named, hold at least one LED and fit the zone in order")]
    BadSegments,
    #[error("zone cannot be resized (the device does not allow it)")]
    ZoneNotResizable,
    #[error("zone size {size} is outside the allowed range {min}..={max}")]
    SizeOutOfRange { size: u32, min: u32, max: u32 },
    #[error("invalid settings: {0}")]
    InvalidSettings(String),
    #[error("unexpected reply from server: {0}")]
    BadReply(&'static str),
}

pub type Result<T> = std::result::Result<T, SdkError>;
