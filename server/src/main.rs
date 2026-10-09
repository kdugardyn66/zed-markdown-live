//! markdown-live — Markdown language server and live browser preview for Zed.
//!
//! ```text
//! markdown-zed                        run the language server on stdio (what Zed starts)
//! markdown-zed preview FILE [--line N] [--new-window]
//!                                     open/refresh the browser preview (Option-M on macOS, Alt-M elsewhere)
//! markdown-zed render FILE            print the preview HTML (debugging)
//! markdown-zed serve                  standalone preview server (started automatically)
//! markdown-zed doctor                 show the detected setup
//! markdown-zed setup-zed [--key K]    add the preview task and key binding to Zed's config
//! ```

mod app;
mod cli_ext;
mod config;
mod csv;
mod diagrams;
mod jobs;
mod http;
mod http_ext;
mod lsp;
mod lsp_ext;
mod markdown;
mod setup;
mod util;
mod web;

use app::{App, Mode};
use config::Settings;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn usage() -> ! {
    eprintln!(
        "markdown-live {}\n\nUSAGE:\n  markdown-zed [--stdio]                  language server (used by Zed)\n  markdown-zed preview FILE [--line N] [--new-window]\n  markdown-zed render FILE                print the rendered HTML\n  markdown-zed serve                      standalone preview server\n  markdown-zed doctor                     show the detected setup\n  markdown-zed setup-zed [--key alt-m]    add the preview task + key binding to Zed\n{}\nEnvironment: MARKDOWN_LIVE_SETTINGS (JSON, same keys as the Zed settings), MARKDOWN_LIVE_CHROME",
        env!("CARGO_PKG_VERSION"),
        cli_ext::usage_lines()
    );
    std::process::exit(2)
}

pub fn env_settings() -> Settings {
    let v = std::env::var("MARKDOWN_LIVE_SETTINGS")
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(json!({}));
    Settings::from_json(&v)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "" | "--stdio" | "lsp" => lsp::run(),
        "preview" | "open" => cli_preview(&args[1..]),
        "render" => cli_render(&args[1..]),
        "serve" => serve(),
        "doctor" => doctor(),
        "setup-zed" => setup::run(&args[1..]),
        "--version" | "-V" | "version" => println!("markdown-live {}", env!("CARGO_PKG_VERSION")),
        "--help" | "-h" | "help" => usage(),
        other => {
            if !cli_ext::run(other, &args[1..]) {
                usage()
            }
        }
    }
}

pub struct FileArgs {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub new_window: bool,
    /// `-f/--format` and other `--key value` options, for the commands added later.
    pub opts: Vec<(String, String)>,
}

pub fn parse_file_args(args: &[String]) -> FileArgs {
    let mut parts: Vec<String> = Vec::new();
    let mut line = None;
    let mut new_window = false;
    let mut opts = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--line" | "-l" => {
                i += 1;
                // 1-based like $ZED_ROW
                line = args.get(i).and_then(|s| s.parse::<usize>().ok()).map(|l| l.saturating_sub(1));
            }
            "--new-window" | "--open" => new_window = true,
            "-f" | "--format" | "-o" | "--output" => {
                let k = if args[i].contains('f') { "format" } else { "output" };
                i += 1;
                if let Some(v) = args.get(i) {
                    opts.push((k.to_string(), v.clone()));
                }
            }
            s if s.starts_with("--") => opts.push((s.trim_start_matches('-').to_string(), "true".into())),
            // Some Zed versions split task arguments on spaces ("Mobile Documents"),
            // so collect every positional word and glue them back together below.
            s => parts.push(s.to_string()),
        }
        i += 1;
    }
    if parts.is_empty() {
        usage()
    }
    FileArgs { file: util::norm_path(&rejoin_path(&parts)), line, new_window, opts }
}

/// Rebuilds a path that was split on whitespace, and strips surrounding quotes.
fn rejoin_path(parts: &[String]) -> PathBuf {
    let clean = |s: &str| s.trim_matches(|c| c == '"' || c == '\'').to_string();
    let joined = clean(&parts.join(" "));
    if parts.len() == 1 || Path::new(&joined).exists() {
        return PathBuf::from(joined);
    }
    for n in (1..=parts.len()).rev() {
        let cand = clean(&parts[..n].join(" "));
        if Path::new(&cand).is_file() {
            return PathBuf::from(cand);
        }
    }
    PathBuf::from(joined)
}

pub struct Instance {
    pub port: u16,
    pub token: String,
    pub mode: String,
    pub files: Vec<String>,
}

pub fn agent() -> ureq::Agent {
    // Never route loopback traffic through a proxy.
    ureq::AgentBuilder::new().timeout(Duration::from_secs(5)).try_proxy_from_env(false).build()
}

pub fn live_instances() -> Vec<Instance> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(util::instances_dir()) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            let _ = std::fs::remove_file(&p);
            continue;
        };
        let port = v["port"].as_u64().unwrap_or(0) as u16;
        let token = v["token"].as_str().unwrap_or("").to_string();
        let url = format!("http://127.0.0.1:{port}/api/ping?t={token}");
        match agent().get(&url).call().ok().and_then(|r| r.into_string().ok()).and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
            Some(ping) => {
                let files = ping["files"].as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(String::from)).collect()).unwrap_or_default();
                out.push(Instance { port, token, mode: ping["mode"].as_str().unwrap_or("").to_string(), files });
            }
            None => {
                let _ = std::fs::remove_file(&p); // stale
            }
        }
    }
    out
}

/// The server that has `file` open in the editor, else any LSP server, else any.
pub fn pick_instance(file: &Path) -> Option<Instance> {
    let f = file.to_string_lossy().to_string();
    let mut all = live_instances();
    if let Some(i) = all.iter().position(|i| i.files.contains(&f)) {
        return Some(all.swap_remove(i));
    }
    if let Some(i) = all.iter().position(|i| i.mode == "lsp") {
        return Some(all.swap_remove(i));
    }
    all.into_iter().next()
}

pub fn spawn_standalone() -> Option<Instance> {
    let exe = std::env::current_exe().ok()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("serve").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008 | 0x00000200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    }
    let child = cmd.spawn().ok()?;
    let reg = util::instances_dir().join(format!("{}.json", child.id()));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if reg.is_file() {
            std::thread::sleep(Duration::from_millis(50));
            if let Ok(v) = std::fs::read_to_string(&reg).map(|t| serde_json::from_str::<Value>(&t).unwrap_or(Value::Null)) {
                if let (Some(port), Some(token)) = (v["port"].as_u64(), v["token"].as_str()) {
                    return Some(Instance { port: port as u16, token: token.into(), mode: "standalone".into(), files: vec![] });
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Shows a desktop notification (the preview task runs hidden, so errors would be invisible).
pub fn notify_error(msg: &str) {
    eprintln!("markdown-live: {msg}");
    let msg = msg.replace('"', "'");
    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("osascript").arg("-e").arg(format!("display notification \"{msg}\" with title \"MarkDown Live\"")).status();
    } else if cfg!(target_os = "linux") && util::which("notify-send").is_some() {
        let _ = std::process::Command::new("notify-send").arg("MarkDown Live").arg(&msg).status();
    }
}

fn cli_preview(args: &[String]) {
    let a = parse_file_args(args);
    if !a.file.is_file() {
        notify_error(&format!("no such file: {} (save the file first)", a.file.display()));
        std::process::exit(1);
    }
    let inst = pick_instance(&a.file).or_else(spawn_standalone);
    let Some(inst) = inst else {
        notify_error("could not start the preview server");
        std::process::exit(1);
    };
    let url = format!("http://127.0.0.1:{}/api/open?t={}", inst.port, inst.token);
    let body = json!({"path": a.file.to_string_lossy(), "line": a.line, "forceOpen": a.new_window});
    match agent().post(&url).set("Content-Type", "application/json").send_string(&body.to_string()) {
        Ok(r) => {
            let v: Value = r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            println!("MarkDown Live preview: {}", v["url"].as_str().unwrap_or(""));
        }
        Err(e) => {
            notify_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn cli_render(args: &[String]) {
    let a = parse_file_args(args);
    let text = std::fs::read_to_string(&a.file).unwrap_or_else(|e| {
        eprintln!("{}: {e}", a.file.display());
        std::process::exit(1)
    });
    let s = env_settings();
    let ctx = markdown::Ctx::new(&s, a.file.clone(), vec![]);
    print!("{}", markdown::render(&text, &ctx).html);
}

fn serve() {
    let app = App::new(Mode::Standalone, env_settings());
    if let Err(e) = http::start(&app) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    app.start_worker();
    // Exit when no browser has been connected for a while.
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let st = app.state.lock().unwrap();
        let idle = match st.last_seen {
            Some(t) => st.active_polls == 0 && t.elapsed() > Duration::from_secs(120),
            None => app.started.elapsed() > Duration::from_secs(120),
        };
        drop(st);
        if idle && cli_ext::may_exit(&app) {
            app.shutdown();
            std::process::exit(0);
        }
    }
}

fn doctor() {
    let s = env_settings();
    println!("markdown-live {}", env!("CARGO_PKG_VERSION"));
    let browser = s.browser.argv();
    println!(
        "browser       : {}",
        if browser.is_empty() {
            util::find_chromium().map(|p| format!("{} --app (chromeless window)", p.display())).unwrap_or_else(|| "system default (no Chromium/Chrome found)".into())
        } else {
            browser.join(" ")
        }
    );
    println!("libraries     : {} ({} embedded files)", s.libraries, http::embedded_count());
    println!("instances     : {}", util::instances_dir().display());
    for i in live_instances() {
        println!("  running     : port {} ({}), {} open file(s)", i.port, i.mode, i.files.len());
    }
    cli_ext::doctor(&s);
}
