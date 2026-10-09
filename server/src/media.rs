//! Pictures and videos the browser can't show by itself.
//!
//! - HEIC/HEIF and TIFF pictures are converted to JPEG/PNG (macOS `sips`, ImageMagick,
//!   `heif-convert` or ffmpeg — whichever is installed).
//! - MKV/AVI/… videos are remuxed (fast, no quality loss) or transcoded to MP4/WebM
//!   with ffmpeg, in the background.
//!
//! Results are cached in `<cache>/markdown-live/media/`, keyed by path, size and mtime.

use crate::diagrams::run_process;
use crate::util;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

pub const IMAGE_NATIVE: &[&str] = &["png", "jpg", "jpeg", "jfif", "pjpeg", "gif", "svg", "webp", "avif", "bmp", "ico", "apng"];
pub const IMAGE_CONVERT: &[&str] = &["heic", "heif", "tif", "tiff"];
pub const VIDEO: &[&str] = &["mp4", "m4v", "webm", "ogv", "mov", "mkv", "avi", "wmv", "flv", "mpg", "mpeg", "3gp", "ts", "mts", "m2ts"];
pub const AUDIO: &[&str] = &["mp3", "wav", "ogg", "oga", "m4a", "aac", "flac", "opus"];

pub fn is_image(ext: &str) -> bool {
    IMAGE_NATIVE.contains(&ext) || IMAGE_CONVERT.contains(&ext)
}

fn tool(name: &str) -> Option<PathBuf> {
    util::which(name).or_else(|| {
        ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].iter().map(|d| Path::new(d).join(name)).find(|p| p.is_file())
    })
}

pub fn ffmpeg() -> Option<PathBuf> {
    tool("ffmpeg")
}

fn media_dir() -> PathBuf {
    util::cache_dir().join("media")
}

/// Cache file for a converted copy of `src`.
pub fn cache_path(src: &Path, ext: &str) -> PathBuf {
    let meta = std::fs::metadata(src).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let key = crate::markdown::hash(&format!("{}|{size}|{mtime}", src.display()));
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let stem: String = stem.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').take(40).collect();
    media_dir().join(format!("{stem}-{key}.{ext}"))
}

// ------------------------------------------------------------------ pictures

/// Last resort: Pillow (+ pillow-heif for HEIC), when installed.
const PIL_CONVERT: &str = "import sys\ntry:\n    import pillow_heif; pillow_heif.register_heif_opener()\nexcept ImportError:\n    pass\nfrom PIL import Image, ImageOps\nim = ImageOps.exif_transpose(Image.open(sys.argv[1]))\nout = sys.argv[2]\nif out.endswith('.jpg'):\n    im = im.convert('RGB')\nim.save(out)\n";

/// Converts a HEIC/TIFF picture to something the browser shows. Returns the cached copy.
pub fn convert_image(src: &Path) -> Result<PathBuf, String> {
    let ext = crate::markdown::ext_of(&src.to_string_lossy());
    let out_ext = if ext.starts_with("tif") { "png" } else { "jpg" };
    let out = cache_path(src, out_ext);
    if out.is_file() {
        return Ok(out);
    }
    std::fs::create_dir_all(media_dir()).map_err(|e| e.to_string())?;
    let tmp = out.with_extension(format!("part.{out_ext}"));
    let mut tried = Vec::new();
    let attempts: Vec<(&str, Vec<String>)> = vec![
        ("sips", vec!["-s".into(), "format".into(), if out_ext == "png" { "png".into() } else { "jpeg".into() }, src.to_string_lossy().into(), "--out".into(), tmp.to_string_lossy().into()]),
        ("magick", vec![format!("{}[0]", src.display()), "-auto-orient".into(), tmp.to_string_lossy().into()]),
        ("convert", vec![format!("{}[0]", src.display()), "-auto-orient".into(), tmp.to_string_lossy().into()]),
        ("heif-convert", vec![src.to_string_lossy().into(), tmp.to_string_lossy().into()]),
        ("ffmpeg", vec!["-v".into(), "error".into(), "-y".into(), "-i".into(), src.to_string_lossy().into(), "-frames:v".into(), "1".into(), tmp.to_string_lossy().into()]),
        ("python3", vec!["-c".into(), PIL_CONVERT.into(), src.to_string_lossy().into(), tmp.to_string_lossy().into()]),
    ];
    for (name, args) in attempts {
        if name == "sips" && !cfg!(target_os = "macos") {
            continue;
        }
        if name == "heif-convert" && out_ext == "png" {
            continue;
        }
        let Some(bin) = tool(name) else { continue };
        let mut cmd = Command::new(bin);
        cmd.args(&args);
        match run_process(cmd, None, Duration::from_secs(60)) {
            Ok(o) if o.status == Some(0) && tmp.is_file() && std::fs::metadata(&tmp).map(|m| m.len() > 0).unwrap_or(false) => {
                std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
            Ok(o) => tried.push(format!("{name}: {}", o.stderr.lines().last().unwrap_or("failed").trim())),
            Err(e) => tried.push(format!("{name}: {e}")),
        }
        let _ = std::fs::remove_file(&tmp);
    }
    if tried.is_empty() {
        Err(format!(".{ext} pictures need a converter: macOS has `sips` built in; elsewhere install ImageMagick, libheif or ffmpeg"))
    } else {
        Err(format!("cannot convert {}: {}", src.display(), tried.join("; ")))
    }
}

// ------------------------------------------------------------------ videos

#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// The browser plays the file as it is.
    Direct,
    /// Same streams, new container (seconds).
    Remux(&'static str),
    /// Re-encode: H.264/AAC MP4, or VP9/Opus WebM for browsers without H.264 (can take a while).
    Transcode(&'static str),
    /// No ffmpeg: serve as is and hope.
    NoTool,
}

static PLANS: Mutex<Option<HashMap<(PathBuf, bool), (u64, Plan)>>> = Mutex::new(None);

fn probe(src: &Path) -> Option<(String, String)> {
    let ffprobe = tool("ffprobe")?;
    let mut cmd = Command::new(ffprobe);
    cmd.args(["-v", "error", "-show_entries", "stream=codec_type,codec_name", "-of", "csv=p=0"]).arg(src);
    let out = run_process(cmd, None, Duration::from_secs(20)).ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut v = String::new();
    let mut a = String::new();
    for line in text.lines() {
        let mut p = line.split(',');
        let (name, kind) = (p.next().unwrap_or(""), p.next().unwrap_or(""));
        if kind == "video" && v.is_empty() && name != "mjpeg" && name != "png" {
            v = name.to_string();
        } else if kind == "audio" && a.is_empty() {
            a = name.to_string();
        }
    }
    Some((v, a))
}

/// What has to happen before the browser can play `src`. `h264`: the browser plays
/// H.264/AAC (Google Chrome, Edge, Brave do; open-source Chromium builds don't).
pub fn video_plan(src: &Path, h264: bool) -> Plan {
    let ext = crate::markdown::ext_of(&src.to_string_lossy());
    if h264 && matches!(ext.as_str(), "mp4" | "m4v" | "webm" | "ogv") {
        return Plan::Direct;
    }
    if !h264 && matches!(ext.as_str(), "webm" | "ogv") {
        return Plan::Direct;
    }
    let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
    let key = (src.to_path_buf(), h264);
    if let Some((s, p)) = PLANS.lock().unwrap().as_ref().and_then(|m| m.get(&key).cloned()) {
        if s == size {
            return p;
        }
    }
    let plan = if ffmpeg().is_none() {
        Plan::NoTool
    } else {
        let fallback = if h264 { Plan::Transcode("mp4") } else { Plan::Transcode("webm") };
        match probe(src) {
            None => fallback,
            Some((v, a)) => {
                let audio_mp4 = a.is_empty() || matches!(a.as_str(), "aac" | "mp3" | "opus" | "flac" | "alac");
                let audio_webm = a.is_empty() || matches!(a.as_str(), "opus" | "vorbis");
                match v.as_str() {
                    "h264" | "hevc" if h264 && audio_mp4 => {
                        if matches!(ext.as_str(), "mov" | "mp4" | "m4v") && v == "h264" {
                            Plan::Direct
                        } else {
                            Plan::Remux("mp4")
                        }
                    }
                    "av1" if h264 && audio_mp4 => Plan::Remux("mp4"),
                    "vp8" | "vp9" | "av1" if audio_webm => Plan::Remux("webm"),
                    _ => fallback,
                }
            }
        }
    };
    PLANS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, (size, plan.clone()));
    plan
}

/// Where the playable copy of `src` is (or will be) cached.
pub fn video_target(src: &Path, plan: &Plan) -> Option<PathBuf> {
    match plan {
        Plan::Remux(ext) => Some(cache_path(src, ext)),
        Plan::Transcode(ext) => Some(cache_path(src, &format!("t.{ext}"))),
        _ => None,
    }
}

/// Creates the playable copy (runs in the background job queue).
pub fn convert_video(src: &Path, plan: &Plan) -> Result<PathBuf, String> {
    let out = video_target(src, plan).ok_or("nothing to convert")?;
    if out.is_file() {
        return Ok(out);
    }
    let ffmpeg = ffmpeg().ok_or("ffmpeg is not installed (macOS: brew install ffmpeg)")?;
    std::fs::create_dir_all(media_dir()).map_err(|e| e.to_string())?;
    let ext = out.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = out.with_extension(format!("part.{ext}"));
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-v", "error", "-y", "-i"]).arg(src);
    match plan {
        Plan::Remux(_) => {
            cmd.args(["-map", "0:v:0", "-map", "0:a:0?", "-c", "copy"]);
            if ext == "mp4" {
                cmd.args(["-tag:v", "hvc1", "-movflags", "+faststart"]);
            }
        }
        Plan::Transcode("webm") => {
            cmd.args([
                "-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1",
                "-b:v", "0", "-crf", "34", "-c:a", "libopus", "-b:a", "128k",
            ]);
        }
        _ => {
            cmd.args([
                "-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p",
                "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", "-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart",
            ]);
        }
    }
    cmd.arg(&tmp);
    let o = run_process(cmd, None, Duration::from_secs(3 * 3600))?;
    if o.status == Some(0) && tmp.is_file() {
        std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
        return Ok(out);
    }
    let _ = std::fs::remove_file(&tmp);
    // hvc1 tagging only applies to HEVC; retry a plain remux, then a transcode.
    if let Plan::Remux("mp4") = plan {
        let mut cmd = Command::new(crate::media::ffmpeg().unwrap());
        cmd.args(["-v", "error", "-y", "-i"]).arg(src).args(["-map", "0:v:0", "-map", "0:a:0?", "-c", "copy", "-movflags", "+faststart"]).arg(&tmp);
        if let Ok(o2) = run_process(cmd, None, Duration::from_secs(3600)) {
            if o2.status == Some(0) && tmp.is_file() {
                std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
        let _ = std::fs::remove_file(&tmp);
        return convert_video(src, &Plan::Transcode("mp4")).map(|p| {
            let _ = std::fs::copy(&p, &out);
            out
        });
    }
    Err(format!("ffmpeg could not convert {}: {}", src.display(), o.stderr.trim()))
}

/// `https://youtu.be/ID`, `youtube.com/watch?v=ID`, `vimeo.com/ID` → embeddable player URL.
pub fn embed_url(url: &str) -> Option<String> {
    let u = url.trim();
    let after = |pat: &str| u.find(pat).map(|i| &u[i + pat.len()..]);
    let id_of = |s: &str| s.split(['?', '&', '#', '/']).next().unwrap_or("").to_string();
    if let Some(r) = after("youtu.be/") {
        return Some(format!("https://www.youtube-nocookie.com/embed/{}", id_of(r)));
    }
    if u.contains("youtube.com/") {
        if let Some(r) = after("v=") {
            return Some(format!("https://www.youtube-nocookie.com/embed/{}", id_of(r)));
        }
        if let Some(r) = after("/embed/").or_else(|| after("/shorts/")) {
            return Some(format!("https://www.youtube-nocookie.com/embed/{}", id_of(r)));
        }
    }
    if let Some(r) = after("vimeo.com/") {
        let id = id_of(r);
        if id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty() {
            return Some(format!("https://player.vimeo.com/video/{id}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plans() {
        if ffmpeg().is_none() {
            return;
        }
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/media");
        assert_eq!(video_plan(&d.join("clip.mp4"), true), Plan::Direct);
        assert_eq!(video_plan(&d.join("clip.mov"), true), Plan::Direct);
        assert_eq!(video_plan(&d.join("clip.mkv"), true), Plan::Remux("mp4"));
        assert_eq!(video_plan(&d.join("clip.avi"), true), Plan::Transcode("mp4"));
        assert_eq!(video_plan(&d.join("clip.webm"), false), Plan::Direct);
        assert_eq!(video_plan(&d.join("clip.mp4"), false), Plan::Transcode("webm"));
        assert_eq!(video_plan(&d.join("clip.avi"), false), Plan::Transcode("webm"));
    }
    #[test]
    fn embeds() {
        assert_eq!(embed_url("https://youtu.be/abc123?t=5").unwrap(), "https://www.youtube-nocookie.com/embed/abc123");
        assert_eq!(embed_url("https://www.youtube.com/watch?v=XyZ&list=1").unwrap(), "https://www.youtube-nocookie.com/embed/XyZ");
        assert_eq!(embed_url("https://vimeo.com/76979871").unwrap(), "https://player.vimeo.com/video/76979871");
        assert!(embed_url("https://example.com/v.mp4").is_none());
    }
}
