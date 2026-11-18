// SPDX-License-Identifier: GPL-2.0-or-later
//! What the PC plays, as three readings: the level, a pulse on each beat, and the spectrum.
//!
//! The sound is taken from the output itself, not a microphone: WASAPI loopback of the default
//! output on Windows, the default output's monitor through PulseAudio on Linux (PipeWire serves
//! the same protocol). One capture serves every sound reading that runs; it closes when the
//! last one stops.

use crate::{Out, Running, Source};
use freergb_effects::react::SourceId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

/// Spectrum bands, low to high, spaced evenly in pitch.
pub const BANDS: usize = 16;
const LOWEST_HZ: f64 = 40.0;
const HIGHEST_HZ: f64 = 16_000.0;
/// Samples per analysis: about 43 ms at 48 kHz, fine enough for 23 Hz bins in the bass.
const N: usize = 2048;
/// How often the readings update: about the engine's frame rate.
const TICK: Duration = Duration::from_millis(33);
/// No samples for this long means silence (WASAPI sends nothing while nothing plays).
const QUIET_AFTER: Duration = Duration::from_millis(100);
const RETRY: Duration = Duration::from_secs(2);

/// The level is loudness against the loudest of late, so it follows the music at any volume
/// instead of sitting near full: this many dB under it reads 0. The reference falls slowly,
/// so a quiet passage reads quieter, and not under a floor, so near silence stays dark.
const LEVEL_SPAN_DB: f64 = 24.0;
const LEVEL_REF_FLOOR_DB: f64 = -30.0;
const REF_FALL_DB_S: f64 = 1.5;
/// A band is its loudness against the loudest band of late (treble lifted, as music falls
/// about 3 dB an octave), turned down while it stays at its own recent average: a steady
/// instrument keeps its band at a glow, a new sound lights it fully.
const BAND_SPAN_DB: f64 = 30.0;
const BAND_REF_FLOOR_DB: f64 = -50.0;
const TILT_DB_PER_OCTAVE: f64 = 3.0;
const BAND_AVG_S: f64 = 2.0;
const BAND_RISE_DB: f64 = 10.0;
const BAND_STEADY: f64 = 0.35;
/// Rising is quick, falling slower, so lights pump with the music without flicker.
const ATTACK_S: f64 = 0.06;
const RELEASE_S: f64 = 0.4;
/// The bass that counts for beats, and what a beat is: bass this much over its recent average,
/// at least this loud, at least this long after the last one. Its strength grows with the
/// jump, full at `BEAT_FULL` times the average.
const BASS_HZ: (f64, f64) = (40.0, 150.0);
const BEAT_RISE: f64 = 1.8;
const BEAT_FULL: f64 = 4.0;
const BEAT_WEAKEST: f64 = 0.3;
const BEAT_FLOOR_DB: f64 = -40.0;
const BEAT_GAP_S: f64 = 0.3;
const BASS_AVG_S: f64 = 0.8;
const BEAT_FADE_S: f64 = 0.25;

/// One update of the three readings.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub level: f64,
    pub beat: f64,
    pub bands: Vec<f64>,
}

/// Turns the latest samples (mono) into readings, one [`Frame`] per tick.
pub struct Analyser {
    rate: f64,
    ring: Vec<f32>,
    pos: usize,
    hann: Vec<f64>,
    /// Each band's FFT bins, `lo..hi`.
    bins: Vec<(usize, usize)>,
    bass: (usize, usize),
    level: f64,
    /// The loudest level of late, dBFS.
    level_ref: f64,
    bands: Vec<f64>,
    /// Each band's lift for the treble, dB.
    tilt: Vec<f64>,
    /// The loudest (lifted) band of late, dBFS.
    band_ref: f64,
    /// Each band's recent average, dBFS.
    band_avg: Vec<f64>,
    beat: f64,
    bass_avg: f64,
    since_beat: f64,
}

impl Analyser {
    pub fn new(rate: u32) -> Self {
        let rate = f64::from(rate.max(8000));
        let bin = |hz: f64| ((hz * N as f64 / rate).floor() as usize).clamp(1, N / 2 - 1);
        let edge = |i: usize| LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ).powf(i as f64 / BANDS as f64);
        let bins = (0..BANDS)
            .map(|i| {
                let lo = bin(edge(i));
                (lo, bin(edge(i + 1)).max(lo + 1))
            })
            .collect();
        let hann = (0..N)
            .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (N - 1) as f64).cos())
            .collect();
        let tilt = (0..BANDS)
            // Around each band's middle, 0 dB at 1 kHz.
            .map(|i| TILT_DB_PER_OCTAVE * ((edge(i) * edge(i + 1)).sqrt() / 1000.0).log2())
            .collect();
        let bass_lo = bin(BASS_HZ.0);
        Self {
            rate,
            ring: vec![0.0; N],
            pos: 0,
            hann,
            bins,
            bass: (bass_lo, bin(BASS_HZ.1).max(bass_lo + 1)),
            level: 0.0,
            level_ref: LEVEL_REF_FLOOR_DB,
            bands: vec![0.0; BANDS],
            tilt,
            band_ref: BAND_REF_FLOOR_DB,
            band_avg: vec![SILENT_DB; BANDS],
            beat: 0.0,
            bass_avg: 0.0,
            since_beat: f64::INFINITY,
        }
    }

    pub fn push(&mut self, mono: &[f32]) {
        for &s in mono {
            self.ring[self.pos] = if s.is_finite() { s } else { 0.0 };
            self.pos = (self.pos + 1) % N;
        }
    }

    /// `secs` of silence.
    pub fn silence(&mut self, secs: f64) {
        let n = ((secs * self.rate) as usize).min(N);
        self.push(&vec![0.0; n]);
    }

    /// The readings after `dt` seconds.
    pub fn frame(&mut self, dt: f64) -> Frame {
        // Oldest first.
        let samples: Vec<f64> = (0..N)
            .map(|i| f64::from(self.ring[(self.pos + i) % N]))
            .collect();

        // Level: RMS of the last 50 ms.
        let recent = ((self.rate * 0.05) as usize).clamp(1, N);
        let rms = (samples[N - recent..].iter().map(|s| s * s).sum::<f64>() / recent as f64).sqrt();
        let now_db = db(rms);
        self.level_ref = fall(self.level_ref, now_db, LEVEL_REF_FLOOR_DB, dt);
        let lo = self.level_ref - LEVEL_SPAN_DB;
        self.level = follow(self.level, unit(now_db, (lo, self.level_ref)), dt);

        // Spectrum: each band's peak, scaled so a full-scale sine reads 0 dB.
        let mut re: Vec<f64> = samples.iter().zip(&self.hann).map(|(s, w)| s * w).collect();
        let mut im = vec![0.0; N];
        fft(&mut re, &mut im);
        let gain = 2.0 / self.hann.iter().sum::<f64>();
        let amp: Vec<f64> = (0..N / 2)
            .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt() * gain)
            .collect();
        let peaks: Vec<f64> = self
            .bins
            .iter()
            .map(|&(lo, hi)| db(amp[lo..hi].iter().copied().fold(0.0, f64::max)))
            .collect();
        let top = peaks
            .iter()
            .zip(&self.tilt)
            .map(|(p, t)| p + t)
            .fold(f64::NEG_INFINITY, f64::max);
        self.band_ref = fall(self.band_ref, top, BAND_REF_FLOOR_DB, dt);
        let avg_k = 1.0 - (-dt / BAND_AVG_S).exp();
        for (b, &peak) in peaks.iter().enumerate() {
            let lifted = peak + self.tilt[b];
            let steady = unit(lifted, (self.band_ref - BAND_SPAN_DB, self.band_ref));
            let new = unit(peak - self.band_avg[b], (0.0, BAND_RISE_DB));
            let target = steady * (BAND_STEADY + (1.0 - BAND_STEADY) * new);
            self.bands[b] = follow(self.bands[b], target, dt);
            self.band_avg[b] += (peak.max(SILENT_DB) - self.band_avg[b]) * avg_k;
        }

        // Beat: the bass's power jumping over its recent average.
        let (lo, hi) = self.bass;
        let bass = amp[lo..hi].iter().map(|a| a * a).sum::<f64>() / (hi - lo) as f64;
        self.since_beat += dt;
        let loud = 10.0 * bass.max(1e-12).log10() > BEAT_FLOOR_DB;
        self.beat *= (-dt / BEAT_FADE_S).exp();
        if loud && bass > BEAT_RISE * self.bass_avg && self.since_beat >= BEAT_GAP_S {
            let jump = (bass / self.bass_avg.max(1e-12)).ln();
            let k = unit(jump, (BEAT_RISE.ln(), BEAT_FULL.ln()));
            self.beat = self.beat.max(BEAT_WEAKEST + (1.0 - BEAT_WEAKEST) * k);
            self.since_beat = 0.0;
        }
        self.bass_avg += (bass - self.bass_avg) * (1.0 - (-dt / BASS_AVG_S).exp());

        Frame {
            level: self.level,
            beat: self.beat,
            bands: self.bands.clone(),
        }
    }
}

/// Where a band's average rests in silence, dBFS.
const SILENT_DB: f64 = -90.0;

/// A reference loudness: up at once to `now`, else down by `REF_FALL_DB_S`, never under `floor`.
fn fall(reference: f64, now: f64, floor: f64, dt: f64) -> f64 {
    (reference - REF_FALL_DB_S * dt).max(now).max(floor)
}

fn db(amplitude: f64) -> f64 {
    20.0 * amplitude.max(1e-9).log10()
}

fn unit(db: f64, (lo, hi): (f64, f64)) -> f64 {
    ((db - lo) / (hi - lo)).clamp(0.0, 1.0)
}

fn follow(prev: f64, target: f64, dt: f64) -> f64 {
    let tau = if target > prev { ATTACK_S } else { RELEASE_S };
    prev + (target - prev) * (1.0 - (-dt / tau).exp())
}

/// In-place radix-2 FFT; `re.len()` is a power of two.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = -std::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (step * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// The sound readings this machine can give: none where no output can be captured.
pub fn sources() -> Vec<Box<dyn Source>> {
    if !capturable() {
        return vec![];
    }
    let shared = Arc::new(Mutex::new(Shared::default()));
    [SourceId::Sound, SourceId::Beat, SourceId::Spectrum]
        .into_iter()
        .map(|id| {
            Box::new(Sound {
                id,
                shared: shared.clone(),
            }) as Box<dyn Source>
        })
        .collect()
}

fn capturable() -> bool {
    #[cfg(windows)]
    return true;
    #[cfg(target_os = "linux")]
    return cpal::available_hosts().contains(&cpal::HostId::PulseAudio);
    #[allow(unreachable_code)]
    false
}

/// Which sound readings run, and the capture thread while any does.
#[derive(Default)]
struct Shared {
    /// Each running reading's live flag, by [`slot`].
    live: [Option<Arc<AtomicBool>>; 3],
    /// Dropping it ends the capture thread.
    stop: Option<mpsc::Sender<()>>,
}

fn slot(id: SourceId) -> usize {
    match id {
        SourceId::Sound => 0,
        SourceId::Beat => 1,
        _ => 2,
    }
}

struct Sound {
    id: SourceId,
    shared: Arc<Mutex<Shared>>,
}

impl Source for Sound {
    fn id(&self) -> SourceId {
        self.id
    }

    fn start(&self, out: Out, _rt: &Handle) -> Running {
        let live = Arc::new(AtomicBool::new(true));
        let mut st = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        st.live[slot(self.id)] = Some(live.clone());
        if st.stop.is_none() {
            let (stop, stopped) = mpsc::channel();
            let shared = self.shared.clone();
            let spawned = std::thread::Builder::new()
                .name("freergb-sound".into())
                .spawn(move || capture(shared, out, stopped));
            if spawned.is_ok() {
                st.stop = Some(stop);
            }
        }
        Running::guard(Leave {
            slot: slot(self.id),
            live,
            shared: self.shared.clone(),
        })
    }
}

/// A sound reading's guard: marks it stopped and closes the capture after the last one.
struct Leave {
    slot: usize,
    live: Arc<AtomicBool>,
    shared: Arc<Mutex<Shared>>,
}

impl Drop for Leave {
    fn drop(&mut self) {
        self.live.store(false, Ordering::Release);
        let mut st = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        st.live[self.slot] = None;
        if st.live.iter().all(Option::is_none) {
            st.stop = None;
        }
    }
}

/// The capture thread: opens the output (again after a failure), analyses each tick and writes
/// the readings that run.
fn capture(shared: Arc<Mutex<Shared>>, out: Out, stopped: mpsc::Receiver<()>) {
    let mut open: Option<(Capture, Analyser)> = None;
    let mut retry_at = Instant::now();
    let mut last_tick = Instant::now();
    let mut last_data = Instant::now();
    let mut check_at = Instant::now() + RETRY;
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f64().min(0.25);
        last_tick = now;
        if open
            .as_ref()
            .is_some_and(|(c, _)| c.failed.load(Ordering::Acquire))
        {
            open = None;
            retry_at = now + RETRY;
        }
        // Follow the default output when the user switches it (speakers to headphones).
        if now >= check_at {
            check_at = now + RETRY;
            if open
                .as_ref()
                .is_some_and(|(c, _)| default_output().is_some_and(|d| d != c.of))
            {
                open = None;
                retry_at = now;
            }
        }
        if open.is_none() && now >= retry_at {
            match Capture::open() {
                Some(c) => {
                    let a = Analyser::new(c.rate);
                    open = Some((c, a));
                    last_data = now;
                }
                None => retry_at = now + RETRY,
            }
        }
        let frame = open.as_mut().map(|(c, a)| {
            let fresh = std::mem::take(&mut *c.buf.lock().unwrap_or_else(|e| e.into_inner()));
            if fresh.is_empty() {
                if now.duration_since(last_data) > QUIET_AFTER {
                    a.silence(dt);
                }
            } else {
                a.push(&fresh);
                last_data = now;
            }
            a.frame(dt)
        });
        let live = shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .live
            .clone();
        for (i, flag) in live.iter().enumerate() {
            let Some(flag) = flag else { continue };
            let f = frame.as_ref();
            out.set(flag, |r| match i {
                0 => r.sound = f.map(|f| f.level),
                1 => r.beat = f.map(|f| f.beat),
                _ => r.spectrum = f.map(|f| f.bands.clone()),
            });
        }
        if !matches!(
            stopped.recv_timeout(TICK),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            return;
        }
    }
}

/// An open capture of the output: its samples (mono) gather in `buf` until the next tick.
struct Capture {
    _stream: cpal::Stream,
    buf: Arc<Mutex<Vec<f32>>>,
    failed: Arc<AtomicBool>,
    rate: u32,
    /// The output captured (its id): a different default output means opening again.
    of: String,
}

/// The id of the default output, where the sound is taken from.
fn default_output() -> Option<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    #[cfg(windows)]
    let host = cpal::default_host();
    #[cfg(target_os = "linux")]
    let host = cpal::host_from_id(cpal::HostId::PulseAudio).ok()?;
    #[cfg(not(any(windows, target_os = "linux")))]
    return None;
    #[cfg(any(windows, target_os = "linux"))]
    Some(host.default_output_device()?.id().ok()?.id().to_string())
}

impl Capture {
    fn open() -> Option<Self> {
        use cpal::traits::{DeviceTrait, HostTrait};
        let of = default_output()?;
        #[cfg(windows)]
        let (device, config) = {
            // An output device opened for input records what it plays (loopback). The default
            // device cpal hands out is a virtual one activated asynchronously, which can wait
            // forever on a thread like this one; the endpoint itself opens the usual way, and
            // the capture thread reopens when the default changes.
            let device = cpal::default_host()
                .output_devices()
                .ok()?
                .find(|d| d.id().is_ok_and(|i| i.id() == of))?;
            let config = device.default_output_config().ok()?;
            (device, config)
        };
        #[cfg(target_os = "linux")]
        let (device, config) = {
            let host = cpal::host_from_id(cpal::HostId::PulseAudio).ok()?;
            let monitor = format!("{of}.monitor");
            let device = host
                .input_devices()
                .ok()?
                .find(|d| d.id().is_ok_and(|i| i.id() == monitor))?;
            let config = device.default_input_config().ok()?;
            (device, config)
        };
        #[cfg(not(any(windows, target_os = "linux")))]
        return None;

        #[cfg(any(windows, target_os = "linux"))]
        {
            let buf = Arc::new(Mutex::new(Vec::new()));
            let failed = Arc::new(AtomicBool::new(false));
            let rate = config.sample_rate();
            let stream = match config.sample_format() {
                cpal::SampleFormat::I16 => build::<i16>(&device, &config, &buf, &failed),
                cpal::SampleFormat::I32 => build::<i32>(&device, &config, &buf, &failed),
                cpal::SampleFormat::U16 => build::<u16>(&device, &config, &buf, &failed),
                _ => build::<f32>(&device, &config, &buf, &failed),
            }?;
            use cpal::traits::StreamTrait;
            stream.play().ok()?;
            Some(Capture {
                _stream: stream,
                buf,
                failed,
                rate,
                of,
            })
        }
    }
}

/// Whether a stream error means the capture has to be opened again. A glitch does not: WASAPI
/// reports one on the first loopback buffer, and reopening for it never let a reading through.
fn ends_stream(kind: cpal::ErrorKind) -> bool {
    use cpal::ErrorKind::*;
    !matches!(kind, Xrun | DeviceChanged | RealtimeDenied)
}

/// Most samples kept between ticks (a tick that comes late drops the oldest).
const BUF_MAX: usize = 4 * N;

#[cfg(any(windows, target_os = "linux"))]
fn build<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    buf: &Arc<Mutex<Vec<f32>>>,
    failed: &Arc<AtomicBool>,
) -> Option<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;
    let channels = usize::from(config.channels()).max(1);
    let (buf, failed) = (buf.clone(), failed.clone());
    device
        .build_input_stream::<T, _, _>(
            cpal::StreamConfig {
                channels: config.channels(),
                sample_rate: config.sample_rate(),
                // PulseAudio's default record fragment is up to two seconds, which would make
                // the lights lag; WASAPI's default period is already about 10 ms.
                buffer_size: if cfg!(target_os = "linux") {
                    cpal::BufferSize::Fixed(1024)
                } else {
                    cpal::BufferSize::Default
                },
            },
            move |data: &[T], _| {
                let mut b = buf.lock().unwrap_or_else(|e| e.into_inner());
                b.extend(data.chunks(channels).map(|frame| {
                    frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / frame.len() as f32
                }));
                let over = b.len().saturating_sub(BUF_MAX);
                b.drain(..over);
            },
            move |e: cpal::Error| {
                if ends_stream(e.kind()) {
                    failed.store(true, Ordering::Release)
                }
            },
            Some(Duration::from_secs(2)),
        )
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    const DT: f64 = 0.033;

    fn tone(hz: f64, amp: f64, secs: f64, from: usize) -> Vec<f32> {
        let n = (secs * f64::from(RATE)) as usize;
        (from..from + n)
            .map(|i| (amp * (std::f64::consts::TAU * hz * i as f64 / f64::from(RATE)).sin()) as f32)
            .collect()
    }

    /// Feeds `samples` in tick-sized pieces, returning every frame.
    fn run(a: &mut Analyser, samples: &[f32]) -> Vec<Frame> {
        let per = (DT * f64::from(RATE)) as usize;
        samples
            .chunks(per)
            .map(|c| {
                a.push(c);
                a.frame(DT)
            })
            .collect()
    }

    #[test]
    fn a_glitch_keeps_the_capture_and_a_lost_device_reopens_it() {
        assert!(!ends_stream(cpal::ErrorKind::Xrun));
        assert!(!ends_stream(cpal::ErrorKind::DeviceChanged));
        assert!(ends_stream(cpal::ErrorKind::DeviceNotAvailable));
        assert!(ends_stream(cpal::ErrorKind::StreamInvalidated));
    }

    #[test]
    fn silence_reads_nothing() {
        let mut a = Analyser::new(RATE);
        let f = run(&mut a, &vec![0.0; RATE as usize]).pop().unwrap();
        assert_eq!(f.level, 0.0);
        assert_eq!(f.beat, 0.0);
        assert!(f.bands.iter().all(|&b| b == 0.0), "{:?}", f.bands);
    }

    #[test]
    fn a_tone_lights_its_band_and_the_level() {
        let mut a = Analyser::new(RATE);
        let f = run(&mut a, &tone(1000.0, 0.5, 1.0, 0)).pop().unwrap();
        assert!(f.level > 0.8, "level {}", f.level);
        let top = (0..BANDS)
            .max_by(|&i, &j| f.bands[i].total_cmp(&f.bands[j]))
            .unwrap();
        let edge = |i: usize| LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ).powf(i as f64 / BANDS as f64);
        assert!(edge(top) <= 1000.0 && 1000.0 < edge(top + 1), "band {top}");
        assert!(f.bands[top] > 0.9, "{:?}", f.bands);
        assert!(
            f.bands[0] < 0.1 && f.bands[BANDS - 1] < 0.1,
            "{:?}",
            f.bands
        );
        assert!(f.beat < 0.05, "a steady tone is no beat: {}", f.beat);
    }

    #[test]
    fn the_level_falls_slowly_after_the_sound_stops() {
        let mut a = Analyser::new(RATE);
        let loud = run(&mut a, &tone(440.0, 0.5, 0.5, 0)).pop().unwrap().level;
        a.silence(DT);
        let next = a.frame(DT).level;
        assert!(next > 0.5 * loud && next < loud, "{loud} then {next}");
        for _ in 0..60 {
            a.silence(DT);
            a.frame(DT);
        }
        assert!(a.frame(DT).level < 0.01);
    }

    #[test]
    fn each_bass_hit_is_a_beat_that_fades() {
        let mut a = Analyser::new(RATE);
        // Eight bars of a kick every half second: 100 ms of 60 Hz, then 400 ms of quiet.
        let mut samples = vec![];
        for k in 0..8 {
            samples.extend(tone(60.0, 0.6, 0.1, k * 24_000));
            samples.extend(vec![0.0; 19_200]);
        }
        let frames = run(&mut a, &samples);
        let hits = frames
            .windows(2)
            .filter(|w| w[1].beat > w[0].beat + 0.2)
            .count();
        assert!((7..=8).contains(&hits), "{hits} beats for 8 kicks");
        assert_eq!(frames.iter().map(|f| f.beat).fold(0.0, f64::max), 1.0);
        // Between kicks it fades toward zero.
        let quiet = frames.iter().map(|f| f.beat).fold(1.0, f64::min);
        assert!(quiet < 0.2, "fades to {quiet}");
    }

    #[test]
    fn a_small_bass_jump_is_a_weaker_beat_than_a_kick_from_quiet() {
        let mut a = Analyser::new(RATE);
        // Two seconds of steady bass, then the same bass half as loud again for 100 ms.
        let mut samples = tone(60.0, 0.2, 2.0, 0);
        samples.extend(tone(60.0, 0.33, 0.1, samples.len()));
        let frames = run(&mut a, &samples);
        let (before, after) = frames.split_at(frames.len() - 3);
        assert!(
            before[30..].iter().all(|f| f.beat < 0.3),
            "steady bass is no beat"
        );
        let peak = after.iter().map(|f| f.beat).fold(0.0, f64::max);
        assert!((0.3..0.9).contains(&peak), "weaker beat {peak}");
    }

    #[test]
    fn a_band_that_stays_steady_settles_to_a_glow() {
        let mut a = Analyser::new(RATE);
        let frames = run(&mut a, &tone(1000.0, 0.5, 8.0, 0));
        let band = |f: &Frame| f.bands.iter().copied().fold(0.0, f64::max);
        assert!(band(&frames[30]) > 0.9, "a new sound lights its band");
        let last = band(frames.last().unwrap());
        assert!((0.3..0.6).contains(&last), "settles to {last}");
    }

    #[test]
    fn the_level_follows_the_music_at_any_volume() {
        let quiet = |amp: f64| {
            let mut a = Analyser::new(RATE);
            // A loud part, then a part 12 dB quieter.
            let mut s = tone(440.0, amp, 1.0, 0);
            s.extend(tone(440.0, amp / 4.0, 1.0, s.len()));
            run(&mut a, &s).pop().unwrap().level
        };
        let (soft, loud) = (quiet(0.1), quiet(0.8));
        assert!((0.3..0.7).contains(&soft), "{soft}");
        assert!((soft - loud).abs() < 0.05, "{soft} vs {loud}");
    }

    #[tokio::test]
    async fn one_capture_serves_every_sound_reading_and_closes_after_the_last() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let sound = |id| -> Box<dyn Source> {
            Box::new(Sound {
                id,
                shared: shared.clone(),
            })
        };
        let hub = crate::Hub::new(
            vec![
                sound(SourceId::Sound),
                sound(SourceId::Beat),
                sound(SourceId::Spectrum),
            ],
            Handle::current(),
        );
        let capturing = || shared.lock().unwrap().stop.is_some();
        hub.want(&[SourceId::Sound, SourceId::Spectrum].into());
        assert!(capturing());
        hub.want(&[SourceId::Beat].into());
        assert!(capturing(), "still one reading running");
        let live = shared
            .lock()
            .unwrap()
            .live
            .iter()
            .filter(|l| l.is_some())
            .count();
        assert_eq!(live, 1);
        hub.want(&Default::default());
        assert!(!capturing(), "the capture closes with the last reading");
        let r = hub.readings().borrow().clone();
        assert_eq!((r.sound, r.beat, r.spectrum), (None, None, None));
    }

    /// Opt-in, needs a real output: `cargo test -p freergb-sources -- --ignored --nocapture`
    /// (play something to see the readings move).
    #[test]
    #[ignore]
    fn the_real_output_can_be_captured() {
        let c = Capture::open().expect("the output opens for capture");
        let mut a = Analyser::new(c.rate);
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            let fresh = std::mem::take(&mut *c.buf.lock().unwrap());
            a.push(&fresh);
            let f = a.frame(0.1);
            println!(
                "{} samples, level {:.2}, beat {:.2}",
                fresh.len(),
                f.level,
                f.beat
            );
        }
        assert!(!c.failed.load(Ordering::Acquire));
    }

    #[test]
    fn the_fft_finds_a_pure_tone_in_its_bin() {
        let n = 64;
        let mut re: Vec<f64> = (0..n)
            .map(|i| (std::f64::consts::TAU * 5.0 * i as f64 / n as f64).cos())
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mag: Vec<f64> = (0..n).map(|k| re[k].hypot(im[k])).collect();
        assert!((mag[5] - 32.0).abs() < 1e-9 && (mag[59] - 32.0).abs() < 1e-9);
        assert!(mag
            .iter()
            .enumerate()
            .all(|(k, &m)| k == 5 || k == 59 || m < 1e-9));
    }
}
