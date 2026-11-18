// SPDX-License-Identifier: GPL-2.0-or-later
//! The About sheet's facts and links. The tray can ask for it while the window is closed, so
//! the request waits until the new window is ready to take it.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// The OpenRGB release the Windows installer bundles (tools/ci/fetch-openrgb.sh).
const BUNDLED_OPENRGB: &str = "1.1";
const BUNDLED_OPENRGB_COMMIT: &str = "8e16de27858cf2be6af54a33ce003df74cb9fd3e";

/// The only pages the window can open: it names one, never a URL.
fn link(id: &str) -> Option<String> {
    Some(match id {
        "source" => "https://github.com/pwalda/freergb".into(),
        "issues" => "https://github.com/pwalda/freergb/issues".into(),
        "releases" => "https://github.com/pwalda/freergb/releases".into(),
        "sponsor" => "https://github.com/sponsors/pwalda".into(),
        "kofi" => "https://ko-fi.com/pwalda".into(),
        "openrgb" => "https://openrgb.org".into(),
        "openrgb-source" => {
            format!("https://codeberg.org/OpenRGB/OpenRGB/src/commit/{BUNDLED_OPENRGB_COMMIT}")
        }
        _ => return None,
    })
}

#[derive(Default)]
pub struct AboutRequest(AtomicBool);

#[derive(Serialize)]
pub struct AboutView {
    version: String,
    /// The bundled OpenRGB's version, when this install has one (Windows).
    bundled_openrgb: Option<&'static str>,
}

/// From the tray: open the window and its About sheet.
pub fn request(handle: &AppHandle) {
    handle
        .state::<AboutRequest>()
        .0
        .store(true, Ordering::SeqCst);
    // A window already open takes it now; a new one asks with `take_about_request`.
    let _ = handle.emit("about", ());
}

#[tauri::command]
pub fn take_about_request(req: State<'_, AboutRequest>) -> bool {
    req.0.swap(false, Ordering::SeqCst)
}

#[tauri::command]
pub fn about(handle: AppHandle) -> AboutView {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| crate::shell::bundled_openrgb(&exe));
    AboutView {
        version: handle.package_info().version.to_string(),
        bundled_openrgb: bundled.map(|_| BUNDLED_OPENRGB),
    }
}

#[tauri::command]
pub fn open_link(handle: AppHandle, id: String) -> Result<(), String> {
    let url = link(&id).ok_or_else(|| format!("Unknown link: {id}"))?;
    handle
        .opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Could not open the browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_version_is_the_one_the_installer_fetches() {
        // Whole lines: a Windows checkout may end them with \r\n.
        let script: Vec<&str> = include_str!("../../../tools/ci/fetch-openrgb.sh")
            .lines()
            .map(str::trim_end)
            .collect();
        assert!(script.contains(&format!("tag=release_{BUNDLED_OPENRGB}").as_str()));
        assert!(script.contains(&format!("commit={BUNDLED_OPENRGB_COMMIT}").as_str()));
    }

    #[test]
    fn only_known_pages_open() {
        assert_eq!(
            link("sponsor").as_deref(),
            Some("https://github.com/sponsors/pwalda")
        );
        assert_eq!(link("kofi").as_deref(), Some("https://ko-fi.com/pwalda"));
        assert!(link("openrgb-source")
            .unwrap()
            .ends_with(BUNDLED_OPENRGB_COMMIT));
        assert_eq!(link("https://example.com"), None);
    }
}
