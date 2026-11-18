// SPDX-License-Identifier: GPL-2.0-or-later

use crate::error::{ProtocolError, Result};
use crate::ids::{HEADER_LEN, MAGIC};

/// Upper bound on an accepted payload. Real controller blocks are far smaller
/// (a 300-LED device is ~10 KB); this only guards against hostile sizes.
pub const MAX_PAYLOAD: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Device index (protocol < 6) or stable controller ID (protocol >= 6).
    pub dev_id: u32,
    pub pkt_id: u32,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(dev_id: u32, pkt_id: u32, payload: Vec<u8>) -> Self {
        Self {
            dev_id,
            pkt_id,
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.dev_id.to_le_bytes());
        out.extend_from_slice(&self.pkt_id.to_le_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Streaming parse: `Ok(None)` means more bytes are needed. On success
    /// returns the packet and how many bytes of `buf` it consumed.
    /// Bad magic and oversized payloads are errors so the caller can drop the
    /// connection rather than buffer forever.
    pub fn decode(buf: &[u8]) -> Result<Option<(Packet, usize)>> {
        if buf.len() < HEADER_LEN {
            return Ok(None);
        }
        let magic: [u8; 4] = buf[0..4].try_into().unwrap();
        if magic != MAGIC {
            return Err(ProtocolError::BadMagic(magic));
        }
        let u32_at = |i: usize| u32::from_le_bytes(buf[i..i + 4].try_into().unwrap());
        let (dev_id, pkt_id, size) = (u32_at(4), u32_at(8), u32_at(12));
        if size > MAX_PAYLOAD {
            return Err(ProtocolError::PacketTooLarge(size, MAX_PAYLOAD));
        }
        let total = HEADER_LEN + size as usize;
        if buf.len() < total {
            return Ok(None);
        }
        Ok(Some((
            Packet {
                dev_id,
                pkt_id,
                payload: buf[HEADER_LEN..total].to_vec(),
            },
            total,
        )))
    }
}
