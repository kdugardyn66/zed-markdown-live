//! Background jobs for slow, server-side blocks (PlantUML, Kroki, code chunks).
//!
//! A render never waits for them: a cache miss schedules a job and the block shows
//! the previous output of the same block (dimmed) or a placeholder. When the job
//! finishes, the document is re-rendered and picks the result from the cache.
//! A job is skipped when the latest render no longer needs it (you kept typing).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub type JobFn = Box<dyn FnOnce() -> Result<String, String> + Send>;

struct Queued {
    key: String,
    run: JobFn,
    not_before: Instant,
    disk: bool,
}

#[derive(Default)]
struct Inner {
    cache: HashMap<String, Result<String, String>>,
    inflight: HashSet<String>,
    queue: VecDeque<Queued>,
    /// last good output per block slot (file + kind + ordinal), shown while re-rendering
    stale: HashMap<String, String>,
    /// keys referenced by the latest render of each file
    wanted: HashMap<PathBuf, HashSet<String>>,
    running: usize,
    shutdown: bool,
}

pub enum Lookup {
    Ready(Result<String, String>),
    /// Rendering; the previous output of this block if there is one.
    Pending(Option<String>),
}

pub struct Jobs {
    inner: Mutex<Inner>,
    cv: Condvar,
    on_done: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    disk_dir: PathBuf,
}

impl Jobs {
    pub fn new() -> Arc<Jobs> {
        let j = Arc::new(Jobs {
            inner: Mutex::new(Inner::default()),
            cv: Condvar::new(),
            on_done: Mutex::new(None),
            disk_dir: crate::util::cache_dir().join("render-cache"),
        });
        let n = std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(1).clamp(1, 3);
        for _ in 0..n {
            let j2 = j.clone();
            std::thread::spawn(move || j2.worker());
        }
        j
    }

    pub fn set_on_done(&self, f: Box<dyn Fn() + Send + Sync>) {
        *self.on_done.lock().unwrap() = Some(f);
    }

    /// Starts a render pass for `file`: keys referenced from now on are "wanted".
    pub fn begin(&self, file: &PathBuf) {
        self.inner.lock().unwrap().wanted.insert(file.clone(), HashSet::new());
    }

    /// Looks `key` up; on a miss schedules `make()` after `delay` (debounce while typing).
    /// `disk` results survive restarts (deterministic renders such as PlantUML).
    pub fn get(&self, file: &PathBuf, slot: &str, key: &str, delay: Duration, disk: bool, make: impl FnOnce() -> JobFn) -> Lookup {
        let mut g = self.inner.lock().unwrap();
        g.wanted.entry(file.clone()).or_default().insert(key.to_string());
        if let Some(r) = g.cache.get(key).cloned() {
            if let Ok(html) = &r {
                g.stale.insert(slot.to_string(), html.clone());
            }
            return Lookup::Ready(r);
        }
        if disk {
            if let Ok(html) = std::fs::read_to_string(self.disk_dir.join(key)) {
                g.cache.insert(key.to_string(), Ok(html.clone()));
                g.stale.insert(slot.to_string(), html.clone());
                return Lookup::Ready(Ok(html));
            }
        }
        if !g.inflight.contains(key) {
            g.inflight.insert(key.to_string());
            g.queue.push_back(Queued { key: key.to_string(), run: make(), not_before: Instant::now() + delay, disk });
            self.cv.notify_all();
        }
        Lookup::Pending(g.stale.get(slot).cloned())
    }

    /// Number of jobs queued or running.
    pub fn busy(&self) -> usize {
        let g = self.inner.lock().unwrap();
        g.queue.len() + g.running
    }

    /// Forgets cached results (the ⟳ button): code chunks run again, diagrams re-render.
    pub fn clear(&self) {
        let mut g = self.inner.lock().unwrap();
        g.cache.clear();
        let _ = std::fs::remove_dir_all(&self.disk_dir);
    }

    pub fn shutdown(&self) {
        self.inner.lock().unwrap().shutdown = true;
        self.cv.notify_all();
    }

    fn worker(self: Arc<Self>) {
        loop {
            let job = {
                let mut g = self.inner.lock().unwrap();
                loop {
                    if g.shutdown {
                        return;
                    }
                    let now = Instant::now();
                    if let Some(i) = g.queue.iter().position(|q| q.not_before <= now) {
                        break g.queue.remove(i).unwrap();
                    }
                    let wait = g.queue.iter().map(|q| q.not_before.saturating_duration_since(now)).min().unwrap_or(Duration::from_secs(3600));
                    g = self.cv.wait_timeout(g, wait).unwrap().0;
                }
            };
            // Skip work the latest render no longer needs (the text changed meanwhile).
            {
                let mut g = self.inner.lock().unwrap();
                let wanted = g.wanted.values().any(|s| s.contains(&job.key));
                if !wanted {
                    g.inflight.remove(&job.key);
                    continue;
                }
                g.running += 1;
            }
            let res = (job.run)();
            {
                let mut g = self.inner.lock().unwrap();
                g.running -= 1;
                g.inflight.remove(&job.key);
                if job.disk {
                    if let Ok(html) = &res {
                        let _ = std::fs::create_dir_all(&self.disk_dir);
                        let _ = std::fs::write(self.disk_dir.join(&job.key), html);
                    }
                }
                g.cache.insert(job.key.clone(), res);
                if g.cache.len() > 2000 {
                    // crude bound: drop everything not wanted right now
                    let keep: HashSet<String> = g.wanted.values().flatten().cloned().collect();
                    g.cache.retain(|k, _| keep.contains(k));
                }
            }
            if let Some(f) = self.on_done.lock().unwrap().as_ref() {
                f();
            }
        }
    }
}

/// Runs a job right away (CLI `render`, where waiting is fine).
pub fn run_now(make: impl FnOnce() -> JobFn) -> Result<String, String> {
    (make())()
}
