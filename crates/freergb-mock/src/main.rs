// SPDX-License-Identifier: GPL-2.0-or-later
//! `freergb-mock [PORT]` / `freergb-mock --server --server-port PORT ...`: run the mock server
//! with the default device set. Other OpenRGB flags (`--noautoconnect`, `--config X`) are
//! accepted and ignored so it can stand in for OpenRGB as a supervised child process.
//!
//! Stdin commands: `add <strip|ram|flaky>`, `remove <id>`, `crash`, `restart`, `quit`.
//! With stdin closed (as under a supervisor) it just keeps serving.
//!
//! Test hooks (environment):
//! - `FREERGB_MOCK_TRIPWIRE=PATH`: write PATH first thing, proving this process was launched.
//! - `FREERGB_MOCK_EXIT=N`: print "mock: refusing to start" to stderr and exit with code N.
//! - `FREERGB_MOCK_NEVER_LISTEN=1`: start but never open the port.
//! - `FREERGB_MOCK_MAX_PROTOCOL=N`: report at most this protocol version.
//! - `FREERGB_MOCK_CRASH_ONCE=PATH`: if PATH does not exist, create it and exit(1) 300 ms
//!   after startup; later runs behave normally.

use freergb_mock::{devices, MockServer};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    if let Ok(path) = std::env::var("FREERGB_MOCK_TRIPWIRE") {
        std::fs::write(path, b"launched")?;
    }
    if let Some(code) = std::env::var("FREERGB_MOCK_EXIT")
        .ok()
        .and_then(|c| c.parse::<i32>().ok())
    {
        eprintln!("mock: refusing to start");
        std::process::exit(code);
    }
    let mut port: u16 = 6742;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--server-port" {
            port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port);
        } else if let Ok(p) = a.parse() {
            port = p;
        }
    }
    if std::env::var_os("FREERGB_MOCK_NEVER_LISTEN").is_some() {
        std::future::pending::<()>().await;
    }
    let server = MockServer::start_on(devices::default_set(), port).await?;
    if let Some(v) = std::env::var("FREERGB_MOCK_MAX_PROTOCOL")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        server.set_max_protocol(v);
    }
    if let Ok(marker) = std::env::var("FREERGB_MOCK_CRASH_ONCE") {
        if !std::path::Path::new(&marker).exists() {
            std::fs::write(&marker, b"crashed")?;
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                eprintln!("mock: simulated crash");
                std::process::exit(1);
            });
        }
    }
    println!("mock OpenRGB server listening on {}", server.addr());
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut w = line.split_whitespace();
        match (w.next(), w.next()) {
            (Some("add"), Some("strip")) => {
                println!("added id {}", server.add_device(devices::resizable_strip()))
            }
            (Some("add"), Some("ram")) => {
                println!("added id {}", server.add_device(devices::ram_stick(9)))
            }
            (Some("add"), Some("flaky")) => {
                println!("added id {}", server.add_device(devices::flaky()))
            }
            (Some("remove"), Some(id)) => println!(
                "removed: {}",
                id.parse().is_ok_and(|i| server.remove_device(i))
            ),
            (Some("crash"), _) => server.crash(),
            (Some("restart"), _) => println!("restart: {:?}", server.restart().await),
            (Some("quit"), _) => return Ok(()),
            _ => println!("commands: add strip|ram|flaky, remove ID, crash, restart, quit"),
        }
    }
    // EOF on stdin (supervised, stdin=null) is not a reason to stop serving.
    std::future::pending::<()>().await;
    Ok(())
}
