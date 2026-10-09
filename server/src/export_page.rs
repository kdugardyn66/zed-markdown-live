//! A hidden browser for exports started from Zed or the CLI when no preview is open:
//! headless Chrome loads the preview page, which builds the snapshot and posts it back.

use crate::config::Settings;
use std::process::{Child, Command, Stdio};

pub fn open_headless(s: &Settings, url: &str) -> Result<Child, String> {
    let chrome = crate::export::chrome(s).ok_or("no preview is open and no Chrome/Chromium was found to render one; open the preview (Option-M) and try again")?;
    let profile = std::env::temp_dir().join(format!("markdown-live-headless-{}-{}", std::process::id(), crate::util::random_token()));
    let mut cmd = Command::new(chrome);
    cmd.args([
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--remote-debugging-port=0",
        "--window-size=1280,900",
        "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE localhost, EXCLUDE 127.0.0.1",
    ]);
    if cfg!(target_os = "linux") && std::env::var_os("MARKDOWN_LIVE_NO_SANDBOX").is_some() {
        cmd.arg("--no-sandbox");
    }
    cmd.arg(format!("--user-data-dir={}", profile.display())).arg(url);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("cannot start headless browser: {e}"))
}
