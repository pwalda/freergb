// SPDX-License-Identifier: GPL-2.0-or-later
//! The screen's colours, as a small grid a look samples by each LED's place on the desk map.
//!
//! Captured through pinray: DXGI or Windows Graphics Capture on Windows, the desktop portal and
//! PipeWire on Wayland, X11 otherwise. Each frame shrinks to [`COLS`] x [`ROWS`] averages that
//! ease towards the new picture, so a cut softens instead of flashing. On Wayland the desktop
//! asks which screen to share; its answer is kept (the portal's restore token) so it asks once.

use crate::{Out, Running, Source, Stop};
use freergb_effects::react::{Screen, SourceId};
use freergb_effects::Rgb;
use pinray::{CaptureEvent, CaptureSession, CursorMode, FrameData, PinrayError, PixelFormat};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

pub const COLS: usize = 32;
pub const ROWS: usize = 18;
/// Samples per cell side: enough to average a cell without reading every pixel.
const SAMPLES: usize = 8;
/// Frames asked of the capture; the lights run at 30 or 60, but colours eased over
/// [`EASE`] need no more.
const FPS: u32 = 20;
/// How long a colour takes to get most of the way (63%) to a new picture.
const EASE: Duration = Duration::from_millis(100);
const TICK: Duration = Duration::from_millis(33);
const RETRY: Duration = Duration::from_secs(2);

const ASKS: &str = "Your desktop asks which screen to share";

/// The screen source, if this machine has a screen to capture. `token` is where the Wayland
/// portal's answer is kept between runs.
pub fn source(token: Option<PathBuf>) -> Option<Box<dyn Source>> {
    #[cfg(target_os = "linux")]
    if !wayland() && std::env::var_os("DISPLAY").is_none() {
        return None;
    }
    Some(Box::new(ScreenSource { token }))
}

fn wayland() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

struct ScreenSource {
    token: Option<PathBuf>,
}

impl Source for ScreenSource {
    fn id(&self) -> SourceId {
        SourceId::Screen
    }

    fn needs(&self) -> Option<&'static str> {
        wayland().then_some(ASKS)
    }

    fn start(&self, out: Out, _rt: &Handle) -> Running {
        let token = self.token.clone();
        let live = Arc::new(AtomicBool::new(true));
        let alive = live.clone();
        let (stop, stopped) = mpsc::channel::<()>();
        let spawned = std::thread::Builder::new()
            .name("freergb-screen".into())
            .spawn(move || capture(token, out, &alive, stopped));
        drop(spawned);
        Running::guard(Stop(live, stop))
    }
}

/// The capture thread: opens the screen (again after a failure), eases the grid towards each
/// new frame and writes it while the source runs.
fn capture(token: Option<PathBuf>, out: Out, live: &AtomicBool, stopped: mpsc::Receiver<()>) {
    let mut session: Option<CaptureSession> = None;
    let mut grid = Grid::default();
    let mut target: Option<Vec<[f32; 3]>> = None;
    let mut retry_at = Instant::now();
    let mut last = Instant::now();
    loop {
        if !matches!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            return;
        }
        let now = Instant::now();
        if session.is_none() && now >= retry_at {
            match open(token.as_ref()) {
                Ok(s) => session = Some(s),
                // Cancelled in the portal dialog: asking again at once would nag; the next
                // start of a look that follows the screen asks again.
                Err(PinrayError::Unsupported(_)) if wayland() => {
                    let _ = stopped.recv();
                    return;
                }
                Err(_) => retry_at = now + RETRY,
            }
        }
        match session.as_mut().map(|s| s.next_event(Some(TICK))) {
            Some(Ok(CaptureEvent::Video(f))) => {
                if let FrameData::Host(px) = &f.data {
                    let bgr = match f.pixel_format {
                        PixelFormat::Bgra8888 => Some(true),
                        PixelFormat::Rgba8888 => Some(false),
                        _ => None,
                    };
                    target = bgr.and_then(|bgr| {
                        shrink(
                            px,
                            f.width as usize,
                            f.height as usize,
                            f.stride as usize,
                            bgr,
                        )
                    });
                }
            }
            Some(Ok(CaptureEvent::Audio(_) | CaptureEvent::Gap(_)))
            | Some(Err(PinrayError::Timeout(_))) => {}
            // Ended or failed (the display changed, the desktop locked): open it again.
            Some(Ok(CaptureEvent::End)) | Some(Err(_)) => {
                session = None;
                retry_at = now + RETRY;
            }
            None => std::thread::sleep(TICK.min(retry_at.saturating_duration_since(now))),
        }
        let now = Instant::now();
        let dt = now.duration_since(last);
        last = now;
        if let Some(t) = &target {
            if grid.ease(t, dt) {
                let screen = grid.screen();
                out.set(live, |r| r.screen = Some(screen));
            }
        }
    }
}

fn open(token: Option<&PathBuf>) -> Result<CaptureSession, PinrayError> {
    let saved = token.and_then(|p| std::fs::read_to_string(p).ok());
    let mut b = CaptureSession::builder()
        .video_target(pinray::VideoCaptureTarget::Display(pinray::SourceId::new(
            "auto",
        )))
        .cursor_mode(CursorMode::Hidden)
        .pixel_format(PixelFormat::Bgra8888)
        .frame_rate(Some(FPS))
        .queue_depth(1);
    if let Some(t) = saved.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        b = b.restore_token(t);
    }
    let mut s = b.build()?;
    if let (Some(path), Some(new)) = (token, s.restore_token()) {
        if saved.as_deref().map(str::trim) != Some(new.as_str()) {
            let _ = std::fs::write(path, new);
        }
    }
    s.start()?;
    Ok(s)
}

/// The average colour of each grid cell of a frame (`stride` bytes per row, four bytes per
/// pixel, blue first when `bgr`), sampled on a lattice inside each cell. `None` for a frame
/// smaller than the grid or with too few bytes.
pub fn shrink(
    px: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    bgr: bool,
) -> Option<Vec<[f32; 3]>> {
    if width < COLS
        || height < ROWS
        || stride < width * 4
        || px.len() < stride * (height - 1) + width * 4
    {
        return None;
    }
    let mut cells = Vec::with_capacity(COLS * ROWS);
    for row in 0..ROWS {
        for col in 0..COLS {
            let mut sum = [0u32; 3];
            for sy in 0..SAMPLES {
                // The centre of each of SAMPLES equal slices of the cell.
                let y = (row * SAMPLES + sy) * 2 + 1;
                let y = y * height / (ROWS * SAMPLES * 2);
                for sx in 0..SAMPLES {
                    let x = ((col * SAMPLES + sx) * 2 + 1) * width / (COLS * SAMPLES * 2);
                    let i = y * stride + x * 4;
                    sum[0] += u32::from(px[i]);
                    sum[1] += u32::from(px[i + 1]);
                    sum[2] += u32::from(px[i + 2]);
                }
            }
            let n = (SAMPLES * SAMPLES) as f32;
            let [a, g, c] = sum.map(|s| s as f32 / n);
            cells.push(if bgr { [c, g, a] } else { [a, g, c] });
        }
    }
    Some(cells)
}

/// The colours shown, easing towards the latest frame.
#[derive(Default)]
struct Grid {
    now: Vec<[f32; 3]>,
    shown: Vec<Rgb>,
}

impl Grid {
    /// Move towards `target` by `dt`; whether the shown colours changed.
    fn ease(&mut self, target: &[[f32; 3]], dt: Duration) -> bool {
        if self.now.len() != target.len() {
            // The first frame shows at once.
            self.now = target.to_vec();
        } else {
            let k = 1.0 - (-dt.as_secs_f32() / EASE.as_secs_f32()).exp();
            for (c, t) in self.now.iter_mut().zip(target) {
                for i in 0..3 {
                    c[i] += (t[i] - c[i]) * k;
                }
            }
        }
        let byte = |v: f32| v.round().clamp(0.0, 255.0) as u8;
        let shown: Vec<Rgb> = self
            .now
            .iter()
            .map(|c| Rgb {
                r: byte(c[0]),
                g: byte(c[1]),
                b: byte(c[2]),
            })
            .collect();
        let changed = shown != self.shown;
        self.shown = shown;
        changed
    }

    fn screen(&self) -> Screen {
        Screen {
            width: COLS as u32,
            height: ROWS as u32,
            colors: self.shown.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BGRA frame, left half red and right half blue, with row padding.
    fn halves(w: usize, h: usize, pad: usize) -> (Vec<u8>, usize) {
        let stride = w * 4 + pad;
        let mut px = vec![0u8; stride * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * stride + x * 4;
                let bgra = if x < w / 2 {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                };
                px[i..i + 4].copy_from_slice(&bgra);
            }
        }
        (px, stride)
    }

    #[test]
    fn a_frame_shrinks_to_the_average_of_each_cell() {
        let (px, stride) = halves(1920, 1080, 64);
        let g = shrink(&px, 1920, 1080, stride, true).unwrap();
        assert_eq!(g.len(), COLS * ROWS);
        assert_eq!(g[0], [255.0, 0.0, 0.0], "top left is red");
        assert_eq!(g[COLS - 1], [0.0, 0.0, 255.0], "top right is blue");
        assert_eq!(g[(ROWS - 1) * COLS + COLS / 2 - 1], [255.0, 0.0, 0.0]);
        assert_eq!(g[(ROWS - 1) * COLS + COLS / 2], [0.0, 0.0, 255.0]);

        // RGBA reads the other way round.
        let rgba = shrink(&px, 1920, 1080, stride, false).unwrap();
        assert_eq!(rgba[0], [0.0, 0.0, 255.0]);

        let (px, stride) = halves(COLS * 3, ROWS, 0);
        assert_eq!(shrink(&px, 8, 8, 32, true), None, "smaller than the grid");
        assert_eq!(
            shrink(&px[..100], COLS * 3, ROWS, stride, true),
            None,
            "too few bytes"
        );
    }

    #[test]
    fn the_colours_ease_towards_a_new_picture() {
        let mut g = Grid::default();
        let black = vec![[0.0; 3]; 4];
        let white = vec![[255.0; 3]; 4];
        assert!(g.ease(&black, TICK), "the first frame shows");
        assert_eq!(g.screen().colors[0], Rgb::BLACK);

        assert!(g.ease(&white, EASE));
        let r = g.screen().colors[0].r;
        assert!((150..=170).contains(&r), "63% of the way after EASE: {r}");
        for _ in 0..30 {
            g.ease(&white, TICK);
        }
        assert_eq!(g.screen().colors[0].r, 255);
        assert!(!g.ease(&white, TICK), "nothing changes once there");
    }

    /// Opt-in: captures the real screen for three seconds (`cargo test -p freergb-sources --
    /// --ignored --nocapture`).
    #[test]
    #[ignore]
    fn the_real_screen_can_be_captured() {
        let mut s = open(None).expect("the screen opens for capture");
        println!("backend {:?}", s.backend_info().kind);
        let until = Instant::now() + Duration::from_secs(3);
        let mut frames = 0;
        while Instant::now() < until {
            let e = s.next_event(Some(TICK));
            if let Err(e) = &e {
                println!("{e}");
            }
            if let Ok(CaptureEvent::Video(f)) = e {
                frames += 1;
                if let FrameData::Host(px) = &f.data {
                    let g = shrink(
                        px,
                        f.width as usize,
                        f.height as usize,
                        f.stride as usize,
                        true,
                    )
                    .expect("a frame larger than the grid");
                    println!(
                        "{}x{} {:?}, top left {:?}",
                        f.width, f.height, f.pixel_format, g[0]
                    );
                }
            }
        }
        assert!(
            frames > 0,
            "no frame in 3 s (DXGI sends only on change: move the mouse)"
        );
    }
}
