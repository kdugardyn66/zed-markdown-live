//! Extra HTTP routes added by later versions (exports, code chunks, media).

use crate::app::App;
use std::sync::Arc;
use crate::web::Request;

pub fn handle_extra(app: &Arc<App>, req: Request, path: &str, _qs: &[(String, String)]) -> std::io::Result<()> {
    if req.method == "GET" {
        if let Some(rest) = path.strip_prefix("/media/") {
            return media(app, req, &std::path::PathBuf::from(crate::util::percent_decode(&format!("/{rest}"))));
        }
    }
    if req.method == "POST" && path == "/api/client" {
        // the page tells what it can play
        let v = crate::http::read_json(&req);
        if let Some(h) = v.get("h264").and_then(|b| b.as_bool()) {
            if app.h264.swap(h, std::sync::atomic::Ordering::SeqCst) != h {
                app.refresh();
            }
        }
        return req.respond(crate::http::json_resp(200, &serde_json::json!({"ok": true})));
    }
    req.respond(crate::http::not_found())
}

/// Converted pictures/videos (HEIC → JPEG, MKV/AVI → MP4); the original when no conversion applies.
fn media(app: &Arc<App>, req: Request, file: &std::path::Path) -> std::io::Result<()> {
    use crate::media as m;
    if !crate::http::allowed(app, file) || !file.is_file() {
        return req.respond(crate::http::not_found());
    }
    let ext = crate::markdown::ext_of(&file.to_string_lossy());
    if m::IMAGE_CONVERT.contains(&ext.as_str()) {
        return match m::convert_image(file) {
            Ok(out) => crate::http::serve_path(req, &out, crate::http::mime(&out.to_string_lossy())),
            Err(e) => req.respond(crate::web::Response::text(500, &e)),
        };
    }
    if m::VIDEO.contains(&ext.as_str()) {
        let plan = m::video_plan(file, app.h264.load(std::sync::atomic::Ordering::SeqCst));
        if let Some(t) = m::video_target(file, &plan).filter(|t| t.is_file()) {
            return crate::http::serve_path(req, &t, crate::http::mime(&t.to_string_lossy()));
        }
        // a QuickTime file with H.264 plays when it's announced as MP4
        let ct = if ext == "mov" { "video/mp4" } else { crate::http::mime(&file.to_string_lossy()) };
        return crate::http::serve_path(req, file, ct);
    }
    crate::http::serve_file(app, req, file)
}
