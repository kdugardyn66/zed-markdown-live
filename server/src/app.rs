//! Shared application state: open documents, the preview target and the render worker.

use crate::config::Settings;
use crate::markdown::{self, Ctx, Heading};
use crate::util::{self, norm_path, path_to_uri};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Lsp,
    Standalone,
}

#[derive(Clone, Debug, Default)]
pub struct View {
    pub version: u64,
    pub file: Option<PathBuf>,
    pub html: String,
    pub title: Option<String>,
    pub headings: Vec<Heading>,
    pub render_ms: u128,
    pub focus: u64,
    /// Code chunks not run because execution is off.
    pub blocked_chunks: usize,
    /// Server-side blocks still rendering.
    pub pending: usize,
    /// `<!-- slide -->` markers and reveal.js options.
    pub slides: usize,
    pub deck: Value,
}

/// An export asked for by the editor/CLI; the preview page builds the snapshot.
pub struct ExportJob {
    pub id: u64,
    pub format: String,
    pub file: PathBuf,
    pub result: Option<Result<PathBuf, String>>,
    /// A preview page has received the request.
    pub delivered: bool,
}

#[derive(Default)]
pub struct State {
    pub docs: HashMap<PathBuf, String>,
    pub roots: Vec<PathBuf>,
    pub target: Option<PathBuf>,
    pub view: View,
    pub dirty: bool,
    pub dirty_since: Option<Instant>,
    /// Editor cursor (0-based line) for scroll sync, and a counter that changes on every move.
    pub cursor_line: Option<usize>,
    pub cursor_seq: u64,
    pub active_polls: usize,
    pub client_visible: bool,
    pub newest_tab: String,
    pub newest_born: u64,
    pub last_seen: Option<Instant>,
    pub disk_mtime: Option<SystemTime>,
    pub shutdown: bool,
    pub exports: Vec<ExportJob>,
    pub export_seq: u64,
}

pub type Notifier = Box<dyn Fn(Value) + Send + Sync>;

pub struct App {
    pub mode: Mode,
    settings: RwLock<Settings>,
    pub token: String,
    pub port: AtomicU16,
    pub state: Mutex<State>,
    /// Signalled whenever the view or the cursor changes (long-polling clients wait on it).
    pub changed: Condvar,
    /// Signalled whenever there is render work.
    pub work: Condvar,
    /// Sends an LSP message to the editor (only in LSP mode).
    pub notify: Mutex<Option<Notifier>>,
    pub next_request: AtomicU64,
    pub started: Instant,
    /// Slow server-side renders (PlantUML, Kroki, code chunks).
    pub jobs: Arc<crate::jobs::Jobs>,
    /// "Run code chunks" was allowed in the preview for this session.
    pub scripts_allowed: AtomicBool,
    /// The preview browser plays H.264/AAC (reported by the page).
    pub h264: AtomicBool,
}

impl App {
    pub fn new(mode: Mode, settings: Settings) -> Arc<App> {
        let app = Arc::new(App {
            mode,
            settings: RwLock::new(settings),
            token: util::random_token(),
            port: AtomicU16::new(0),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            work: Condvar::new(),
            notify: Mutex::new(None),
            next_request: AtomicU64::new(1),
            started: Instant::now(),
            jobs: crate::jobs::Jobs::new(),
            scripts_allowed: AtomicBool::new(false),
            h264: AtomicBool::new(true),
        });
        // A finished background job: re-render so the result shows up.
        let weak = Arc::downgrade(&app);
        app.jobs.set_on_done(Box::new(move || {
            if let Some(a) = weak.upgrade() {
                let mut st = a.state.lock().unwrap();
                st.dirty = true;
                st.dirty_since = Some(Instant::now() - Duration::from_secs(10));
                a.work.notify_all();
            }
        }));
        app
    }

    pub fn allow_scripts(&self) {
        self.scripts_allowed.store(true, Ordering::SeqCst);
        self.refresh();
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn set_settings(&self, s: Settings) {
        *self.settings.write().unwrap() = s;
        let mut st = self.state.lock().unwrap();
        self.mark_dirty(&mut st);
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port.load(Ordering::SeqCst), self.token)
    }

    pub fn text_of(&self, st: &State, path: &Path) -> Option<String> {
        st.docs.get(path).cloned().or_else(|| std::fs::read_to_string(path).ok())
    }

    pub fn bump(&self, st: &mut State) {
        st.view.version += 1;
        self.changed.notify_all();
    }

    pub fn mark_dirty(&self, st: &mut State) {
        st.dirty = true;
        st.dirty_since = Some(Instant::now());
        self.work.notify_all();
    }

    pub fn client_connected(st: &State) -> bool {
        st.active_polls > 0 || st.last_seen.map(|t| t.elapsed() < Duration::from_secs(4)).unwrap_or(false)
    }

    /// Sends an LSP notification or request to the editor.
    pub fn send(&self, v: Value) {
        if let Some(n) = self.notify.lock().unwrap().as_ref() {
            n(v)
        }
    }

    // ------------------------------------------------------------ documents

    pub fn doc_opened(&self, path: PathBuf, text: String) {
        let mut st = self.state.lock().unwrap();
        st.docs.insert(path.clone(), text);
        if st.target.as_ref() == Some(&path) {
            self.mark_dirty(&mut st);
        }
    }

    pub fn doc_changed(&self, path: PathBuf, text: String) {
        let mut st = self.state.lock().unwrap();
        st.docs.insert(path.clone(), text);
        if st.target.as_ref() == Some(&path) {
            self.mark_dirty(&mut st);
        }
    }

    pub fn doc_saved(&self, _path: PathBuf) {
        // Imported files (CSV, other Markdown) may have changed too.
        let mut st = self.state.lock().unwrap();
        if st.target.is_some() {
            self.mark_dirty(&mut st);
        }
    }

    pub fn doc_closed(&self, path: PathBuf) {
        let mut st = self.state.lock().unwrap();
        st.docs.remove(&path);
    }

    /// Cursor moved in the editor (via textDocument/documentHighlight).
    pub fn cursor(&self, path: PathBuf, line: usize) {
        let s = self.settings();
        let mut st = self.state.lock().unwrap();
        if !App::client_connected(&st) {
            return;
        }
        if st.target.as_ref() != Some(&path) {
            if !s.follow_editor {
                return;
            }
            st.target = Some(path);
            st.disk_mtime = None;
            self.mark_dirty(&mut st);
        }
        if s.scroll_sync && st.cursor_line != Some(line) {
            st.cursor_line = Some(line);
            st.cursor_seq += 1;
            self.changed.notify_all();
        }
    }

    // ------------------------------------------------------------ preview

    /// Points the preview at `path` and opens a browser unless a visible preview is
    /// already connected. Returns (url, opened_browser).
    pub fn open_preview(&self, path: PathBuf, line: Option<usize>, force_open: bool) -> (String, bool) {
        let path = norm_path(&path);
        let mut st = self.state.lock().unwrap();
        st.target = Some(path);
        st.disk_mtime = None;
        if let Some(l) = line {
            st.cursor_line = Some(l);
            st.cursor_seq += 1;
        }
        st.view.focus += 1;
        self.mark_dirty(&mut st);
        let reuse = App::client_connected(&st) && st.client_visible;
        drop(st);
        let url = self.url();
        let s = self.settings();
        let open = force_open || !(s.reuse_preview && reuse);
        if open {
            util::open_url(&url, &s.browser.argv());
        }
        (url, open)
    }

    /// A link to another Markdown file was clicked in the preview.
    pub fn follow_link(&self, path: PathBuf) {
        let path = norm_path(&path);
        {
            let mut st = self.state.lock().unwrap();
            st.target = Some(path.clone());
            st.disk_mtime = None;
            st.cursor_line = None;
            self.mark_dirty(&mut st);
        }
        if self.mode == Mode::Lsp && path.is_file() {
            let id = self.next_request.fetch_add(1, Ordering::SeqCst);
            self.send(json!({
                "jsonrpc": "2.0", "id": format!("ml-{id}"), "method": "window/showDocument",
                "params": {"uri": path_to_uri(&path), "takeFocus": true}
            }));
        }
    }

    pub fn refresh(&self) {
        let mut st = self.state.lock().unwrap();
        self.mark_dirty(&mut st);
    }

    pub fn view_json(&self, st: &State, include_html: bool) -> Value {
        let v = &st.view;
        let file = v.file.as_ref();
        json!({
            "version": v.version,
            "file": file.map(|f| f.to_string_lossy().to_string()),
            "fileName": file.and_then(|f| f.file_name()).map(|f| f.to_string_lossy().to_string()),
            "title": v.title,
            "html": if include_html { Some(&v.html) } else { None },
            "headings": if include_html { Some(&v.headings) } else { None },
            "renderMs": v.render_ms as u64,
            "blockedChunks": v.blocked_chunks,
            "slides": v.slides,
            "deck": v.deck,
            "exportRequest": st.exports.iter().find(|e| e.result.is_none() && st.view.file.as_ref() == Some(&e.file)).map(|e| json!({"id": e.id, "format": e.format, "flavour": crate::export::flavour(&e.format)})),
            "pending": v.pending,
            "focus": v.focus,
            "cursorLine": st.cursor_line,
            "cursorSeq": st.cursor_seq,
            "mode": if self.mode == Mode::Lsp { "lsp" } else { "standalone" },
            "newestTab": st.newest_tab,
            "settings": self.settings().client_json(),
            "version_server": env!("CARGO_PKG_VERSION"),
        })
    }

    // ------------------------------------------------------------ render worker

    pub fn start_worker(self: &Arc<Self>) {
        let app = self.clone();
        std::thread::spawn(move || app.worker());
    }

    fn worker(self: Arc<Self>) {
        loop {
            let mut st = self.state.lock().unwrap();
            loop {
                if st.shutdown {
                    return;
                }
                if st.dirty {
                    break;
                }
                // Watch the file on disk when it isn't open in the editor.
                if let Some(t) = st.target.clone() {
                    if !st.docs.contains_key(&t) && App::client_connected(&st) {
                        let m = std::fs::metadata(&t).and_then(|m| m.modified()).ok();
                        if m != st.disk_mtime {
                            st.disk_mtime = m;
                            st.dirty = true;
                            st.dirty_since = Some(Instant::now() - Duration::from_secs(10));
                            break;
                        }
                    }
                }
                st = self.work.wait_timeout(st, Duration::from_millis(400)).unwrap().0;
            }
            let delay = Duration::from_millis(self.settings().live_update_delay);
            if let Some(since) = st.dirty_since {
                let el = since.elapsed();
                if el < delay {
                    drop(st);
                    std::thread::sleep(delay - el);
                    continue;
                }
            }
            st.dirty = false;
            let Some(target) = st.target.clone() else { continue };
            let text = self.text_of(&st, &target).unwrap_or_else(|| format!("> Cannot read `{}`", target.display()));
            let roots = st.roots.clone();
            drop(st);
            let t0 = Instant::now();
            let settings = self.settings();
            let mut ctx = Ctx::new(&settings, target.clone(), roots);
            ctx.jobs = Some(self.jobs.clone());
            ctx.h264 = self.h264.load(Ordering::SeqCst);
            ctx.allow_scripts = settings.enable_script_execution || self.scripts_allowed.load(Ordering::SeqCst);
            self.jobs.begin(&target);
            let r = markdown::render(&text, &ctx);
            let (blocked, pending) = (ctx.blocked_chunks.get(), ctx.pending.get());
            let (slides, deck) = (r.slides, r.deck.clone());
            let ms = t0.elapsed().as_millis();
            let mut st = self.state.lock().unwrap();
            if st.target.as_ref() != Some(&target) {
                continue; // switched meanwhile
            }
            st.view.file = Some(target);
            st.view.html = r.html;
            st.view.title = r.title;
            st.view.headings = r.headings;
            st.view.render_ms = ms;
            st.view.blocked_chunks = blocked;
            st.view.pending = pending;
            st.view.slides = slides;
            st.view.deck = deck;
            self.bump(&mut st);
        }
    }

    // ------------------------------------------------------------ v4: slides and exports

    /// Inserts `<!-- slide -->` at the start of `line` (in the editor through LSP, else in the file).
    pub fn insert_slide(&self, path: &Path, line: usize) -> Result<(), String> {
        let marker = "<!-- slide -->\n\n";
        let open = self.state.lock().unwrap().docs.contains_key(path);
        if self.mode == Mode::Lsp && open {
            let id = self.next_request.fetch_add(1, Ordering::SeqCst);
            let pos = json!({"line": line, "character": 0});
            self.send(json!({
                "jsonrpc": "2.0", "id": format!("ml-{id}"), "method": "workspace/applyEdit",
                "params": {"label": "Insert new slide", "edit": {"changes": {path_to_uri(path): [{"range": {"start": pos, "end": pos}, "newText": marker}]}}}
            }));
            return Ok(());
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut lines: Vec<&str> = text.split('\n').collect();
        let at = line.min(lines.len());
        lines.insert(at, "<!-- slide -->\n");
        std::fs::write(path, lines.join("\n")).map_err(|e| e.to_string())?;
        self.refresh();
        Ok(())
    }

    /// Asks the preview page for a snapshot and converts it. Opens a hidden headless
    /// browser when no preview is connected. Blocks until done (or 3 minutes).
    pub fn export(&self, path: &Path, format: &str) -> Result<PathBuf, String> {
        if !crate::export::FORMATS.contains(&format) {
            return Err(format!("unknown format {format}; use one of {}", crate::export::FORMATS.join(", ")));
        }
        let path = norm_path(path);
        let (id, connected) = {
            let mut st = self.state.lock().unwrap();
            st.export_seq += 1;
            let id = st.export_seq;
            st.exports.push(ExportJob { id, format: format.to_string(), file: path.clone(), result: None, delivered: false });
            if st.target.as_ref() != Some(&path) {
                st.target = Some(path.clone());
                st.disk_mtime = None;
                self.mark_dirty(&mut st);
            }
            self.bump(&mut st);
            (id, App::client_connected(&st))
        };
        // A page that looks connected may be gone (closed tab): if nobody picks the
        // request up within a few seconds, render it in a hidden browser.
        let mut headless = None;
        let picked_up = connected && {
            let until = Instant::now() + Duration::from_secs(4);
            let mut st = self.state.lock().unwrap();
            loop {
                if st.exports.iter().any(|e| e.id == id && (e.delivered || e.result.is_some())) {
                    break true;
                }
                if Instant::now() >= until {
                    break false;
                }
                st = self.changed.wait_timeout(st, Duration::from_millis(200)).unwrap().0;
            }
        };
        if !picked_up {
            headless = Some(crate::export_page::open_headless(&self.settings(), &self.url())?);
        }
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut st = self.state.lock().unwrap();
        let res = loop {
            if let Some(i) = st.exports.iter().position(|e| e.id == id && e.result.is_some()) {
                break st.exports.remove(i).result.unwrap();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || st.shutdown {
                st.exports.retain(|e| e.id != id);
                break Err("the preview page did not answer (is it open?)".into());
            }
            st = self.changed.wait_timeout(st, left.min(Duration::from_millis(500))).unwrap().0;
        };
        drop(st);
        if let Some(mut h) = headless {
            let _ = h.kill();
            let _ = h.wait();
        }
        res
    }

    /// The page delivered a snapshot: convert it.
    pub fn export_snapshot(&self, id: Option<u64>, format: &str, snapshot: &str) -> Result<PathBuf, String> {
        let file = {
            let st = self.state.lock().unwrap();
            match id.and_then(|id| st.exports.iter().find(|e| e.id == id)) {
                Some(e) => e.file.clone(),
                None => st.view.file.clone().ok_or("nothing to export")?,
            }
        };
        let res = crate::export::convert(&self.settings(), &file, format, snapshot);
        if let Some(id) = id {
            let mut st = self.state.lock().unwrap();
            if let Some(e) = st.exports.iter_mut().find(|e| e.id == id) {
                e.result = Some(res.clone());
            }
            self.changed.notify_all();
        }
        res
    }

    pub fn shutdown(&self) {
        self.jobs.shutdown();
        let mut st = self.state.lock().unwrap();
        st.shutdown = true;
        self.work.notify_all();
        self.changed.notify_all();
        drop(st);
        let _ = std::fs::remove_file(util::instances_dir().join(format!("{}.json", std::process::id())));
    }
}
