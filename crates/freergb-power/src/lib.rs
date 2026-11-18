// SPDX-License-Identifier: GPL-2.0-or-later
//! When the PC is about to sleep and when it wakes.
//!
//! * Windows: `PowerRegisterSuspendResumeNotification` with a callback (no window needed, so it
//!   works while freeRGB sits in the tray with its window closed).
//! * Linux: logind's `PrepareForSleep` signal, holding a delay lock while awake so the handler
//!   runs before the system sleeps (logind waits up to its `InhibitDelayMaxSec`, 5 s by default).
//!
//! The handler runs on the notifying thread and sleep waits for it to return, so it should be
//! quick: switch the lights and give the frame a moment to go out.

/// A change in the PC's power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// About to sleep or hibernate.
    Sleep,
    /// Back from sleep or hibernation.
    Wake,
}

/// Call `handler` on every sleep and wake for the rest of the process. Whether this machine
/// tells (false without logind, or if Windows refuses the registration).
pub fn watch(handler: impl Fn(Power) + Send + Sync + 'static) -> bool {
    #[cfg(windows)]
    return windows::watch(Box::new(handler));
    #[cfg(target_os = "linux")]
    return linux::watch(Box::new(handler));
    #[allow(unreachable_code)]
    {
        drop(handler);
        false
    }
}

type Handler = Box<dyn Fn(Power) + Send + Sync>;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use super::{Handler, Power};
    use std::ffi::c_void;
    use windows_sys::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND,
    };

    /// The power event a broadcast code stands for. Waking sends both "automatic" (always)
    /// and "suspend" (once a person is present); only the first counts.
    pub(crate) fn event(code: u32) -> Option<Power> {
        match code {
            PBT_APMSUSPEND => Some(Power::Sleep),
            PBT_APMRESUMEAUTOMATIC => Some(Power::Wake),
            _ => None,
        }
    }

    unsafe extern "system" fn notify(context: *const c_void, code: u32, _: *const c_void) -> u32 {
        if let Some(p) = event(code) {
            // SAFETY: `context` is the leaked `Handler` given at registration; it lives for the
            // rest of the process and is only read.
            let handler = unsafe { &*(context as *const Handler) };
            handler(p);
        }
        0
    }

    pub(crate) fn watch(handler: Handler) -> bool {
        // Both stay registered until the process ends, so they are leaked on purpose.
        let context: &'static mut Handler = Box::leak(Box::new(handler));
        let params: &'static mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS =
            Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
                Callback: Some(notify),
                Context: context as *mut Handler as *mut c_void,
            }));
        let mut registration: *mut c_void = std::ptr::null_mut();
        // SAFETY: with DEVICE_NOTIFY_CALLBACK the recipient is a pointer to the subscribe
        // parameters, which outlive the registration (leaked above).
        let err = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                params as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS as *mut c_void,
                &mut registration,
            )
        };
        err == 0
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn only_suspend_and_the_automatic_resume_count() {
            assert_eq!(event(PBT_APMSUSPEND), Some(Power::Sleep));
            assert_eq!(event(PBT_APMRESUMEAUTOMATIC), Some(Power::Wake));
            assert_eq!(
                event(windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMESUSPEND),
                None
            );
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{Handler, Power};
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::OwnedFd;

    fn manager(conn: &Connection) -> zbus::Result<Proxy<'static>> {
        Proxy::new(
            conn,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
    }

    /// A delay lock: logind waits for it to be dropped (or its delay limit) before sleeping.
    fn delay(m: &Proxy<'_>) -> Option<OwnedFd> {
        m.call(
            "Inhibit",
            &(
                "sleep",
                "freeRGB",
                "Turns the lights off or switches profile before sleep",
                "delay",
            ),
        )
        .ok()
    }

    pub(crate) fn watch(handler: Handler) -> bool {
        let Ok(conn) = Connection::system() else {
            return false;
        };
        let Ok(m) = manager(&conn) else {
            return false;
        };
        let Ok(signals) = m.receive_signal("PrepareForSleep") else {
            return false;
        };
        std::thread::Builder::new()
            .name("freergb-power".into())
            .spawn(move || {
                let mut lock = delay(&m);
                for msg in signals {
                    let Ok(sleeping) = msg.body().deserialize::<bool>() else {
                        continue;
                    };
                    if sleeping {
                        handler(Power::Sleep);
                        lock = None;
                    } else {
                        lock = lock.or_else(|| delay(&m));
                        handler(Power::Wake);
                    }
                }
                drop(lock);
            })
            .is_ok()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    /// Opt-in (needs logind): the watch starts and holds a delay lock, as
    /// `systemd-inhibit --list` shows (`cargo test -p freergb-power -- --ignored`).
    #[test]
    #[ignore]
    fn logind_holds_our_delay_lock() {
        assert!(super::watch(|p| println!("{p:?}")));
        std::thread::sleep(std::time::Duration::from_millis(500));
        let list = std::process::Command::new("systemd-inhibit")
            .arg("--list")
            .output()
            .expect("systemd-inhibit runs");
        let text = String::from_utf8_lossy(&list.stdout);
        assert!(text.contains("freeRGB"), "{text}");
    }
}
