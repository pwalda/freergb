// SPDX-License-Identifier: GPL-2.0-or-later
//! The sensor service (Windows). Installed by freeRGB's installer as `freeRGB-Sensors`,
//! running as SYSTEM: it loads one PawnIO module (IntelMSR or AMDFamily17, which refuse the
//! other vendor's CPU), and answers every connection to its pipe with the CPU temperature.
//! It reads nothing else and takes no input. `--console` runs it in a terminal for testing.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
fn main() {
    if std::env::args().any(|a| a == "--console") {
        win::console();
    } else if let Err(e) = win::dispatch() {
        eprintln!("freergb-sensord: run it as a service, or with --console ({e})");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("freergb-sensord is for Windows; on Linux the kernel gives the CPU temperature.");
    std::process::exit(1);
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod win {
    use freergb_sensord::{amd_offset, amd_tctl, intel_temp, Reading, PIPE, SERVICE};
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FlushFileBuffers, WriteFile, PIPE_ACCESS_OUTBOUND,
    };
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    static INTEL_MSR: &[u8] = include_bytes!("../modules/IntelMSR.bin");
    static AMD_FAMILY_17: &[u8] = include_bytes!("../modules/AMDFamily17.bin");

    /// A missing PawnIO (or a CPU no module takes) is tried again this often: the user may
    /// install PawnIO later.
    const RETRY: Duration = Duration::from_secs(30);

    const MSR_TEMPERATURE_TARGET: u64 = 0x1A2;
    const MSR_PACKAGE_THERM_STATUS: u64 = 0x1B1;
    const MSR_THERM_STATUS: u64 = 0x19C;
    const SMN_THM_TCON_CUR_TMP: u64 = 0x0005_9800;

    define_windows_service!(ffi_main, service_main);

    pub fn dispatch() -> windows_service::Result<()> {
        service_dispatcher::start(SERVICE, ffi_main)
    }

    pub fn console() {
        let mut cpu = Cpu::default();
        loop {
            print!("{}", cpu.reading().line());
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn service_main(_args: Vec<std::ffi::OsString>) {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let handler = move |c| match c {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stopping.store(true, Ordering::SeqCst);
                // Wake the pipe server waiting for a connection.
                let _ = std::fs::File::open(PIPE);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let Ok(status) = service_control_handler::register(SERVICE, handler) else {
            return;
        };
        let report = |state, accept| {
            let _ = status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: accept,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(5),
                process_id: None,
            });
        };
        report(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        );
        serve(&stop);
        report(ServiceState::Stopped, ServiceControlAccept::empty());
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    /// One client at a time, one line each: the app asks about once a second.
    fn serve(stop: &AtomicBool) {
        let name = wide(PIPE);
        let mut cpu = Cpu::default();
        while !stop.load(Ordering::SeqCst) {
            let pipe = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_OUTBOUND,
                    PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    PIPE_UNLIMITED_INSTANCES,
                    512,
                    0,
                    0,
                    std::ptr::null(),
                )
            };
            if pipe == INVALID_HANDLE_VALUE {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0
                || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if connected && !stop.load(Ordering::SeqCst) {
                let line = cpu.reading().line();
                let mut written = 0u32;
                unsafe {
                    WriteFile(
                        pipe,
                        line.as_ptr(),
                        line.len() as u32,
                        &mut written,
                        std::ptr::null_mut(),
                    );
                    FlushFileBuffers(pipe);
                }
            }
            // Close without DisconnectNamedPipe: the client then sees a broken pipe (an
            // end of file) after the line, where a disconnect fails its read instead.
            unsafe { CloseHandle(pipe) };
        }
    }

    type FnOpen = unsafe extern "system" fn(*mut HANDLE) -> i32;
    type FnLoad = unsafe extern "system" fn(HANDLE, *const u8, usize) -> i32;
    type FnExecute = unsafe extern "system" fn(
        HANDLE,
        *const u8,
        *const u64,
        usize,
        *mut u64,
        usize,
        *mut usize,
    ) -> i32;
    /// What GetProcAddress gives.
    type Proc = unsafe extern "system" fn() -> isize;
    type FnClose = unsafe extern "system" fn(HANDLE) -> i32;

    /// PawnIOLib's entry points, from PawnIO's own install (never a copy beside us).
    struct Lib {
        open: FnOpen,
        load: FnLoad,
        execute: FnExecute,
        close: FnClose,
    }

    impl Lib {
        fn find() -> Option<Lib> {
            let dir = install_location().or_else(|| {
                std::env::var("ProgramFiles")
                    .ok()
                    .map(|p| format!(r"{p}\PawnIO\"))
            })?;
            let path = wide(&format!(r"{}\PawnIOLib.dll", dir.trim_end_matches('\\')));
            let module = unsafe {
                LoadLibraryExW(
                    path.as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_WITH_ALTERED_SEARCH_PATH,
                )
            };
            if module.is_null() {
                return None;
            }
            let get = |name: &[u8]| unsafe { GetProcAddress(module, name.as_ptr()) };
            // SAFETY: the signatures are PawnIOLib.h's (HRESULT, stdcall = system on x64).
            unsafe {
                Some(Lib {
                    open: std::mem::transmute::<Proc, FnOpen>(get(b"pawnio_open\0")?),
                    load: std::mem::transmute::<Proc, FnLoad>(get(b"pawnio_load\0")?),
                    execute: std::mem::transmute::<Proc, FnExecute>(get(b"pawnio_execute\0")?),
                    close: std::mem::transmute::<Proc, FnClose>(get(b"pawnio_close\0")?),
                })
            }
        }

        /// A handle with `blob` loaded, or None when the module refuses this CPU.
        fn module(&self, blob: &[u8]) -> Option<HANDLE> {
            let mut h: HANDLE = std::ptr::null_mut();
            if unsafe { (self.open)(&mut h) } < 0 {
                return None;
            }
            if unsafe { (self.load)(h, blob.as_ptr(), blob.len()) } < 0 {
                unsafe { (self.close)(h) };
                return None;
            }
            Some(h)
        }

        fn call(&self, h: HANDLE, ioctl: &[u8], arg: u64) -> Option<u64> {
            let mut out = 0u64;
            let mut got = 0usize;
            let hr = unsafe { (self.execute)(h, ioctl.as_ptr(), &arg, 1, &mut out, 1, &mut got) };
            (hr >= 0 && got == 1).then_some(out)
        }
    }

    fn reg_string(key: &str, value: &str) -> Option<String> {
        let (key, value) = (wide(key), wide(value));
        let mut buf = vec![0u16; 512];
        let mut size = (buf.len() * 2) as u32;
        let err = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut c_void,
                &mut size,
            )
        };
        if err != 0 {
            return None;
        }
        buf.truncate((size as usize / 2).saturating_sub(1));
        Some(String::from_utf16_lossy(&buf))
    }

    fn install_location() -> Option<String> {
        reg_string(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO",
            "InstallLocation",
        )
        .filter(|s| !s.is_empty())
    }

    enum Kind {
        Intel,
        Amd { offset: f64 },
    }

    struct Session {
        lib: Lib,
        handle: HANDLE,
        kind: Kind,
    }

    #[derive(Default)]
    struct Cpu {
        open: Option<Session>,
        problem: Option<&'static str>,
        tried: Option<Instant>,
    }

    impl Cpu {
        fn reading(&mut self) -> Reading {
            if self.open.is_none() && self.tried.map_or(true, |t| t.elapsed() >= RETRY) {
                self.tried = Some(Instant::now());
                match Self::start() {
                    Ok(o) => {
                        self.open = Some(o);
                        self.problem = None;
                    }
                    Err(p) => self.problem = Some(p),
                }
            }
            let temp = self.open.as_ref().and_then(Self::read);
            if temp.is_none() && self.open.is_some() {
                self.problem = Some("The CPU did not give a temperature.");
            }
            Reading {
                cpu_temp: temp,
                problem: temp
                    .is_none()
                    .then(|| self.problem.unwrap_or("Not read yet.").into()),
            }
        }

        fn start() -> Result<Session, &'static str> {
            let lib = Lib::find().ok_or("PawnIO is not installed.")?;
            if let Some(handle) = lib.module(INTEL_MSR) {
                return Ok(Session {
                    lib,
                    handle,
                    kind: Kind::Intel,
                });
            }
            if let Some(handle) = lib.module(AMD_FAMILY_17) {
                let brand = reg_string(
                    r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
                    "ProcessorNameString",
                )
                .unwrap_or_default();
                let offset = amd_offset(&brand);
                return Ok(Session {
                    lib,
                    handle,
                    kind: Kind::Amd { offset },
                });
            }
            Err("This CPU is not supported yet.")
        }

        fn read(o: &Session) -> Option<f64> {
            match o.kind {
                Kind::Intel => {
                    let target =
                        o.lib
                            .call(o.handle, b"ioctl_read_msr\0", MSR_TEMPERATURE_TARGET)?;
                    let package = o
                        .lib
                        .call(o.handle, b"ioctl_read_msr\0", MSR_PACKAGE_THERM_STATUS)
                        .and_then(|s| intel_temp(target, s));
                    package.or_else(|| {
                        let core = o
                            .lib
                            .call(o.handle, b"ioctl_read_msr\0", MSR_THERM_STATUS)?;
                        intel_temp(target, core)
                    })
                }
                Kind::Amd { offset } => {
                    let reg = o
                        .lib
                        .call(o.handle, b"ioctl_read_smn\0", SMN_THM_TCON_CUR_TMP)?;
                    Some(amd_tctl(reg as u32) - offset)
                }
            }
        }
    }
}
