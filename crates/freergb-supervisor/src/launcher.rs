// SPDX-License-Identifier: GPL-2.0-or-later
//! Running (and stopping) a server process this supervisor owns.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// How to start the server. `{port}` in an argument is replaced by the port to listen on.
#[derive(Debug, Clone)]
pub struct ServerCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(OsString, OsString)>,
}

impl ServerCommand {
    /// Headless OpenRGB: `--server` without `--gui` runs no window (verified on 1.1 with no
    /// display at all). `--noautoconnect` stops a fresh instance from attaching as a client
    /// to another local server in the gap between our probe and our launch; it must stay.
    pub fn openrgb(program: impl Into<PathBuf>, config_dir: Option<PathBuf>) -> Self {
        let mut args = vec![
            "--server".into(),
            "--noautoconnect".into(),
            "--server-port".into(),
            "{port}".into(),
        ];
        if let Some(dir) = config_dir {
            args.push("--config".into());
            args.push(dir.to_string_lossy().into_owned());
        }
        Self {
            program: program.into(),
            args,
            env: vec![],
        }
    }
}

const TAIL_LINES: usize = 20;

/// A running server we started. Dropping it kills the process (`kill_on_drop`), so a
/// supervisor that is dropped or panics does not leave a server behind.
pub struct OwnedServer {
    child: Child,
    pid: u32,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stderr_task: Option<tokio::task::JoinHandle<()>>,
    stopped: bool,
}

impl OwnedServer {
    pub fn spawn(cmd: &ServerCommand, port: u16) -> io::Result<Self> {
        let mut c = Command::new(&cmd.program);
        for a in &cmd.args {
            c.arg(a.replace("{port}", &port.to_string()));
        }
        c.envs(cmd.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Own process group, so stopping covers wrappers (an AppImage's AppRun launches
        // OpenRGB as a grandchild) and nothing else on the user's machine.
        #[cfg(unix)]
        c.process_group(0);
        let mut child = c.spawn()?;
        let pid = child.id().unwrap_or(0);
        let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
        let mut stderr_task = None;
        if let Some(err) = child.stderr.take() {
            let tail = stderr_tail.clone();
            stderr_task = Some(tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let mut t = tail.lock().unwrap_or_else(|e| e.into_inner());
                    if t.len() == TAIL_LINES {
                        t.pop_front();
                    }
                    t.push_back(l);
                }
            }));
        }
        Ok(Self {
            child,
            pid,
            stderr_tail,
            stderr_task,
            stopped: false,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Resolves with the exit description when the process ends.
    pub async fn exited(&mut self) -> String {
        match self.child.wait().await {
            Ok(s) => s.to_string(),
            Err(e) => format!("wait failed: {e}"),
        }
    }

    /// Last stderr lines. Call after `stop`, which lets the reader drain the pipe.
    pub fn stderr_tail(&self) -> String {
        let t = self.stderr_tail.lock().unwrap_or_else(|e| e.into_inner());
        t.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    /// Ask the process (group) to exit, then force it after `grace`. Always reaps.
    pub async fn stop(&mut self, grace: Duration) {
        self.stopped = true;
        #[cfg(unix)]
        {
            // `kill` is used instead of libc: the workspace forbids unsafe code.
            signal_group("-TERM", self.pid).await;
            if tokio::time::timeout(grace, self.child.wait()).await.is_ok() {
                // Wrapper gone; make sure no grandchild outlived it.
                signal_group("-KILL", self.pid).await;
                self.drain_stderr().await;
                return;
            }
            signal_group("-KILL", self.pid).await;
        }
        #[cfg(not(unix))]
        let _ = grace;
        let _ = self.child.kill().await;
        self.drain_stderr().await;
    }

    /// The pipe closes when every holder is gone; bounded in case something inherited it.
    async fn drain_stderr(&mut self) {
        if let Some(t) = self.stderr_task.take() {
            let _ = tokio::time::timeout(Duration::from_millis(500), t).await;
        }
    }
}

#[cfg(unix)]
async fn signal_group(sig: &str, pgid: u32) {
    if pgid == 0 {
        return;
    }
    let _ = Command::new("kill")
        .arg(sig)
        .arg("--")
        .arg(format!("-{pgid}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

impl Drop for OwnedServer {
    /// Dropped without `stop` (panic, abort, process exit path that skipped shutdown): the
    /// `kill_on_drop` flag kills only the direct child, which would leave a wrapped OpenRGB
    /// (an AppImage's AppRun starts it as a grandchild) running and holding the port. Kill the
    /// whole group too, but only while the group leader is verifiably still ours and alive,
    /// so a reused PID can never make us signal somebody else's processes.
    fn drop(&mut self) {
        #[cfg(unix)]
        if !self.stopped && self.pid != 0 && matches!(self.child.try_wait(), Ok(None)) {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.pid)])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}
