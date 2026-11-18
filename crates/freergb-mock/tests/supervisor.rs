// SPDX-License-Identifier: GPL-2.0-or-later
//! Supervisor behaviour with the real `freergb-mock` binary as the "OpenRGB" child process.
//! Lives in this package because `CARGO_BIN_EXE_*` is only set for a package's own tests.
//! Every port is ephemeral; 6742 is never touched.

use freergb_mock::{devices, MockServer};
use freergb_sdk::{Client, ConnectOptions, RgbBackend};
use freergb_supervisor::{Config, FailureKind, RestartPolicy, ServerCommand, State, Supervisor};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

const MOCK: &str = env!("CARGO_BIN_EXE_freergb-mock");

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

fn addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("freergb-sup-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

fn mock_cmd(env: &[(&str, &PathBuf)], flags: &[(&str, &str)]) -> ServerCommand {
    let mut c = ServerCommand::openrgb(MOCK, None);
    c.env = env
        .iter()
        .map(|(k, v)| ((*k).into(), v.as_os_str().to_owned()))
        .chain(flags.iter().map(|(k, v)| ((*k).into(), (*v).into())))
        .collect();
    c
}

fn cfg(port: u16, command: Option<ServerCommand>) -> Config {
    Config {
        port,
        command,
        probe_timeout: Duration::from_millis(400),
        ready_timeout: Duration::from_secs(10),
        ready_poll: Duration::from_millis(40),
        stop_grace: Duration::from_secs(1),
        foreign_grace: Duration::from_millis(400),
        restart: RestartPolicy {
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(300),
            healthy_after: Duration::from_secs(30),
            max_consecutive_failures: 3,
        },
        ..Config::default()
    }
}

async fn wait_state(sup: &Supervisor, what: &str, pred: impl Fn(&State) -> bool) -> State {
    let mut rx = sup.watch_state();
    let res = tokio::time::timeout(Duration::from_secs(15), rx.wait_for(|s| pred(s))).await;
    match res {
        Ok(Ok(s)) => s.clone(),
        _ => panic!("timed out waiting for {what}; state is {:?}", sup.state()),
    }
}

fn connected(owned: bool) -> impl Fn(&State) -> bool {
    move |s| matches!(s, State::Connected { owned: o, .. } if *o == owned)
}

async fn can_connect(port: u16) -> bool {
    Client::connect(addr(port), ConnectOptions::default())
        .await
        .is_ok()
}

async fn port_closes(port: u16) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while tokio::net::TcpStream::connect(addr(port)).await.is_ok() {
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .expect("server on the port is still running");
}

#[cfg(unix)]
fn kill9(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status();
}

#[tokio::test]
async fn existing_server_is_used_nothing_is_launched_and_it_survives_shutdown() {
    let port = free_port();
    let _foreign = MockServer::start_on(devices::default_set(), port)
        .await
        .unwrap();
    let tripwire = tmp("t1");
    let sup = Supervisor::start(cfg(
        port,
        Some(mock_cmd(&[("FREERGB_MOCK_TRIPWIRE", &tripwire)], &[])),
    ));
    wait_state(&sup, "adopting the existing server", connected(false)).await;
    assert!(sup.owned_pid().is_none());
    assert!(!sup.client().unwrap().devices().is_empty());
    sup.shutdown().await;
    assert!(
        can_connect(port).await,
        "a server we did not start must keep running"
    );
    assert!(
        !tripwire.exists(),
        "nothing may be launched when a server answers"
    );
}

#[tokio::test]
async fn launches_when_nothing_listens_and_stops_its_own_child() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, Some(mock_cmd(&[], &[]))));
    wait_state(&sup, "owned server", connected(true)).await;
    assert!(sup.owned_pid().is_some());
    assert!(!sup.client().unwrap().devices().is_empty());
    sup.shutdown().await;
    port_closes(port).await;
}

#[cfg(unix)]
#[tokio::test]
async fn owned_server_killed_is_restarted_and_reconnected() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, Some(mock_cmd(&[], &[]))));
    wait_state(&sup, "first start", connected(true)).await;
    let first_pid = sup.owned_pid().unwrap();
    let old_client = sup.client().unwrap();
    kill9(first_pid);
    wait_state(&sup, "restart", |s| {
        matches!(
            s,
            State::Reconnecting { .. } | State::Starting { owned: true }
        )
    })
    .await;
    wait_state(&sup, "reconnect", connected(true)).await;
    let second_pid = sup.owned_pid().unwrap();
    assert_ne!(first_pid, second_pid);
    assert!(!old_client.is_connected());
    let new_client = sup.client().unwrap();
    assert!(new_client.is_connected());
    assert!(!new_client.devices().is_empty());
    sup.shutdown().await;
}

#[tokio::test]
async fn a_restart_asked_for_starts_the_owned_server_again_and_leaves_a_foreign_one() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, Some(mock_cmd(&[], &[]))));
    wait_state(&sup, "first start", connected(true)).await;
    let first_pid = sup.owned_pid().unwrap();
    assert!(sup.restart_owned());
    wait_state(&sup, "stopping", |s| !matches!(s, State::Connected { .. })).await;
    wait_state(&sup, "back", connected(true)).await;
    assert_ne!(sup.owned_pid().unwrap(), first_pid);
    sup.shutdown().await;

    let foreign = MockServer::start_on(devices::default_set(), free_port())
        .await
        .unwrap();
    let sup = Supervisor::start(cfg(foreign.addr().port(), None));
    wait_state(&sup, "adopted", connected(false)).await;
    assert!(!sup.restart_owned());
    sup.shutdown().await;
}

#[tokio::test]
async fn crash_loop_ends_failed_with_the_childs_stderr() {
    let port = free_port();
    let sup = Supervisor::start(cfg(
        port,
        Some(mock_cmd(&[], &[("FREERGB_MOCK_EXIT", "3")])),
    ));
    let s = wait_state(&sup, "crash loop", |s| matches!(s, State::Failed(_))).await;
    let State::Failed(f) = s else { unreachable!() };
    assert_eq!(f.kind, FailureKind::CrashLoop);
    assert!(
        f.message.contains("refusing to start"),
        "stderr tail missing: {}",
        f.message
    );
    assert!(f.message.contains("3 times"), "{}", f.message);
    assert!(sup.owned_pid().is_none());
    sup.shutdown().await;
}

#[tokio::test]
async fn crash_once_then_recovers_without_failing() {
    let port = free_port();
    let marker = tmp("crash-once");
    let sup = Supervisor::start(cfg(
        port,
        Some(mock_cmd(&[("FREERGB_MOCK_CRASH_ONCE", &marker)], &[])),
    ));
    // The child dies 300 ms after launch, which may be before or after we connected; assert
    // on the whole state history instead of on timing.
    let history = std::sync::Arc::new(std::sync::Mutex::new(vec![sup.state()]));
    let mut w = sup.watch_state();
    let h = history.clone();
    tokio::spawn(async move {
        while w.changed().await.is_ok() {
            h.lock().unwrap().push(w.borrow().clone());
        }
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        {
            let h = history.lock().unwrap();
            let crashed = h.iter().any(|s| matches!(s, State::Reconnecting { .. }));
            if crashed && matches!(h.last(), Some(State::Connected { owned: true, .. })) {
                break;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no recovery; history: {:?}",
            history.lock().unwrap()
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert!(marker.exists(), "the crash hook must have fired");
    assert!(!history
        .lock()
        .unwrap()
        .iter()
        .any(|s| matches!(s, State::Failed(_))));
    assert!(sup.client().unwrap().is_connected());
    sup.shutdown().await;
}

#[tokio::test]
async fn startup_timeout_kills_the_child_and_fails() {
    let port = free_port();
    let mut c = cfg(
        port,
        Some(mock_cmd(&[], &[("FREERGB_MOCK_NEVER_LISTEN", "1")])),
    );
    c.ready_timeout = Duration::from_millis(300);
    c.restart.max_consecutive_failures = 2;
    let sup = Supervisor::start(c);
    let s = wait_state(&sup, "start timeout", |s| matches!(s, State::Failed(_))).await;
    let State::Failed(f) = s else { unreachable!() };
    assert_eq!(f.kind, FailureKind::StartTimeout);
    assert!(sup.owned_pid().is_none(), "timed-out child must be stopped");
    sup.shutdown().await;
}

#[tokio::test]
async fn unlaunchable_program_is_a_clear_failure() {
    let port = free_port();
    let cmd = ServerCommand::openrgb("/nonexistent/openrgb", None);
    let sup = Supervisor::start(cfg(port, Some(cmd)));
    let s = wait_state(&sup, "launch failure", |s| matches!(s, State::Failed(_))).await;
    let State::Failed(f) = s else { unreachable!() };
    assert_eq!(f.kind, FailureKind::LaunchFailed);
    assert!(f.message.contains("/nonexistent/openrgb"));
    sup.shutdown().await;
}

#[tokio::test]
async fn old_protocol_is_refused_and_nothing_is_launched() {
    let port = free_port();
    let foreign = MockServer::start_on(devices::default_set(), port)
        .await
        .unwrap();
    foreign.set_max_protocol(5);
    let tripwire = tmp("t-old");
    let sup = Supervisor::start(cfg(
        port,
        Some(mock_cmd(&[("FREERGB_MOCK_TRIPWIRE", &tripwire)], &[])),
    ));
    let s = wait_state(&sup, "version refusal", |s| matches!(s, State::Failed(_))).await;
    let State::Failed(f) = s else { unreachable!() };
    assert_eq!(f.kind, FailureKind::UnsupportedVersion);
    assert!(
        f.message.contains("protocol 5") && f.message.contains("Update OpenRGB"),
        "{}",
        f.message
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        matches!(sup.state(), State::Failed(_)),
        "must not retry on its own"
    );
    assert!(!tripwire.exists());
    sup.shutdown().await;
}

#[tokio::test]
async fn a_silent_listener_is_reported_as_port_in_use_and_nothing_is_launched() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let held = tokio::spawn(async move {
        let mut socks = vec![];
        while let Ok((s, _)) = l.accept().await {
            socks.push(s); // accept and say nothing
        }
    });
    let tripwire = tmp("t-silent");
    let sup = Supervisor::start(cfg(
        port,
        Some(mock_cmd(&[("FREERGB_MOCK_TRIPWIRE", &tripwire)], &[])),
    ));
    let s = wait_state(&sup, "port-in-use", |s| matches!(s, State::Failed(_))).await;
    let State::Failed(f) = s else { unreachable!() };
    assert_eq!(f.kind, FailureKind::PortInUse);
    assert!(!tripwire.exists());
    sup.shutdown().await;
    held.abort();
}

#[tokio::test]
async fn retry_leaves_failed_and_adopts_a_server_that_has_since_appeared() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, None));
    let s = wait_state(&sup, "no command", |s| matches!(s, State::Failed(_))).await;
    let State::Failed(f) = s else { unreachable!() };
    assert_eq!(f.kind, FailureKind::NoServerCommand);
    let _foreign = MockServer::start_on(devices::default_set(), port)
        .await
        .unwrap();
    sup.retry();
    wait_state(&sup, "adoption after retry", connected(false)).await;
    sup.shutdown().await;
}

#[tokio::test]
async fn a_server_that_appears_during_restart_backoff_is_adopted_not_fought() {
    let port = free_port();
    let mut c = cfg(port, Some(mock_cmd(&[], &[("FREERGB_MOCK_EXIT", "3")])));
    c.restart.max_consecutive_failures = 1000;
    let sup = Supervisor::start(c);
    wait_state(
        &sup,
        "backoff",
        |s| matches!(s, State::Reconnecting { attempt, .. } if *attempt >= 1),
    )
    .await;
    let _user_started_openrgb = MockServer::start_on(devices::default_set(), port)
        .await
        .unwrap();
    wait_state(&sup, "adoption", connected(false)).await;
    assert!(sup.owned_pid().is_none());
    sup.shutdown().await;
}

#[tokio::test]
async fn foreign_server_that_comes_back_within_grace_is_not_replaced() {
    let port = free_port();
    let foreign = MockServer::start_on(devices::default_set(), port)
        .await
        .unwrap();
    let tripwire = tmp("t-grace");
    let mut c = cfg(
        port,
        Some(mock_cmd(&[("FREERGB_MOCK_TRIPWIRE", &tripwire)], &[])),
    );
    c.foreign_grace = Duration::from_secs(3);
    let sup = Supervisor::start(c);
    wait_state(&sup, "adoption", connected(false)).await;
    let first = sup.client().unwrap();
    foreign.crash();
    wait_state(&sup, "loss", |s| !matches!(s, State::Connected { .. })).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    foreign.restart().await.unwrap();
    wait_state(&sup, "re-adoption", connected(false)).await;
    assert!(!first.is_connected());
    assert!(sup.owned_pid().is_none());
    assert!(!tripwire.exists());
    sup.shutdown().await;
}

#[tokio::test]
async fn foreign_server_that_stays_gone_is_taken_over_after_grace() {
    let port = free_port();
    let foreign = MockServer::start_on(devices::default_set(), port)
        .await
        .unwrap();
    let sup = Supervisor::start(cfg(port, Some(mock_cmd(&[], &[]))));
    wait_state(&sup, "adoption", connected(false)).await;
    foreign.crash();
    wait_state(&sup, "takeover", connected(true)).await;
    assert!(sup.owned_pid().is_some());
    sup.shutdown().await;
    port_closes(port).await;
}

#[tokio::test]
async fn dropping_the_supervisor_kills_the_child_it_started() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, Some(mock_cmd(&[], &[]))));
    wait_state(&sup, "owned server", connected(true)).await;
    drop(sup);
    port_closes(port).await;
}

/// Pins today's behaviour for a server orphaned by a crashed backend: it is indistinguishable
/// from a foreign one, so it is adopted as External and never killed by us.
#[tokio::test]
async fn an_orphaned_server_is_treated_as_foreign_and_left_running() {
    let port = free_port();
    let mut orphan = std::process::Command::new(MOCK)
        .args(["--server-port", &port.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !can_connect(port).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("orphan did not start");
    let sup = Supervisor::start(cfg(port, Some(mock_cmd(&[], &[]))));
    wait_state(&sup, "adopting the orphan", connected(false)).await;
    sup.shutdown().await;
    assert!(can_connect(port).await, "must not be killed");
    orphan.kill().unwrap();
    orphan.wait().unwrap();
}

/// An AppImage's AppRun starts OpenRGB as a grandchild, so the process we spawn is a wrapper.
/// Both the clean stop and the drop path must take the whole group down.
#[cfg(unix)]
fn wrapped_cmd() -> ServerCommand {
    ServerCommand {
        program: "sh".into(),
        args: [
            "-c",
            r#""$0" "$@" & wait"#,
            MOCK,
            "--server",
            "--noautoconnect",
            "--server-port",
            "{port}",
        ]
        .map(String::from)
        .to_vec(),
        env: vec![],
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_wrapped_server_is_stopped_with_its_wrapper_on_shutdown() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, Some(wrapped_cmd())));
    wait_state(&sup, "wrapped server", connected(true)).await;
    sup.shutdown().await;
    port_closes(port).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_wrapped_server_is_killed_with_its_wrapper_when_the_supervisor_is_dropped() {
    let port = free_port();
    let sup = Supervisor::start(cfg(port, Some(wrapped_cmd())));
    wait_state(&sup, "wrapped server", connected(true)).await;
    drop(sup);
    port_closes(port).await;
}
