//! Extra code actions added by later versions (printing, slides, code chunks).

use crate::app::App;
use std::path::PathBuf;
use std::sync::Arc;

pub fn extra_commands() -> Vec<(&'static str, &'static str)> {
    vec![
        ("markdownLive.insertSlide", "MarkDown Live: Insert new slide"),
        ("markdownLive.print", "MarkDown Live: Print"),
        ("markdownLive.runCodeChunks", "MarkDown Live: Run code chunks (this session)"),
    ]
}

/// Extra Zed tasks: (label, args JSON, reveal, optional key).
pub fn extra_tasks() -> Vec<(&'static str, &'static str, &'static str, Option<&'static str>)> {
    vec![
        ("MarkDown Live: Insert new slide", r#"["insert-slide", "$ZED_FILE", "--line", "$ZED_ROW"]"#, "never", Some("alt-shift-n")),
        ("MarkDown Live: Print", r#"["print", "$ZED_FILE"]"#, "never", Some("alt-shift-p")),
    ]
}

pub fn execute(a: &Arc<App>, cmd: &str, path: PathBuf, line: Option<usize>) -> Result<(), String> {
    match cmd {
        "markdownLive.insertSlide" => a.insert_slide(&path, line.unwrap_or(0)),
        "markdownLive.print" => {
            a.print(&path);
            Ok(())
        }
        "markdownLive.runCodeChunks" => {
            a.allow_scripts();
            Ok(())
        }
        _ => Err(format!("unknown command {cmd}")),
    }
}

pub fn done_message(cmd: &str) -> Option<String> {
    match cmd {
        "markdownLive.runCodeChunks" => Some("MarkDown Live: code chunks run for this session (set enableScriptExecution to make it permanent).".into()),
        _ => None,
    }
}
