// SPDX-License-Identifier: GPL-2.0-or-later
//! Opt-in: the supervisor driving a REAL OpenRGB 1.1 binary (headless, debug devices).
//!
//!   FREERGB_OPENRGB_BIN=/path/to/OpenRGB FREERGB_OPENRGB_CONFIG=/path/to/cfgdir \
//!   cargo test -p freergb-supervisor --test real_openrgb -- --ignored --test-threads=1
//!
//! The config dir should enable DebugDevices (crates/freergb-protocol/tests/fixtures/README.md).
//! Needs no display. Uses an ephemeral port, never 6742.

use freergb_sdk::RgbBackend;
use freergb_supervisor::{Config, ServerCommand, State, Supervisor};
use std::time::Duration;

/// A port nothing is listening on, from below the OS ephemeral range (Linux 32768+,
/// Windows 49152+) so no other test's outgoing connection can land on it.
///
/// Probed by *connecting*, not by binding: a probe listener would stay alive in any child
/// process forked by a concurrently running test (between fork and exec) and make the
/// caller's own bind fail with EADDRINUSE. Seen as a ~3% flake before this.
fn free_port() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let base = 20_000 + (std::process::id() % 4_000) as u16 * 2;
    loop {
        let p = base + NEXT.fetch_add(1, Ordering::Relaxed);
        let target = std::net::SocketAddr::from(([127, 0, 0, 1], p));
        let busy =
            std::net::TcpStream::connect_timeout(&target, std::time::Duration::from_millis(200))
                .is_ok();
        if !busy {
            return p;
        }
    }
}

async fn wait_connected(sup: &Supervisor) {
    let mut rx = sup.watch_state();
    let r = tokio::time::timeout(
        Duration::from_secs(90),
        rx.wait_for(|s| matches!(s, State::Connected { owned: true, .. })),
    )
    .await;
    assert!(r.is_ok(), "not connected; state {:?}", sup.state());
}

#[tokio::test]
#[ignore]
async fn real_openrgb_is_launched_headless_restarted_after_kill_and_stopped() {
    let bin = std::env::var("FREERGB_OPENRGB_BIN").expect("set FREERGB_OPENRGB_BIN");
    let conf = std::env::var("FREERGB_OPENRGB_CONFIG").ok().map(Into::into);
    let port = free_port();
    let sup = Supervisor::start(Config {
        port,
        command: Some(ServerCommand::openrgb(bin, conf)),
        ready_timeout: Duration::from_secs(60),
        restart: freergb_supervisor::RestartPolicy {
            base_delay: Duration::from_millis(200),
            ..Default::default()
        },
        ..Config::default()
    });
    wait_connected(&sup).await;
    let c = sup.client().unwrap();
    assert!(c.is_local_client());
    assert!(
        c.server_name().starts_with("OpenRGB"),
        "{}",
        c.server_name()
    );
    assert!(!c.devices().is_empty());
    let keys_before: Vec<_> = c.device_keys().into_iter().map(|(_, k)| k).collect();

    let pid = sup.owned_pid().unwrap();
    let _ = std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status();
    let mut rx = sup.watch_state();
    let _ = tokio::time::timeout(
        Duration::from_secs(10),
        rx.wait_for(|s| !matches!(s, State::Connected { .. })),
    )
    .await;
    wait_connected(&sup).await;
    let c2 = sup.client().unwrap();
    assert!(!c.is_connected());
    // A freshly started server can accept the handshake before detection has produced any
    // device (seen in ~2% of restarts: an empty list at connect). Devices then arrive as list
    // updates, so "Connected" must never be read as "all devices present": wait for them.
    let t = std::time::Instant::now();
    let at_connect = c2.devices().len();
    let keys_after = loop {
        let k: Vec<_> = c2.device_keys().into_iter().map(|(_, k)| k).collect();
        if k == keys_before {
            break k;
        }
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "devices never (re)appeared after restart: have {} of {}",
            k.len(),
            keys_before.len()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    eprintln!(
        "DEVICES at connect: {at_connect}, complete after {:?}",
        t.elapsed()
    );
    assert_eq!(
        keys_before, keys_after,
        "same devices, same identities after restart"
    );

    sup.shutdown().await;
    let still_up = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok();
    assert!(!still_up, "OpenRGB must be stopped on shutdown");
}
