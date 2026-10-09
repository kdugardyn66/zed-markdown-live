//! Extra code actions added by later versions (exports, slides, code chunks).

use crate::app::App;
use std::path::PathBuf;
use std::sync::Arc;

pub fn extra_commands() -> Vec<(&'static str, &'static str)> {
    vec![]
}

/// Extra Zed tasks: (label, args JSON, reveal, optional key).
pub fn extra_tasks() -> Vec<(&'static str, &'static str, &'static str, Option<&'static str>)> {
    vec![]
}

pub fn execute(_a: &Arc<App>, cmd: &str, _path: PathBuf, _line: Option<usize>) -> Result<(), String> {
    Err(format!("unknown command {cmd}"))
}

pub fn done_message(_cmd: &str) -> Option<String> {
    None
}
