//! Exports. The browser builds a self-contained snapshot of the rendered document
//! (diagrams, math and charts included; videos left out); the server turns it into:
//!
//! | format        | how                                                    |
//! |---------------|--------------------------------------------------------|
//! | `html`        | the snapshot itself                                    |
//! | `pdf`         | headless Chrome/Chromium prints the snapshot           |
//! | `docx`, `odt` | pandoc (math as real equations)                        |
//! | `slides-html` | a self-contained reveal.js deck                        |
//! | `slides-pdf`  | headless Chrome prints the deck, one slide per page    |
//! | `pptx`        | pandoc                                                 |
//! | `odp`         | pandoc → pptx → LibreOffice                            |

use crate::config::Settings;
use crate::diagrams::run_process;
use crate::util;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const FORMATS: &[&str] = &["html", "pdf", "docx", "odt", "slides-html", "slides-pdf", "pptx", "odp"];

pub fn label(format: &str) -> &'static str {
    match format {
        "html" => "HTML",
        "pdf" => "PDF",
        "docx" => "Word (.docx)",
        "odt" => "OpenDocument text (.odt)",
        "slides-html" => "slides as HTML (reveal.js)",
        "slides-pdf" => "slides as PDF",
        "pptx" => "PowerPoint (.pptx)",
        "odp" => "Impress (.odp)",
        _ => "?",
    }
}

/// Which snapshot the browser must build for a format.
pub fn flavour(format: &str) -> &'static str {
    match format {
        "html" | "pdf" => "web",
        "docx" | "odt" => "pandoc",
        "pptx" | "odp" => "pandoc-slides",
        _ => "deck",
    }
}

/// `notes.md` + `pdf` → `<exportDir>/notes.pdf`; slides get `notes.slides.pdf`.
pub fn output_path(s: &Settings, source: &Path, format: &str) -> PathBuf {
    let dir = source.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
    let dir = if s.export_dir.trim().is_empty() {
        dir
    } else {
        let p = PathBuf::from(s.export_dir.trim());
        if p.is_absolute() {
            p
        } else {
            dir.join(p)
        }
    };
    let stem = source.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into());
    let name = match format {
        "slides-html" => format!("{stem}.slides.html"),
        "slides-pdf" => format!("{stem}.slides.pdf"),
        f => format!("{stem}.{f}"),
    };
    dir.join(util::sanitize_file_name(&name))
}

fn tool(name: &str, mac_app: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = util::which(name) {
        return Some(p);
    }
    for d in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let p = Path::new(d).join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(app) = mac_app {
        let p = PathBuf::from(app);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn pandoc(s: &Settings) -> Option<PathBuf> {
    if !s.pandoc.is_empty() {
        return Some(PathBuf::from(&s.pandoc));
    }
    tool("pandoc", None)
}

pub fn soffice() -> Option<PathBuf> {
    tool("soffice", Some("/Applications/LibreOffice.app/Contents/MacOS/soffice")).or_else(|| tool("libreoffice", None))
}

pub fn chrome(s: &Settings) -> Option<PathBuf> {
    if !s.chrome.is_empty() {
        return Some(PathBuf::from(&s.chrome));
    }
    util::find_chromium()
}

/// A throw-away work folder.
fn work_dir() -> Result<PathBuf, String> {
    let d = std::env::temp_dir().join(format!("markdown-live-export-{}-{}", std::process::id(), util::random_token()));
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Prints an HTML file to PDF with headless Chrome/Chromium.
pub fn print_pdf(s: &Settings, html: &Path, out: &Path) -> Result<(), String> {
    let chrome = chrome(s).ok_or("PDF export needs Chrome, Chromium, Edge or Brave (or set \"chrome\" in the settings)")?;
    let profile = html.parent().unwrap().join("chrome-profile");
    let mut cmd = Command::new(chrome);
    cmd.args([
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--no-pdf-header-footer",
        "--run-all-compositor-stages-before-draw",
        "--virtual-time-budget=20000",
        // Keep Chrome from calling home while it prints.
        "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE localhost",
    ]);
    if cfg!(target_os = "linux") && std::env::var_os("MARKDOWN_LIVE_NO_SANDBOX").is_some() {
        cmd.arg("--no-sandbox");
    }
    cmd.arg(format!("--user-data-dir={}", profile.display()));
    cmd.arg(format!("--print-to-pdf={}", out.display()));
    cmd.arg(util::path_to_uri(html));
    let o = run_process(cmd, None, Duration::from_secs(120))?;
    if out.is_file() && std::fs::metadata(out).map(|m| m.len() > 0).unwrap_or(false) {
        Ok(())
    } else {
        Err(format!("Chrome could not print the PDF: {}", o.stderr.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("no output")))
    }
}

fn run_pandoc(s: &Settings, input: &Path, to: &str, out: &Path, extra: &[String], resource_dir: &Path) -> Result<(), String> {
    let pandoc = pandoc(s).ok_or("Word/OpenDocument/PowerPoint export needs pandoc (macOS: brew install pandoc)")?;
    let mut cmd = Command::new(pandoc);
    cmd.arg(input).args(["-f", "html", "-t", to, "-o"]).arg(out);
    cmd.arg(format!("--resource-path={}", resource_dir.display()));
    let reference = if to == "pptx" { &s.reference_pptx } else if to == "docx" { &s.reference_docx } else if to == "odt" { &s.reference_odt } else { &String::new() };
    if !reference.is_empty() {
        cmd.arg(format!("--reference-doc={reference}"));
    }
    cmd.args(extra);
    let o = run_process(cmd, None, Duration::from_secs(180))?;
    if o.status == Some(0) && out.is_file() {
        Ok(())
    } else {
        Err(format!("pandoc failed: {}", o.stderr.trim()))
    }
}

/// Turns a snapshot (from the browser) into the requested file. Returns the written path.
pub fn convert(s: &Settings, source: &Path, format: &str, snapshot: &str) -> Result<PathBuf, String> {
    if !FORMATS.contains(&format) {
        return Err(format!("unknown export format {format}"));
    }
    let out = output_path(s, source, format);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let resource_dir = source.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
    let work = work_dir()?;
    let result = (|| -> Result<(), String> {
        match format {
            "html" | "slides-html" => std::fs::write(&out, snapshot).map_err(|e| e.to_string()),
            "pdf" | "slides-pdf" => {
                let html = work.join("export.html");
                std::fs::write(&html, snapshot).map_err(|e| e.to_string())?;
                let tmp = work.join("out.pdf");
                print_pdf(s, &html, &tmp)?;
                std::fs::copy(&tmp, &out).map(|_| ()).map_err(|e| e.to_string())
            }
            "docx" | "odt" => {
                let html = work.join("export.html");
                std::fs::write(&html, snapshot).map_err(|e| e.to_string())?;
                let tmp = work.join(format!("out.{format}"));
                run_pandoc(s, &html, format, &tmp, &["--standalone".into()], &resource_dir)?;
                std::fs::copy(&tmp, &out).map(|_| ()).map_err(|e| e.to_string())
            }
            "pptx" | "odp" => {
                let html = work.join("export.html");
                std::fs::write(&html, snapshot).map_err(|e| e.to_string())?;
                let pptx = work.join("slides.pptx");
                run_pandoc(s, &html, "pptx", &pptx, &["--slide-level=2".into()], &resource_dir)?;
                if format == "pptx" {
                    return std::fs::copy(&pptx, &out).map(|_| ()).map_err(|e| e.to_string());
                }
                let office = soffice().ok_or("Impress (.odp) export needs LibreOffice (macOS: brew install --cask libreoffice)")?;
                let mut cmd = Command::new(office);
                cmd.args(["--headless", "--norestore", "--convert-to", "odp", "--outdir"]).arg(&work).arg(&pptx);
                cmd.env("HOME", &work); // its own profile, so a running LibreOffice doesn't block it
                let o = run_process(cmd, None, Duration::from_secs(180))?;
                let odp = work.join("slides.odp");
                if !odp.is_file() {
                    return Err(format!("LibreOffice could not convert to .odp: {}", o.stderr.trim()));
                }
                std::fs::copy(&odp, &out).map(|_| ()).map_err(|e| e.to_string())
            }
            _ => unreachable!(),
        }
    })();
    let _ = std::fs::remove_dir_all(&work);
    result.map(|_| out)
}

/// Lines for `markdown-zed doctor`.
pub fn doctor(s: &Settings) {
    let show = |p: Option<PathBuf>, hint: &str| p.map(|p| p.display().to_string()).unwrap_or_else(|| format!("not found — {hint}"));
    println!("export pdf    : {}", show(chrome(s), "install Chrome or Chromium"));
    println!("export docx/odt/pptx : {}", show(pandoc(s), "brew install pandoc"));
    println!("export odp    : {}", show(soffice(), "brew install --cask libreoffice"));
}
