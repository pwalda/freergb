// SPDX-License-Identifier: GPL-2.0-or-later
//! Little-endian, bounds-checked primitives shared by every packet codec.

use crate::error::{ProtocolError, Result};

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(ProtocolError::Truncated {
                needed: n - self.remaining(),
                available: self.remaining(),
            });
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take(n)
    }

    /// Validate that `count` elements of at least `min_elem` bytes each could
    /// still fit, so a hostile count cannot trigger a huge allocation.
    pub fn check_count(&self, count: usize, min_elem: usize) -> Result<()> {
        if count.saturating_mul(min_elem) > self.remaining() {
            return Err(ProtocolError::CountTooLarge {
                count,
                available: self.remaining(),
            });
        }
        Ok(())
    }

    fn cstr(&mut self, len: usize) -> Result<String> {
        let raw = self.take(len)?;
        // Length includes the NUL terminator. Anything after the first NUL is
        // ignored, matching how the C++ side treats these as C strings.
        match raw.iter().position(|&b| b == 0) {
            Some(end) => Ok(String::from_utf8_lossy(&raw[..end]).into_owned()),
            None => Err(ProtocolError::StringNotTerminated),
        }
    }

    /// `u16` length (including NUL) followed by that many bytes.
    pub fn string16(&mut self) -> Result<String> {
        let len = self.u16()? as usize;
        self.cstr(len)
    }

    /// `u32` length (including NUL) followed by that many bytes.
    pub fn string32(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        self.check_count(len, 1)?;
        self.cstr(len)
    }

    /// Read a NUL-terminated string that fills the rest of the buffer.
    pub fn rest_cstr(&mut self) -> Result<String> {
        let n = self.remaining();
        self.cstr(n)
    }

    pub fn finish(self) -> Result<()> {
        match self.remaining() {
            0 => Ok(()),
            n => Err(ProtocolError::TrailingBytes(n)),
        }
    }
}

#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }

    pub fn string16(&mut self, s: &str) -> Result<()> {
        let len = s.len() + 1;
        let len16 = u16::try_from(len).map_err(|_| ProtocolError::TooLargeToEncode("string16"))?;
        self.u16(len16);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
        Ok(())
    }

    pub fn string32(&mut self, s: &str) -> Result<()> {
        let len =
            u32::try_from(s.len() + 1).map_err(|_| ProtocolError::TooLargeToEncode("string32"))?;
        self.u32(len);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
        Ok(())
    }

    pub fn cstr(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
}
