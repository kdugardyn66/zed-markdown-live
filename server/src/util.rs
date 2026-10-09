//! Small helpers: paths/URIs, UTF-16 columns, PATH lookup, browser launching.
#![allow(dead_code)]

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        eprintln!("[markdown-live] {}", format!($($arg)*));
    }};
}

/// 128 bits of randomness without extra crates (RandomState is seeded from the OS).
pub fn random_token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i ^ std::process::id() as u64);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Per-user cache directory for this tool.
pub fn cache_dir() -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library").join("Caches"))
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("markdown-live")
}

pub fn instances_dir() -> PathBuf {
    cache_dir().join("instances")
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn percent_encode_path(s: &str) -> String {
    let mut out = String::new();
    for &c in s.as_bytes() {
        let keep = c.is_ascii_alphanumeric() || b"/-_.~:".contains(&c);
        if keep {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

/// `file:///Users/x/a%20b.puml` -> `/Users/x/a b.puml` (also `file:///C:/x` on Windows).
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let decoded = percent_decode(rest);
    if cfg!(windows) {
        let d = decoded.trim_start_matches('/');
        Some(PathBuf::from(d.replace('/', "\\")))
    } else {
        Some(PathBuf::from(decoded))
    }
}

pub fn path_to_uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{}", percent_encode_path(&s))
    } else {
        format!("file:///{}", percent_encode_path(&s))
    }
}

/// Normalises a path for use as a map key (absolute, canonical when possible).
pub fn norm_path(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf())
        }
    })
}

/// Byte offset in `line` for a UTF-16 column (LSP default position encoding).
pub fn utf16_to_byte(line: &str, col16: usize) -> usize {
    let mut n = 0;
    for (i, ch) in line.char_indices() {
        if n >= col16 {
            return i;
        }
        n += ch.len_utf16();
    }
    line.len()
}

pub fn byte_to_utf16(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().map(|c| c.len_utf16()).sum()
}

/// Looks a program up on PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() || name.contains('/') || name.contains('\\') {
        return if p.is_file() { Some(p.to_path_buf()) } else { None };
    }
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        vec![".exe".into(), ".cmd".into(), ".bat".into(), String::new()]
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// Finds a Chromium-based browser (Chromium, Chrome, Brave, Edge). Used for the
/// default chromeless preview window and for PDF export.
pub fn find_chromium() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MARKDOWN_LIVE_CHROME").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let mut cands: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        for app in ["Chromium", "Google Chrome", "Brave Browser", "Microsoft Edge", "Google Chrome Canary"] {
            let rel = format!("{app}.app/Contents/MacOS/{app}");
            cands.push(PathBuf::from("/Applications").join(&rel));
            if let Some(h) = home_dir() {
                cands.push(h.join("Applications").join(&rel));
            }
        }
    } else if cfg!(windows) {
        for base in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            if let Some(b) = std::env::var_os(base).map(PathBuf::from) {
                cands.push(b.join("Chromium\\Application\\chrome.exe"));
                cands.push(b.join("Google\\Chrome\\Application\\chrome.exe"));
                cands.push(b.join("Microsoft\\Edge\\Application\\msedge.exe"));
                cands.push(b.join("BraveSoftware\\Brave-Browser\\Application\\brave.exe"));
            }
        }
    }
    if let Some(p) = cands.into_iter().find(|p| p.is_file()) {
        return Some(p);
    }
    ["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "brave-browser", "microsoft-edge", "chrome"]
        .iter()
        .find_map(|n| which(n))
}

fn spawn_detached(argv: &[String]) -> std::io::Result<()> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
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
    cmd.spawn().map(|_| ())
}

/// Opens `url` in a browser. `browser` may be empty (Chromium in app mode when
/// installed, else the system default), `"default"` (system default), a command
/// line (`"firefox"`, `"chromium --app={url}"`) or an argv array.
pub fn open_url(url: &str, browser: &[String]) {
    let mut argv: Vec<String> = if browser.is_empty() {
        match find_chromium() {
            Some(c) => vec![c.to_string_lossy().to_string(), "--app={url}".into()],
            None => Vec::new(),
        }
    } else if browser.len() == 1 && browser[0].trim().eq_ignore_ascii_case("default") {
        Vec::new()
    } else if browser.len() == 1 {
        shell_split(&browser[0])
    } else {
        browser.to_vec()
    };
    let result = if argv.is_empty() {
        let argv: Vec<String> = if cfg!(target_os = "macos") {
            vec!["open".into(), url.into()]
        } else if cfg!(windows) {
            vec!["cmd".into(), "/C".into(), "start".into(), String::new(), url.into()]
        } else {
            vec!["xdg-open".into(), url.into()]
        };
        spawn_detached(&argv)
    } else {
        if !argv.iter().any(|a| a.contains("{url}")) {
            // macOS app names: "Safari", "Google Chrome" -> open -a <app> <url>
            if cfg!(target_os = "macos") && argv.len() == 1 && which(&argv[0]).is_none() {
                argv = vec!["open".into(), "-a".into(), argv[0].clone()];
            }
            argv.push(url.to_string());
        }
        let argv: Vec<String> = argv.iter().map(|a| a.replace("{url}", url)).collect();
        spawn_detached(&argv)
    };
    if let Err(e) = result {
        crate::log!("could not open browser: {e}");
    }
}

/// Minimal shell-like splitting with "double" and 'single' quotes.
pub fn shell_split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                has = true;
            }
            None if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            None => cur.push(c),
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn html_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#160;", " ")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// Turns `diagram name.puml` + extension into a file name that doesn't contain separators.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "/\\:*?\"<>|\0".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        "diagram".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uri_roundtrip() {
        if !cfg!(windows) {
            let p = PathBuf::from("/tmp/a b/c#d.puml");
            let u = path_to_uri(&p);
            assert_eq!(u, "file:///tmp/a%20b/c%23d.puml");
            assert_eq!(uri_to_path(&u).unwrap(), p);
        }
    }
    #[test]
    fn split() {
        assert_eq!(shell_split("chromium --app={url}"), vec!["chromium", "--app={url}"]);
        assert_eq!(shell_split("\"Google Chrome\" x"), vec!["Google Chrome", "x"]);
    }
    #[test]
    fn utf16() {
        let l = "a😀b";
        assert_eq!(utf16_to_byte(l, 3), 5);
        assert_eq!(byte_to_utf16(l, 5), 3);
    }
}
