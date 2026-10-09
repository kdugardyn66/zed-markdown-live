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
pub fn may_exit(_app: &App) -> bool {
    true
}

pub fn doctor(_s: &Settings) {}
