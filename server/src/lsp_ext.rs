//! Extra code actions added by later versions (exports, slides, code chunks).

use crate::app::App;
use std::path::PathBuf;
use std::sync::Arc;

pub fn extra_commands() -> Vec<(&'static str, &'static str)> {
    vec![("markdownLive.runCodeChunks", "MarkDown Live: Run code chunks (this session)")]
}

/// Extra Zed tasks: (label, args JSON, reveal, optional key).
pub fn extra_tasks() -> Vec<(&'static str, &'static str, &'static str, Option<&'static str>)> {
    vec![]
}

pub fn execute(a: &Arc<App>, cmd: &str, _path: PathBuf, _line: Option<usize>) -> Result<(), String> {
    match cmd {
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
