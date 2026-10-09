//! Local preview web server (127.0.0.1 only, token-protected).

use crate::app::App;
use crate::log;
use crate::util::{self, percent_decode};
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::web::{Request, Response};

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

pub fn start(app: &Arc<App>) -> Result<u16, String> {
    let app2 = app.clone();
    let port = crate::web::serve(app.settings().port, move |req| handle(&app2, req)).map_err(|e| format!("cannot start preview server: {e}"))?;
    app.port.store(port, Ordering::SeqCst);
    // Register this instance so `markdown-zed preview <file>` can find it.
    let dir = util::instances_dir();
    let _ = std::fs::create_dir_all(&dir);
    let info = json!({
        "pid": std::process::id(),
        "port": port,
        "token": app.token,
        "mode": if app.mode == crate::app::Mode::Lsp { "lsp" } else { "standalone" },
    });
    let file = dir.join(format!("{}.json", std::process::id()));
    if let Err(e) = write_private(&file, info.to_string().as_bytes()) {
        log!("cannot write {}: {e}", file.display());
    }
    log!("preview server on http://127.0.0.1:{port}");
    Ok(port)
}

fn write_private(path: &PathBuf, data: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.write_all(data)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, data)
    }
}

pub fn json_resp(code: u16, v: &Value) -> Response {
    Response::bytes(code, v.to_string().into_bytes())
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Cache-Control", "no-store")
}

pub fn not_found() -> Response {
    Response::text(404, "not found")
}

fn query(url: &str) -> Vec<(String, String)> {
    let q = url.split_once('?').map(|x| x.1).unwrap_or("");
    q.split('&')
        .filter(|s| !s.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (percent_decode(&k.replace('+', " ")), percent_decode(&v.replace('+', " ")))
        })
        .collect()
}

fn q<'a>(qs: &'a [(String, String)], k: &str) -> Option<&'a str> {
    qs.iter().find(|(a, _)| a == k).map(|(_, b)| b.as_str())
}

fn req_header(req: &Request, name: &str) -> Option<String> {
    req.header(name).map(|s| s.to_string())
}

fn cookie_token(req: &Request) -> Option<String> {
    let c = req_header(req, "Cookie")?;
    c.split(';').find_map(|part| part.trim().strip_prefix("mlt=").map(|v| v.to_string()))
}

fn host_ok(req: &Request, port: u16) -> bool {
    match req_header(req, "Host") {
        None => false,
        Some(h) => {
            let h = h.to_ascii_lowercase();
            h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}") || h == format!("[::1]:{port}")
        }
    }
}

pub fn mime(path: &str) -> &'static str {
    match crate::markdown::ext_of(path).as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" | "jfif" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "tif" | "tiff" => "image/tiff",
        "heic" | "heif" => "image/heic",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "ogv" => "video/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        "csv" => "text/csv; charset=utf-8",
        "md" | "markdown" | "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Returns an embedded asset (decompressed).
pub fn embedded(path: &str) -> Option<Vec<u8>> {
    let gz = EMBEDDED.iter().find(|(p, _)| *p == path)?.1;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(gz).read_to_end(&mut out).ok()?;
    Some(out)
}

pub fn embedded_text(path: &str) -> String {
    embedded(path).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default()
}

fn csp(app: &App, nonce: &str) -> String {
    let cdn = if app.settings().libraries == "cdn" { " https://cdn.jsdelivr.net" } else { "" };
    format!(
        "default-src 'self'; script-src 'self' 'nonce-{nonce}'{cdn}; style-src 'self' 'unsafe-inline'{cdn}; \
         font-src 'self' data:{cdn}; img-src 'self' data: blob: https: http:; media-src 'self' blob: https: http:; \
         connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    )
}

fn handle(app: &Arc<App>, req: Request) {
    let port = app.port.load(Ordering::SeqCst);
    if !host_ok(&req, port) {
        let _ = req.respond(Response::text(403, "bad host"));
        return;
    }
    let url = req.url.clone();
    if std::env::var_os("MARKDOWN_LIVE_DEBUG").is_some() {
        log!("{} {}", req.method, url.split('?').next().unwrap_or(""));
    }
    let path = url.split('?').next().unwrap_or("/").to_string();
    let qs = query(&url);
    let token_ok = q(&qs, "t") == Some(app.token.as_str()) || cookie_token(&req).as_deref() == Some(app.token.as_str());
    if !token_ok {
        let _ = req.respond(Response::text(403, "forbidden: open the preview from the editor"));
        return;
    }
    let method = req.method.clone();
    let result = match (method.as_str(), path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            let nonce = util::random_token();
            let html = embedded_text("assets/viewer.html").replace("{{NONCE}}", &nonce).replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
            let r = Response::bytes(200, html)
                .header("Content-Type", "text/html; charset=utf-8")
                .header("Cache-Control", "no-store")
                .header("Set-Cookie", &format!("mlt={}; HttpOnly; SameSite=Strict; Path=/", app.token))
                .header("Content-Security-Policy", &csp(app, &nonce));
            req.respond(r)
        }
        ("GET", p) if p.starts_with("/assets/") || p.starts_with("/vendor/") => serve_embedded(app, req, &p[1..]),
        ("GET", p) if p.starts_with("/fs/") => {
            let file = PathBuf::from(percent_decode(&p[3..]));
            serve_file(app, req, &file)
        }
        ("GET", "/api/ping") => {
            let st = app.state.lock().unwrap();
            let files: Vec<String> = st.docs.keys().map(|p| p.to_string_lossy().to_string()).collect();
            let v = json!({"pid": std::process::id(), "mode": if app.mode == crate::app::Mode::Lsp {"lsp"} else {"standalone"}, "files": files});
            drop(st);
            req.respond(json_resp(200, &v))
        }
        ("GET", "/api/state") => {
            let since: u64 = q(&qs, "since").and_then(|s| s.parse().ok()).unwrap_or(0);
            let cseq: u64 = q(&qs, "cseq").and_then(|s| s.parse().ok()).unwrap_or(u64::MAX);
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut st = app.state.lock().unwrap();
            st.active_polls += 1;
            st.last_seen = Some(Instant::now());
            let tab = q(&qs, "tab").unwrap_or("").to_string();
            let born: u64 = q(&qs, "born").and_then(|s| s.parse().ok()).unwrap_or(0);
            if !tab.is_empty() && born >= st.newest_born && tab != st.newest_tab {
                st.newest_born = born;
                st.newest_tab = tab.clone();
                st.view.version += 1; // tell older tabs to retire
                app.changed.notify_all();
            }
            if tab.is_empty() || tab == st.newest_tab {
                st.client_visible = q(&qs, "visible") != Some("0");
            }
            while st.view.version <= since && (cseq == u64::MAX || st.cursor_seq == cseq) && !st.shutdown {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                st = app.changed.wait_timeout(st, left).unwrap().0;
            }
            st.active_polls -= 1;
            st.last_seen = Some(Instant::now());
            let v = app.view_json(&st, st.view.version > since);
            drop(st);
            req.respond(json_resp(200, &v))
        }
        ("POST", "/api/open") => {
            let body = read_json(&req);
            let Some(p) = body.get("path").and_then(|v| v.as_str()) else {
                let _ = req.respond(json_resp(400, &json!({"error": "path required"})));
                return;
            };
            let line = body.get("line").and_then(|v| v.as_u64()).map(|l| l as usize);
            let force = body.get("forceOpen").and_then(|v| v.as_bool()).unwrap_or(false);
            let (url, opened) = app.open_preview(PathBuf::from(p), line, force);
            req.respond(json_resp(200, &json!({"url": url, "opened": opened})))
        }
        ("POST", "/api/open-link") => {
            // body: {"path": "/abs/file.md"} (from a /fs/ link in the preview)
            let body = read_json(&req);
            let p = body.get("path").and_then(|v| v.as_str()).map(PathBuf::from);
            match p {
                Some(p) if allowed(app, &p) && p.is_file() => {
                    app.follow_link(p);
                    req.respond(json_resp(200, &json!({"ok": true})))
                }
                _ => req.respond(json_resp(404, &json!({"error": "not found"}))),
            }
        }
        ("POST", "/api/refresh") => {
            app.refresh();
            req.respond(json_resp(200, &json!({"ok": true})))
        }
        _ => crate::http_ext::handle_extra(app, req, &path, &qs),
    };
    if let Err(e) = result {
        log!("http: {e}");
    }
}

fn serve_embedded(app: &Arc<App>, req: Request, rel: &str) -> std::io::Result<()> {
    if rel.starts_with("vendor/") && app.settings().libraries == "cdn" {
        if let Some((_, url)) = CDN.iter().find(|(p, _)| *p == rel) {
            return req.respond(Response::bytes(302, Vec::new()).header("Location", url));
        }
    }
    let Some((_, gz)) = EMBEDDED.iter().find(|(p, _)| *p == rel) else {
        return req.respond(not_found());
    };
    let accepts_gzip = req_header(&req, "Accept-Encoding").map(|v| v.contains("gzip")).unwrap_or(false);
    let ct = mime(rel);
    let cache = "public, max-age=86400";
    if accepts_gzip {
        req.respond(Response::bytes(200, gz.to_vec()).header("Content-Type", ct).header("Content-Encoding", "gzip").header("Cache-Control", cache))
    } else {
        let data = embedded(rel).unwrap_or_default();
        req.respond(Response::bytes(200, data).header("Content-Type", ct).header("Cache-Control", cache))
    }
}

/// Local files the preview may read: inside a workspace root, or next to (or one
/// level above) an open/previewed document. Hidden folders are refused.
pub fn allowed(app: &App, p: &Path) -> bool {
    let Ok(canon) = std::fs::canonicalize(p) else { return false };
    if canon.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') && s != "." && s != ".."
    }) {
        return false;
    }
    let st = app.state.lock().unwrap();
    let mut roots: Vec<PathBuf> = st.roots.clone();
    for d in st.docs.keys().chain(st.target.iter()) {
        if let Some(dir) = d.parent() {
            roots.push(dir.to_path_buf());
            if let Some(up) = dir.parent() {
                if up.parent().is_some() {
                    roots.push(up.to_path_buf());
                }
            }
        }
    }
    drop(st);
    roots.iter().filter_map(|r| std::fs::canonicalize(r).ok()).any(|r| canon.starts_with(&r))
}

pub fn serve_file(app: &Arc<App>, req: Request, file: &Path) -> std::io::Result<()> {
    if !allowed(app, file) || !file.is_file() {
        return req.respond(not_found());
    }
    let ct = mime(&file.to_string_lossy());
    serve_path(req, file, ct)
}

/// Serves a file with HTTP Range support (needed for seeking in videos).
pub fn serve_path(req: Request, file: &Path, ct: &str) -> std::io::Result<()> {
    let mut f = match std::fs::File::open(file) {
        Ok(f) => f,
        Err(_) => return req.respond(not_found()),
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let range = req_header(&req, "Range").and_then(|r| {
        let r = r.strip_prefix("bytes=")?;
        let (a, b) = r.split_once('-')?;
        let (start, end) = if a.is_empty() {
            let n: u64 = b.parse().ok()?;
            (len.saturating_sub(n), len.saturating_sub(1))
        } else {
            let s: u64 = a.parse().ok()?;
            let e: u64 = if b.is_empty() { len.saturating_sub(1) } else { b.parse().ok()? };
            (s, e.min(len.saturating_sub(1)))
        };
        if start > end || start >= len {
            None
        } else {
            Some((start, end))
        }
    });
    let r = match range {
        Some((start, end)) => {
            f.seek(SeekFrom::Start(start))?;
            let n = end - start + 1;
            Response::reader(206, Box::new(f), n).header("Content-Range", &format!("bytes {start}-{end}/{len}"))
        }
        None => Response::reader(200, Box::new(f), len),
    };
    req.respond(r.header("Content-Type", ct).header("Accept-Ranges", "bytes").header("Cache-Control", "no-cache"))
}

pub fn read_json(req: &Request) -> Value {
    serde_json::from_slice(&req.body).unwrap_or(Value::Null)
}

pub fn embedded_count() -> usize {
    EMBEDDED.len()
}
