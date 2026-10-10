//! Extra CLI commands added by later versions (print, insert-slide…).

use crate::app::App;
use crate::config::Settings;

pub fn usage_lines() -> &'static str {
    "  markdown-zed print FILE                 open the preview and show the print dialog\n  markdown-zed insert-slide FILE --line N insert <!-- slide --> (in Zed: through the language server)\n"
}

/// Runs `cmd`; returns false when it isn't an extra command.
pub fn run(cmd: &str, args: &[String]) -> bool {
    match cmd {
        "print" => cli_print(args),
        "insert-slide" => cli_insert_slide(args),
        _ => return false,
    }
    true
}

fn instance_for(file: &std::path::Path) -> crate::Instance {
    match crate::pick_instance(file).or_else(crate::spawn_standalone) {
        Some(i) => i,
        None => {
            crate::notify_error("could not start the preview server");
            std::process::exit(1)
        }
    }
}

fn post(inst: &crate::Instance, path: &str, body: serde_json::Value, timeout: u64) -> Result<serde_json::Value, String> {
    let url = format!("http://127.0.0.1:{}{path}?t={}", inst.port, inst.token);
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(timeout)).try_proxy_from_env(false).build();
    match agent.post(&url).set("Content-Type", "application/json").send_string(&body.to_string()) {
        Ok(r) => r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).ok_or_else(|| "bad answer".to_string()),
        Err(ureq::Error::Status(_, r)) => {
            let v: serde_json::Value = r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            Err(v["error"].as_str().unwrap_or("failed").to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn cli_print(args: &[String]) {
    let a = crate::parse_file_args(args);
    if !a.file.is_file() {
        crate::notify_error(&format!("no such file: {}", a.file.display()));
        std::process::exit(1);
    }
    let inst = instance_for(&a.file);
    if let Err(e) = post(&inst, "/api/request-print", serde_json::json!({"path": a.file.to_string_lossy()}), 10) {
        crate::notify_error(&format!("print failed: {e}"));
        std::process::exit(1);
    }
}

fn cli_insert_slide(args: &[String]) {
    let a = crate::parse_file_args(args);
    let inst = instance_for(&a.file);
    if let Err(e) = post(&inst, "/api/insert-slide", serde_json::json!({"path": a.file.to_string_lossy(), "line": a.line.unwrap_or(0)}), 10) {
        crate::notify_error(&e);
        std::process::exit(1);
    }
}

/// Whether an idle standalone server may exit (false while background jobs run).
pub fn may_exit(app: &App) -> bool {
    app.jobs.busy() == 0
}

pub fn doctor(s: &Settings) {
    use crate::diagrams;
    println!("math          : {}", s.math);
    println!("plantuml      : {}", diagrams::plantuml_backend(s).unwrap_or_else(|e| format!("not available — {e}")));
    println!("  java        : {}", diagrams::find_java(s).map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into()));
    println!("  jar         : {}", diagrams::find_jar(s).map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into()));
    println!("kroki         : {} (only for ```kroki-…``` blocks)", s.kroki_server);
    let py = if s.python.trim().is_empty() {
        format!("{} (or the workspace .venv)", crate::util::which("python3").map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into()))
    } else {
        s.python.clone()
    };
    println!("ffmpeg        : {}", crate::media::ffmpeg().map(|p| p.display().to_string()).unwrap_or_else(|| "not found — needed to convert MKV/AVI/… videos (brew install ffmpeg)".into()));
    println!("code chunks   : {} (python: {py})", if s.enable_script_execution { "enabled" } else { "off — enable with \"enableScriptExecution\": true or per session in the preview" });
}
