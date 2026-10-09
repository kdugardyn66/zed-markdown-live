//! Extra CLI commands added by later versions (export, insert-slide…).

use crate::app::App;
use crate::config::Settings;

pub fn usage_lines() -> &'static str {
    ""
}

/// Runs `cmd`; returns false when it isn't an extra command.
pub fn run(_cmd: &str, _args: &[String]) -> bool {
    false
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
    let py = crate::util::which("python3").map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into());
    println!("ffmpeg        : {}", crate::media::ffmpeg().map(|p| p.display().to_string()).unwrap_or_else(|| "not found — needed to convert MKV/AVI/… videos (brew install ffmpeg)".into()));
    println!("code chunks   : {} (python: {py})", if s.enable_script_execution { "enabled" } else { "off — enable with \"enableScriptExecution\": true or per session in the preview" });
}
