// SPDX-License-Identifier: GPL-2.0-or-later
//! Where the readings come from for looks that follow one (see `freergb_effects::react`).
//!
//! A [`Hub`] holds every [`Source`] this machine offers and runs only those a playing look
//! follows ([`Hub::want`]): reading a sensor, the sound or the screen costs something, and the
//! tray should cost nothing when no look reacts. A stopped source's value is cleared, so a
//! look never follows a stale reading. All values land in one [`Readings`] snapshot that the
//! engine and the window's preview read.

use freergb_effects::react::{Readings, SourceId};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub mod audio;
pub mod hwmon;
pub mod lhm;
pub mod machine;
pub mod screen;

/// Where a running source puts its values.
#[derive(Clone)]
pub struct Out(Arc<watch::Sender<Readings>>);

impl Out {
    /// Change the snapshot (only this source's own field, by convention).
    pub fn update(&self, f: impl FnOnce(&mut Readings)) {
        self.0.send_modify(f);
    }

    /// Set `id`'s value unless the source was stopped. The check runs under the snapshot's
    /// lock, so a read that finishes after [`Hub::want`] stopped and cleared the source
    /// cannot bring its value back.
    fn put(&self, live: &AtomicBool, id: SourceId, v: Option<f64>) {
        self.set(live, |r| set_value(r, id, v));
    }

    /// [`Out::update`] unless the source was stopped (see [`Out::put`]).
    fn set(&self, live: &AtomicBool, f: impl FnOnce(&mut Readings)) {
        self.0.send_modify(|r| {
            if live.load(Ordering::Acquire) {
                f(r)
            }
        });
    }
}

/// Marks a source stopped before the rest of its guard (a task, a thread's channel) drops.
struct Stop<T: Send>(Arc<AtomicBool>, #[allow(dead_code)] T);

impl<T: Send> Drop for Stop<T> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// A source while it runs; dropping it stops the source.
pub struct Running(#[allow(dead_code)] Box<dyn Send>);

impl Running {
    /// Stops when dropped by aborting `task` (for sources that own nothing beyond the task).
    pub fn task(task: JoinHandle<()>) -> Self {
        struct Abort(JoinHandle<()>);
        impl Drop for Abort {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        Running(Box::new(Abort(task)))
    }

    /// Stops when dropped by dropping `guard` (a stream, a thread's stop flag).
    pub fn guard(guard: impl Send + 'static) -> Self {
        Running(Box::new(guard))
    }
}

/// One kind of reading this machine can give.
pub trait Source: Send + Sync {
    fn id(&self) -> SourceId;
    /// What it takes to get a reading, shown while there is none (another program running).
    fn needs(&self) -> Option<&'static str> {
        None
    }
    /// Begin reading into `out`, spawning on `rt`; runs until the result is dropped. A source
    /// that fails later just stops writing (its value stays unset).
    fn start(&self, out: Out, rt: &Handle) -> Running;
}

/// A single-number source read every `period` by a plain function (temperatures, loads):
/// `read` returns `None` while it has no value.
pub struct Polled<F> {
    pub id: SourceId,
    pub period: Duration,
    pub read: Arc<F>,
}

impl<F: Fn() -> Option<f64> + Send + Sync + 'static> Source for Polled<F> {
    fn id(&self) -> SourceId {
        self.id
    }

    fn start(&self, out: Out, rt: &Handle) -> Running {
        let (id, period, read) = (self.id, self.period, self.read.clone());
        let live = Arc::new(AtomicBool::new(true));
        let alive = live.clone();
        let task = Running::task(rt.spawn(async move {
            let mut tick = tokio::time::interval(period);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                let v = read();
                out.put(&alive, id, v);
            }
        }));
        Running::guard(Stop(live, task))
    }
}

/// A single-number source whose state lives on a thread of its own while it runs: for readers
/// that are slow to open or bound to one thread (a driver library, a COM connection).
/// `open` makes the state, `read` reads it each `period`; a read giving `None` drops the state,
/// so the next tick opens it again (the other program may have started or come back).
pub struct Blocking<S> {
    pub id: SourceId,
    pub period: Duration,
    pub needs: Option<&'static str>,
    pub open: fn() -> Option<S>,
    pub read: fn(&mut S) -> Option<f64>,
}

impl<S: 'static> Source for Blocking<S> {
    fn id(&self) -> SourceId {
        self.id
    }

    fn needs(&self) -> Option<&'static str> {
        self.needs
    }

    fn start(&self, out: Out, _rt: &Handle) -> Running {
        let (id, period, open, read) = (self.id, self.period, self.open, self.read);
        let live = Arc::new(AtomicBool::new(true));
        let alive = live.clone();
        // Dropping the sender wakes the thread at once and ends it.
        let (stop, stopped) = mpsc::channel::<()>();
        let spawned = std::thread::Builder::new()
            .name(format!("freergb-{id:?}"))
            .spawn(move || {
                let mut state = None;
                loop {
                    if state.is_none() {
                        state = open();
                    }
                    let v = state.as_mut().and_then(read);
                    if v.is_none() {
                        state = None;
                    }
                    out.put(&alive, id, v);
                    if !matches!(
                        stopped.recv_timeout(period),
                        Err(mpsc::RecvTimeoutError::Timeout)
                    ) {
                        return;
                    }
                }
            });
        // A thread that cannot start gives no reading, as a sensor that is not there.
        drop(spawned);
        Running::guard(Stop(live, stop))
    }
}

/// One thing a look can follow on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offer {
    pub id: SourceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs: Option<&'static str>,
}

fn set_value(r: &mut Readings, id: SourceId, v: Option<f64>) {
    let v = v.filter(|v| v.is_finite());
    match id {
        SourceId::CpuTemp => r.cpu_temp = v,
        SourceId::GpuTemp => r.gpu_temp = v,
        SourceId::CpuLoad => r.cpu_load = v,
        SourceId::GpuLoad => r.gpu_load = v,
        SourceId::Sound => r.sound = v,
        SourceId::Beat => r.beat = v,
        SourceId::Spectrum | SourceId::Screen => {}
    }
}

/// The sources this machine offers, run on demand.
pub struct Hub {
    rt: Handle,
    sources: Vec<Box<dyn Source>>,
    running: Mutex<BTreeMap<SourceId, Running>>,
    out: Out,
}

impl Hub {
    /// `rt` is where sources run, so [`Hub::want`] may be called from any thread.
    pub fn new(sources: Vec<Box<dyn Source>>, rt: Handle) -> Self {
        Self {
            rt,
            sources,
            running: Mutex::new(BTreeMap::new()),
            out: Out(Arc::new(watch::channel(Readings::default()).0)),
        }
    }

    /// What a look can follow here, in a stable order (the first source of an id counts).
    pub fn available(&self) -> Vec<Offer> {
        let mut offers: BTreeMap<SourceId, Offer> = BTreeMap::new();
        for s in &self.sources {
            offers.entry(s.id()).or_insert(Offer {
                id: s.id(),
                needs: s.needs(),
            });
        }
        offers.into_values().collect()
    }

    /// Run exactly the sources in `ids` that this machine offers: start the missing ones,
    /// stop (and clear) the rest.
    pub fn want(&self, ids: &BTreeSet<SourceId>) {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let gone: Vec<SourceId> = running
            .keys()
            .filter(|id| !ids.contains(id))
            .copied()
            .collect();
        for id in gone {
            running.remove(&id);
            self.out.update(|r| r.clear(id));
        }
        for id in ids {
            if running.contains_key(id) {
                continue;
            }
            if let Some(s) = self.sources.iter().find(|s| s.id() == *id) {
                running.insert(*id, s.start(self.out.clone(), &self.rt));
            }
        }
    }

    /// The sources running now.
    pub fn running(&self) -> Vec<SourceId> {
        let running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        running.keys().copied().collect()
    }

    /// Follows every change to the readings.
    pub fn readings(&self) -> watch::Receiver<Readings> {
        self.out.0.subscribe()
    }
}

/// The sources the looks in `effects` follow.
pub fn demand<'a>(
    effects: impl IntoIterator<Item = &'a freergb_effects::EffectSpec>,
) -> BTreeSet<SourceId> {
    effects
        .into_iter()
        .filter_map(|e| e.react.map(|m| m.source))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn counter(id: SourceId, calls: Arc<AtomicU32>) -> Box<dyn Source> {
        Box::new(Polled {
            id,
            period: Duration::from_millis(100),
            read: Arc::new(move || Some(calls.fetch_add(1, Ordering::Relaxed) as f64 + 40.0)),
        })
    }

    #[tokio::test(start_paused = true)]
    async fn a_source_runs_only_while_a_look_follows_it_and_its_value_goes_with_it() {
        let calls = Arc::new(AtomicU32::new(0));
        let hub = Hub::new(
            vec![counter(SourceId::CpuTemp, calls.clone())],
            Handle::current(),
        );
        assert_eq!(
            hub.available(),
            [Offer {
                id: SourceId::CpuTemp,
                needs: None
            }]
        );
        let rx = hub.readings();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "nothing runs until wanted"
        );

        hub.want(&[SourceId::CpuTemp, SourceId::Sound].into());
        // Only what this machine offers runs.
        assert_eq!(hub.running(), [SourceId::CpuTemp]);
        tokio::time::sleep(Duration::from_millis(350)).await;
        let n = calls.load(Ordering::Relaxed);
        assert!((3..=5).contains(&n), "{n} reads in 350 ms at 100 ms");
        assert!(rx.borrow().cpu_temp.is_some_and(|t| t >= 40.0));

        hub.want(&BTreeSet::new());
        assert!(hub.running().is_empty());
        assert_eq!(
            rx.borrow().cpu_temp,
            None,
            "a stopped source leaves no stale value"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            calls.load(Ordering::Relaxed),
            n,
            "a stopped source reads no more"
        );
    }

    static OPENS: AtomicU32 = AtomicU32::new(0);
    static READS: AtomicU32 = AtomicU32::new(0);

    #[tokio::test]
    async fn a_threaded_source_opens_again_after_a_failed_read_and_stops_cleanly() {
        let hub = Hub::new(
            vec![Box::new(Blocking {
                id: SourceId::GpuTemp,
                period: Duration::from_millis(10),
                needs: Some("Needs a test"),
                // The first open fails (the program is not running yet).
                open: || (OPENS.fetch_add(1, Ordering::SeqCst) > 0).then_some(0u32),
                // Every third read fails (the program went away), which opens it again.
                read: |n: &mut u32| {
                    *n += 1;
                    READS.fetch_add(1, Ordering::SeqCst);
                    (*n % 3 != 0).then_some(70.0)
                },
            })],
            Handle::current(),
        );
        assert_eq!(hub.available()[0].needs, Some("Needs a test"));
        let mut rx = hub.readings();
        hub.want(&[SourceId::GpuTemp].into());
        tokio::time::timeout(
            Duration::from_secs(5),
            rx.wait_for(|r| r.gpu_temp == Some(70.0)),
        )
        .await
        .expect("a reading arrives")
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            OPENS.load(Ordering::SeqCst) >= 3,
            "opened again after failures"
        );

        hub.want(&BTreeSet::new());
        assert_eq!(rx.borrow().gpu_temp, None);
        let n = READS.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(READS.load(Ordering::SeqCst) <= n + 1, "the thread ends");
        assert_eq!(
            rx.borrow().gpu_temp,
            None,
            "no value comes back after the stop"
        );
    }

    #[test]
    fn this_machine_always_offers_the_cpu_load() {
        let found = machine::local(None);
        assert_eq!(found[0].id(), SourceId::CpuLoad);
    }

    #[test]
    fn the_demand_is_what_the_looks_follow() {
        let mut a: freergb_effects::EffectSpec = rainbow();
        a.react = Some(freergb_effects::react::React {
            source: SourceId::Sound,
            drives: freergb_effects::react::Drive::Brightness,
            low: None,
            high: None,
            sensitivity: None,
            strength: None,
        });
        let b = rainbow();
        assert_eq!(demand([&a, &b]), [SourceId::Sound].into());
    }

    fn rainbow() -> freergb_effects::EffectSpec {
        freergb_effects::EffectSpec {
            kind: freergb_effects::EffectKind::Rainbow,
            colors: vec![],
            speed: 0.2,
            direction: Default::default(),
            brightness: 1.0,
            density: 1.0,
            mode: None,
            mode_direction: None,
            angle_deg: None,
            zones: vec![],
            react: None,
        }
    }
}
