// SPDX-License-Identifier: GPL-2.0-or-later
//! The Microsoft Store build: freeRGB running from its MSIX package. The Store updates it, so
//! the app's own updater stays off, and starting at login is the package's startup task (a
//! Run key written from a package lands in the package's private registry, where Windows never
//! looks).

/// The startup task's id in the package manifest (`windows.startupTask`).
#[cfg_attr(not(windows), allow(dead_code))]
const STARTUP_TASK: &str = "freeRGBAtLogin";

/// What the startup task's state means for the "Start when I log in" switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum Startup {
    Off,
    On,
    /// Turned off in Windows (Settings > Apps > Startup or Task Manager): only Windows can turn
    /// it back on.
    OffByUser,
    OffByPolicy,
    OnByPolicy,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Startup {
    fn is_on(self) -> bool {
        matches!(self, Startup::On | Startup::OnByPolicy)
    }

    /// Why asking for `want` did not give it, in words for the window.
    fn refusal(self, want: bool) -> Option<String> {
        if self.is_on() == want {
            return None;
        }
        Some(
            match self {
                Startup::OffByUser => "Starting at login is turned off in Windows. Turn freeRGB on in Settings > Apps > Startup.",
                Startup::OffByPolicy | Startup::OnByPolicy => "Your organisation decides whether freeRGB starts at login.",
                Startup::Off | Startup::On => "Windows did not change starting at login.",
            }
            .into(),
        )
    }
}

#[cfg(windows)]
mod imp {
    use super::{Startup, STARTUP_TASK};
    use std::sync::OnceLock;
    use windows::core::HSTRING;
    use windows::ApplicationModel::Activation::ActivationKind;
    use windows::ApplicationModel::{AppInstance, Package, StartupTask, StartupTaskState};

    pub fn packaged() -> bool {
        static PACKAGED: OnceLock<bool> = OnceLock::new();
        *PACKAGED.get_or_init(|| Package::Current().is_ok())
    }

    pub fn started_at_login() -> bool {
        packaged()
            && AppInstance::GetActivatedEventArgs()
                .and_then(|a| a.Kind())
                .is_ok_and(|k| k == ActivationKind::StartupTask)
    }

    fn task() -> Result<StartupTask, String> {
        StartupTask::GetAsync(&HSTRING::from(STARTUP_TASK))
            .and_then(|op| op.join())
            .map_err(|e| format!("Windows did not find freeRGB's startup task: {e}"))
    }

    fn of(s: StartupTaskState) -> Startup {
        match s {
            StartupTaskState::Enabled => Startup::On,
            StartupTaskState::DisabledByUser => Startup::OffByUser,
            StartupTaskState::DisabledByPolicy => Startup::OffByPolicy,
            StartupTaskState::EnabledByPolicy => Startup::OnByPolicy,
            _ => Startup::Off,
        }
    }

    pub fn startup() -> Result<bool, String> {
        let s = task()?.State().map_err(|e| e.to_string())?;
        Ok(of(s).is_on())
    }

    /// Blocks on Windows' answer; call off the main thread.
    pub fn set_startup(on: bool) -> Result<bool, String> {
        let t = task()?;
        let now = if on {
            t.RequestEnableAsync().and_then(|op| op.join())
        } else {
            t.Disable().and_then(|()| t.State())
        }
        .map_err(|e| format!("Could not change starting at login: {e}"))?;
        match of(now).refusal(on) {
            Some(why) => Err(why),
            None => Ok(on),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn packaged() -> bool {
        false
    }
    pub fn started_at_login() -> bool {
        false
    }
    pub fn startup() -> Result<bool, String> {
        Err("No package on this system".into())
    }
    pub fn set_startup(_: bool) -> Result<bool, String> {
        Err("No package on this system".into())
    }
}

pub use imp::{packaged, set_startup, started_at_login, startup};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_reads_the_task_state_and_says_why_a_change_did_not_take() {
        assert!(Startup::On.is_on() && Startup::OnByPolicy.is_on());
        assert!(!Startup::Off.is_on() && !Startup::OffByUser.is_on());
        assert_eq!(Startup::On.refusal(true), None);
        assert_eq!(Startup::Off.refusal(false), None);
        assert!(Startup::OffByUser
            .refusal(true)
            .unwrap()
            .contains("Settings > Apps > Startup"));
        assert!(Startup::OnByPolicy
            .refusal(false)
            .unwrap()
            .contains("organisation"));
        assert!(Startup::Off.refusal(true).is_some());
    }

    #[test]
    fn a_plain_build_is_not_packaged() {
        assert!(!packaged() && !started_at_login());
    }
}
