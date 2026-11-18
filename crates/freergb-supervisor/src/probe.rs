// SPDX-License-Identifier: GPL-2.0-or-later
//! What is on the server port right now? Uses a real SDK handshake as the probe, so
//! "something accepts TCP" is never mistaken for "an OpenRGB server is there".

use freergb_sdk::{Client, ConnectOptions, SdkError};
use std::net::SocketAddr;
use std::time::Duration;

pub enum Probe {
    /// An SDK server answered and the connection is ready: use it, do not launch anything.
    Server(Client),
    /// Nothing listening. The only outcome that allows launching our own server.
    Closed,
    /// Speaks protocol < 6: refuse, with the version it reported.
    TooOld(u32),
    /// Something holds the port but is not OpenRGB 1.0+ (silent, garbage, or old OpenRGB).
    NotOpenRgb,
    /// Dropped or failed mid-handshake: probably a server that is still starting.
    Unsettled(String),
}

pub async fn probe(addr: SocketAddr, opts: &ConnectOptions, timeout: Duration) -> Probe {
    let opts = ConnectOptions {
        request_timeout: timeout,
        ..opts.clone()
    };
    match Client::connect(addr, opts).await {
        Ok(c) => Probe::Server(c),
        Err(e) => classify(e),
    }
}

/// Map a failed connect to what it says about the port.
pub(crate) fn classify(e: SdkError) -> Probe {
    match e {
        SdkError::UnsupportedProtocol { server } => Probe::TooOld(server),
        SdkError::NotOpenRgb => Probe::NotOpenRgb,
        // Refused, or no TCP handshake within the connect timeout: nothing accepted us, so
        // nothing holds the port. (A closed loopback port can take ~2 s to refuse on Windows.)
        SdkError::Io(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => Probe::Closed,
        SdkError::Timeout("connect") => Probe::Closed,
        e => Probe::Unsettled(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn nothing_accepted_means_closed() {
        assert!(matches!(
            classify(SdkError::Timeout("connect")),
            Probe::Closed
        ));
        assert!(matches!(
            classify(SdkError::Io(Error::from(ErrorKind::ConnectionRefused))),
            Probe::Closed
        ));
    }

    #[test]
    fn something_answered_badly_is_not_closed() {
        assert!(matches!(classify(SdkError::NotOpenRgb), Probe::NotOpenRgb));
        assert!(matches!(
            classify(SdkError::UnsupportedProtocol { server: 5 }),
            Probe::TooOld(5)
        ));
        // accepted, then went quiet or dropped us during the handshake
        assert!(matches!(
            classify(SdkError::Timeout("handshake")),
            Probe::Unsettled(_)
        ));
        assert!(matches!(
            classify(SdkError::Disconnected),
            Probe::Unsettled(_)
        ));
        assert!(matches!(
            classify(SdkError::Io(Error::from(ErrorKind::ConnectionReset))),
            Probe::Unsettled(_)
        ));
    }
}
