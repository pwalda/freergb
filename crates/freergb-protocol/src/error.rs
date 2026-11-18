// SPDX-License-Identifier: GPL-2.0-or-later

use thiserror::Error;

/// Errors from decoding or encoding SDK packets. Decoding never panics on
/// malformed input; every bounds problem surfaces as one of these.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("truncated packet: needed {needed} more byte(s), {available} available")]
    Truncated { needed: usize, available: usize },
    #[error("bad packet magic {0:?}, expected \"ORGB\"")]
    BadMagic([u8; 4]),
    #[error("string is missing its NUL terminator")]
    StringNotTerminated,
    #[error("declared element count {count} cannot fit in the {available} remaining byte(s)")]
    CountTooLarge { count: usize, available: usize },
    #[error("packet size {0} exceeds the limit of {1}")]
    PacketTooLarge(u32, u32),
    #[error("value too large to encode: {0}")]
    TooLargeToEncode(&'static str),
    #[error("invalid matrix map: {0}")]
    InvalidMatrixMap(&'static str),
    #[error("{0} trailing byte(s) after packet payload")]
    TrailingBytes(usize),
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
