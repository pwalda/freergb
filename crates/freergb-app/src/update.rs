// SPDX-License-Identifier: GPL-2.0-or-later
//! In-app updates: release builds read a signed latest.json (published on the repository's
//! `updates` branch), tell the window when a newer version is out, and install it only when the
//! user says so.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Update>>,
    installing: AtomicBool,
    /// Checks turned back on: look now rather than at the next daily tick.
    resumed: tokio::sync::Notify,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateView {
    pub version: String,
    /// What's new, as plain lines.
    pub notes: Option<String>,
}

impl UpdateView {
    fn of(u: &Update) -> Self {
        let notes = u
            .body
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .map(String::from);
        UpdateView {
            version: u.version.clone(),
            notes,
        }
    }
}

/// Download progress in whole percent, reported only when it moves.
#[derive(Default)]
struct Progress {
    done: u64,
    shown: Option<u8>,
}

impl Progress {
    fn advance(&mut self, chunk: usize, total: Option<u64>) -> Option<u8> {
        self.done += chunk as u64;
        let total = total.filter(|t| *t > 0)?;
        let pct = (self.done.min(total) * 100 / total) as u8;
        (self.shown != Some(pct)).then(|| {
            self.shown = Some(pct);
            pct
        })
    }
}

/// Checks now and then daily, unless the user turned checks off. Debug builds do not check:
/// their version is not a release; nor does the Store build, which the Store updates.
pub fn spawn(handle: AppHandle) {
    if cfg!(debug_assertions) || crate::package::packaged() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let updates = handle.state::<Updates>();
        loop {
            if crate::shell::update_checks_on(&handle) {
                check(&handle).await;
            }
            tokio::select! {
                _ = tokio::time::sleep(CHECK_EVERY) => {}
                _ = updates.resumed.notified() => {}
            }
        }
    });
}

/// Checks were turned on again.
pub fn resume(handle: &AppHandle) {
    handle.state::<Updates>().resumed.notify_one();
}

async fn check(handle: &AppHandle) {
    let found = match handle.updater() {
        Ok(u) => u.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => {
            let view = UpdateView::of(&update);
            *lock(&handle.state::<Updates>().pending) = Some(update);
            let _ = handle.emit("update", &view);
            crate::shell::tray_changed(handle);
        }
        Ok(None) => {}
        // Offline or the feed is down: the next daily check tries again.
        Err(e) => eprintln!("freergb: update check failed: {e}"),
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The tray's update entry: absent with no update, disabled while it installs.
pub fn tray_item(handle: &AppHandle) -> Option<(String, bool)> {
    let updates = handle.state::<Updates>();
    let version = lock(&updates.pending).as_ref().map(|u| u.version.clone());
    tray_label(
        version.as_deref(),
        updates.installing.load(Ordering::SeqCst),
    )
}

fn tray_label(version: Option<&str>, installing: bool) -> Option<(String, bool)> {
    let version = version?;
    Some(if installing {
        ("Installing update…".into(), false)
    } else {
        (format!("Install freeRGB {version}"), true)
    })
}

/// The tray's "Install freeRGB X": the same install as Settings. A failure shows in the window.
pub fn install_from_tray(handle: &AppHandle) {
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = install_update(handle.clone()).await {
            eprintln!("freergb: {e}");
            let _ = handle.emit("notice", e);
        }
    });
}

#[tauri::command]
pub fn update_status(updates: State<'_, Updates>) -> Option<UpdateView> {
    lock(&updates.pending).as_ref().map(UpdateView::of)
}

/// Downloads with "update-progress" events, then installs. The app restarts into the new
/// version; on Windows the installer closes it and starts it again.
#[tauri::command]
pub async fn install_update(handle: AppHandle) -> Result<(), String> {
    let updates = handle.state::<Updates>();
    let Some(update) = lock(&updates.pending).clone() else {
        return Err("There is no update to install.".into());
    };
    if updates.installing.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    crate::shell::tray_changed(&handle);
    let result = download_and_install(&handle, &update).await;
    handle
        .state::<Updates>()
        .installing
        .store(false, Ordering::SeqCst);
    crate::shell::tray_changed(&handle);
    result
}

async fn download_and_install(handle: &AppHandle, update: &Update) -> Result<(), String> {
    let mut progress = Progress::default();
    let bytes = update
        .download(
            |chunk, total| {
                if let Some(pct) = progress.advance(chunk, total) {
                    let _ = handle.emit("update-progress", pct);
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("The update could not be downloaded: {e}"))?;
    // The installer replaces files OpenRGB and our lights use: hand devices back first.
    #[cfg(windows)]
    {
        crate::shell::shut_down(handle).await;
        // Only returns on failure; the update did not happen, so come back as we were.
        if let Err(e) = update.install(&bytes) {
            eprintln!("freergb: the update could not be installed: {e}");
        }
        handle.restart();
    }
    #[cfg(not(windows))]
    {
        // Linux replaces files in place (a cancelled pkexec prompt fails here, harmlessly).
        update
            .install(&bytes)
            .map_err(|e| format!("The update could not be installed: {e}"))?;
        crate::shell::shut_down(handle).await;
        handle.restart();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_reported_once_per_percent() {
        let mut p = Progress::default();
        assert_eq!(p.advance(10, Some(1000)), Some(1));
        assert_eq!(p.advance(1, Some(1000)), None, "still 1%");
        assert_eq!(p.advance(489, Some(1000)), Some(50));
        assert_eq!(p.advance(600, Some(1000)), Some(100), "never past 100");
        assert_eq!(p.advance(10, Some(1000)), None);
    }

    #[test]
    fn the_tray_offers_the_update_until_it_installs() {
        assert_eq!(tray_label(None, false), None);
        assert_eq!(
            tray_label(Some("0.1.0-beta.2"), false),
            Some(("Install freeRGB 0.1.0-beta.2".into(), true))
        );
        assert_eq!(
            tray_label(Some("0.1.0-beta.2"), true),
            Some(("Installing update…".into(), false))
        );
    }

    #[test]
    fn progress_without_a_size_stays_quiet() {
        let mut p = Progress::default();
        assert_eq!(p.advance(10, None), None);
        assert_eq!(p.advance(10, Some(0)), None);
    }
}
