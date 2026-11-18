// SPDX-License-Identifier: GPL-2.0-or-later
//! Detect-first OpenRGB supervisor.
//!
//! 1. Probe the SDK port with a real handshake. An SDK server there: connect, launch nothing.
//! 2. Nothing listening: start the bundled OpenRGB headless, wait for it, connect.
//! 3. Only a process this supervisor started is ever stopped or restarted.
//! 4. An owned server that dies is restarted with backoff; too many failures in a row end in
//!    [`State::Failed`] with the server's last stderr lines. Before every restart the port is
//!    probed again, so an OpenRGB the user started in the meantime is adopted, not fought.
//! 5. A wrong protocol version, or another program on the port, is `Failed` with a plain
//!    message; nothing is launched and nothing is retried until [`Supervisor::retry`].
//! 6. A foreign server that disappears is waited for ([`Config::foreign_grace`]); after that
//!    the bundled server takes over.
//!
//! Controller IDs are only stable within one server process, so a new [`Client`] after a
//! restart must be matched to old state with [`freergb_sdk::DeviceKey`], never raw IDs.

mod launcher;
mod policy;
mod probe;

pub use launcher::{OwnedServer, ServerCommand};
pub use policy::{RestartPolicy, RestartState, Verdict};

use freergb_sdk::{Client, ConnectOptions, Event};
use probe::{probe, Probe};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast::error::RecvError, watch, Notify};

#[derive(Debug, Clone)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    /// How to launch a server. `None`: connect-only, never launch.
    pub command: Option<ServerCommand>,
    pub connect: ConnectOptions,
    /// Handshake timeout for each probe (the TCP connect has `connect.connect_timeout`).
    pub probe_timeout: Duration,
    /// How long a launched server gets to start answering. Real SMBus detection on a
    /// loaded machine is slow (12 s with only debug devices), so be generous.
    pub ready_timeout: Duration,
    pub ready_poll: Duration,
    /// Grace between asking the owned server to exit and killing it.
    pub stop_grace: Duration,
    /// How long to wait for a vanished foreign server to come back before launching ours.
    pub foreign_grace: Duration,
    pub restart: RestartPolicy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 6742,
            command: None,
            connect: ConnectOptions::default(),
            probe_timeout: Duration::from_secs(2),
            ready_timeout: Duration::from_secs(60),
            ready_poll: Duration::from_millis(250),
            stop_grace: Duration::from_secs(3),
            foreign_grace: Duration::from_secs(5),
            restart: RestartPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    UnsupportedVersion,
    PortInUse,
    NoServerCommand,
    LaunchFailed,
    StartTimeout,
    CrashLoop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    /// Plain-language, safe to show to the user.
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Probing, or waiting for a server we just launched.
    Starting {
        owned: bool,
    },
    Connected {
        owned: bool,
        server_name: String,
    },
    /// Between attempts. `retry_in` is zero while probing for a vanished foreign server.
    Reconnecting {
        attempt: u32,
        retry_in: Duration,
    },
    Failed(Failure),
}

pub struct Supervisor {
    state: watch::Receiver<State>,
    client: watch::Receiver<Option<Client>>,
    pid: Arc<Mutex<Option<u32>>>,
    retry: Arc<Notify>,
    restart: Arc<Notify>,
    shutdown: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Supervisor {
    pub fn start(config: Config) -> Self {
        let (state_tx, state) = watch::channel(State::Starting { owned: false });
        let (client_tx, client) = watch::channel(None);
        let (shutdown, shutdown_rx) = watch::channel(false);
        let pid = Arc::new(Mutex::new(None));
        let retry = Arc::new(Notify::new());
        let restart = Arc::new(Notify::new());
        let task = tokio::spawn(run(
            config,
            Out {
                state: state_tx,
                client: client_tx,
                pid: pid.clone(),
            },
            Signals {
                retry: retry.clone(),
                restart: restart.clone(),
            },
            shutdown_rx,
        ));
        Self {
            state,
            client,
            pid,
            retry,
            restart,
            shutdown,
            task: Some(task),
        }
    }

    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    pub fn watch_state(&self) -> watch::Receiver<State> {
        self.state.clone()
    }

    /// The live connection, if any. A new `Client` appears after every reconnect; watch
    /// [`Supervisor::watch_client`] to follow them.
    pub fn client(&self) -> Option<Client> {
        self.client.borrow().clone()
    }

    pub fn watch_client(&self) -> watch::Receiver<Option<Client>> {
        self.client.clone()
    }

    /// PID of the server this supervisor started, if one is running.
    pub fn owned_pid(&self) -> Option<u32> {
        *self.pid.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Leave `Failed` and try again from the top (also forgets the crash history).
    pub fn retry(&self) {
        self.retry.notify_one();
    }

    /// Stop the server this supervisor started and start it again (it re-reads its settings).
    /// False, and nothing happens, unless an owned server is connected.
    pub fn restart_owned(&self) -> bool {
        let owned = matches!(*self.state.borrow(), State::Connected { owned: true, .. });
        if owned {
            self.restart.notify_one();
        }
        owned
    }

    /// Stop supervising. Stops the owned server if there is one; never touches a foreign one.
    pub async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(t) = self.task.take() {
            let _ = t.await;
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // Not shut down cleanly: abort the task; dropping its `OwnedServer` kills the child.
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}

struct Out {
    state: watch::Sender<State>,
    client: watch::Sender<Option<Client>>,
    pid: Arc<Mutex<Option<u32>>>,
}

impl Out {
    fn state(&self, s: State) {
        self.state.send_replace(s);
    }
    fn set_pid(&self, p: Option<u32>) {
        *self.pid.lock().unwrap_or_else(|e| e.into_inner()) = p;
    }
}

struct Signals {
    retry: Arc<Notify>,
    restart: Arc<Notify>,
}

struct Shutdown;
type Flow<T> = Result<T, Shutdown>;

/// Await `fut`, unless shutdown is requested first.
async fn guard<T>(
    sd: &mut watch::Receiver<bool>,
    fut: impl std::future::Future<Output = T>,
) -> Flow<T> {
    tokio::select! {
        _ = sd.wait_for(|v| *v) => Err(Shutdown),
        v = fut => Ok(v),
    }
}

async fn run(cfg: Config, out: Out, sig: Signals, mut sd: watch::Receiver<bool>) {
    let mut owned: Option<OwnedServer> = None;
    let _ = supervise(&cfg, &out, &sig, &mut sd, &mut owned).await;
    out.client.send_replace(None);
    if let Some(mut o) = owned.take() {
        o.stop(cfg.stop_grace).await;
    }
    out.set_pid(None);
}

async fn supervise(
    cfg: &Config,
    out: &Out,
    sig: &Signals,
    sd: &mut watch::Receiver<bool>,
    owned: &mut Option<OwnedServer>,
) -> Flow<()> {
    let retry = &*sig.retry;
    let addr = SocketAddr::new(cfg.host, cfg.port);
    let mut restart = RestartState::new(cfg.restart.clone());
    let mut foreign_lost_at: Option<Instant> = None;
    let mut unsettled = 0u32;
    let fail = |kind, message: String| {
        out.client.send_replace(None);
        out.state(State::Failed(Failure { kind, message }));
    };

    loop {
        out.state(State::Starting { owned: false });
        match guard(sd, probe(addr, &cfg.connect, cfg.probe_timeout)).await? {
            Probe::Server(client) => {
                unsettled = 0;
                restart.reset();
                connected(out, sd, client, false, None).await?;
                foreign_lost_at = Some(Instant::now());
            }
            Probe::Closed => {
                unsettled = 0;
                if foreign_lost_at.is_some_and(|t| t.elapsed() < cfg.foreign_grace) {
                    out.state(State::Reconnecting {
                        attempt: 1,
                        retry_in: Duration::ZERO,
                    });
                    guard(sd, tokio::time::sleep(cfg.ready_poll)).await?;
                    continue;
                }
                foreign_lost_at = None;
                let Some(cmd) = &cfg.command else {
                    fail(
                        FailureKind::NoServerCommand,
                        format!("No OpenRGB server is running on port {} and no bundled OpenRGB is configured.", cfg.port),
                    );
                    guard(sd, retry.notified()).await?;
                    continue;
                };
                match owned_run(cfg, out, sd, owned, cmd, addr, &sig.restart).await? {
                    // Asked for: start again at once, it is not a crash.
                    RunEnd::Restart => restart.reset(),
                    RunEnd::Ended { connected_for, why } => {
                        match restart.run_ended(connected_for) {
                            Verdict::Retry { attempt, after } => {
                                out.client.send_replace(None);
                                out.state(State::Reconnecting {
                                    attempt,
                                    retry_in: after,
                                });
                                // retry() skips the wait
                                guard(sd, async {
                                    tokio::select! {
                                        _ = tokio::time::sleep(after) => {}
                                        _ = retry.notified() => {}
                                    }
                                })
                                .await?;
                            }
                            Verdict::GiveUp { failures } => {
                                let kind = if why.timed_out {
                                    FailureKind::StartTimeout
                                } else {
                                    FailureKind::CrashLoop
                                };
                                fail(
                                kind,
                                format!(
                                    "OpenRGB stopped {failures} times in a row and was not restarted again. Last: {}{}",
                                    why.text,
                                    if why.stderr.is_empty() { String::new() } else { format!("\n{}", why.stderr) }
                                ),
                            );
                                guard(sd, retry.notified()).await?;
                                restart.reset();
                            }
                        }
                    }
                    RunEnd::LaunchFailed(e) => {
                        fail(
                            FailureKind::LaunchFailed,
                            format!("Could not start OpenRGB ({}): {e}", cmd.program.display()),
                        );
                        guard(sd, retry.notified()).await?;
                    }
                }
            }
            Probe::TooOld(v) => {
                fail(
                    FailureKind::UnsupportedVersion,
                    format!("The OpenRGB server on port {} speaks protocol {v}, but protocol 6 (OpenRGB 1.0 or newer) is needed. Update OpenRGB.", cfg.port),
                );
                guard(sd, retry.notified()).await?;
            }
            Probe::NotOpenRgb => {
                fail(
                    FailureKind::PortInUse,
                    format!("Port {} is in use by a program that is not OpenRGB 1.0 or newer. Close it, or change the OpenRGB server port.", cfg.port),
                );
                guard(sd, retry.notified()).await?;
            }
            Probe::Unsettled(msg) => {
                unsettled += 1;
                if unsettled >= 3 {
                    fail(
                        FailureKind::PortInUse,
                        format!("The program on port {} keeps dropping the connection during the OpenRGB handshake ({msg}).", cfg.port),
                    );
                    guard(sd, retry.notified()).await?;
                    unsettled = 0;
                } else {
                    guard(sd, tokio::time::sleep(cfg.ready_poll)).await?;
                }
            }
        }
    }
}

struct EndReason {
    text: String,
    stderr: String,
    timed_out: bool,
}

enum RunEnd {
    Ended {
        connected_for: Option<Duration>,
        why: EndReason,
    },
    LaunchFailed(std::io::Error),
    Restart,
}

/// Launch, wait for readiness, serve until the connection or the process ends, then make
/// sure the process is gone. Never leaves a running child in `owned` on return.
async fn owned_run(
    cfg: &Config,
    out: &Out,
    sd: &mut watch::Receiver<bool>,
    owned: &mut Option<OwnedServer>,
    cmd: &ServerCommand,
    addr: SocketAddr,
    restart: &Notify,
) -> Flow<RunEnd> {
    out.state(State::Starting { owned: true });
    let server = match OwnedServer::spawn(cmd, addr.port()) {
        Ok(s) => s,
        Err(e) => return Ok(RunEnd::LaunchFailed(e)),
    };
    out.set_pid(Some(server.pid()));
    let child = owned.insert(server);

    // Readiness: poll the handshake until it works, the child dies, or time runs out.
    let deadline = Instant::now() + cfg.ready_timeout;
    let client = loop {
        if Instant::now() >= deadline {
            return Ok(end_owned(cfg, out, owned, "did not answer in time".into(), true).await);
        }
        let step = async {
            tokio::select! {
                code = child.exited() => Err(code),
                p = probe(addr, &cfg.connect, cfg.probe_timeout) => Ok(p),
            }
        };
        match guard(sd, step).await? {
            Err(code) => {
                return Ok(end_owned(
                    cfg,
                    out,
                    owned,
                    format!("exited during startup ({code})"),
                    false,
                )
                .await)
            }
            Ok(Probe::Server(c)) => break c,
            // Wrong version / not OpenRGB from our own child: it is broken, count it as a crash.
            Ok(Probe::TooOld(v)) => {
                return Ok(end_owned(
                    cfg,
                    out,
                    owned,
                    format!("the bundled server speaks protocol {v}"),
                    false,
                )
                .await)
            }
            Ok(Probe::NotOpenRgb) => {
                return Ok(end_owned(
                    cfg,
                    out,
                    owned,
                    "the port is held by something that is not OpenRGB".into(),
                    false,
                )
                .await)
            }
            Ok(Probe::Closed | Probe::Unsettled(_)) => {
                guard(sd, tokio::time::sleep(cfg.ready_poll)).await?
            }
        }
    };

    let since = Instant::now();
    let child = owned.as_mut().expect("just inserted");
    let reason = connected(out, sd, client, true, Some((child, restart))).await;
    // Shutdown propagates after cleanup in `run`; otherwise it is a crash, a drop or a restart.
    if reason? == Ended::Restart {
        end_owned(cfg, out, owned, "restarting".into(), false).await;
        return Ok(RunEnd::Restart);
    }
    let text = "connection lost".to_string();
    let mut end = end_owned(cfg, out, owned, text, false).await;
    if let RunEnd::Ended { connected_for, .. } = &mut end {
        *connected_for = Some(since.elapsed());
    }
    Ok(end)
}

/// The owned process is done for (or being declared so): reap it and describe why.
async fn end_owned(
    cfg: &Config,
    out: &Out,
    owned: &mut Option<OwnedServer>,
    text: String,
    timed_out: bool,
) -> RunEnd {
    let mut stderr = String::new();
    if let Some(mut o) = owned.take() {
        o.stop(cfg.stop_grace).await;
        stderr = o.stderr_tail();
    }
    out.set_pid(None);
    RunEnd::Ended {
        connected_for: None,
        why: EndReason {
            text,
            stderr,
            timed_out,
        },
    }
}

#[derive(PartialEq)]
enum Ended {
    Lost,
    Restart,
}

/// Publish a ready connection and block until it drops, the owned process exits, or a
/// restart of it is asked for.
async fn connected(
    out: &Out,
    sd: &mut watch::Receiver<bool>,
    client: Client,
    is_owned: bool,
    child: Option<(&mut OwnedServer, &Notify)>,
) -> Flow<Ended> {
    let mut events = client.events();
    out.client.send_replace(Some(client.clone()));
    out.state(State::Connected {
        owned: is_owned,
        server_name: client.server_name().to_string(),
    });
    let lost = async {
        loop {
            if !client.is_connected() {
                break;
            }
            match events.recv().await {
                Ok(Event::Disconnected) | Err(RecvError::Closed) => break,
                _ => {}
            }
        }
    };
    let res = match child {
        Some((c, r)) => {
            guard(sd, async {
                tokio::select! {
                    _ = lost => Ended::Lost,
                    _ = c.exited() => Ended::Lost,
                    _ = r.notified() => Ended::Restart,
                }
            })
            .await
        }
        None => {
            guard(sd, async {
                lost.await;
                Ended::Lost
            })
            .await
        }
    };
    out.client.send_replace(None);
    res
}
