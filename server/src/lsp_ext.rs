//! Extra code actions added by later versions (exports, slides, code chunks).

use crate::app::App;
use std::path::PathBuf;
use std::sync::Arc;

pub fn extra_commands() -> Vec<(&'static str, &'static str)> {
    vec![
        ("markdownLive.insertSlide", "MarkDown Live: Insert new slide"),
        ("markdownLive.export.pdf", "MarkDown Live: Export document → PDF"),
        ("markdownLive.export.docx", "MarkDown Live: Export document → Word (.docx)"),
        ("markdownLive.export.odt", "MarkDown Live: Export document → OpenDocument (.odt)"),
        ("markdownLive.export.html", "MarkDown Live: Export document → HTML"),
        ("markdownLive.export.pptx", "MarkDown Live: Export slides → PowerPoint (.pptx)"),
        ("markdownLive.export.odp", "MarkDown Live: Export slides → Impress (.odp)"),
        ("markdownLive.export.slides-pdf", "MarkDown Live: Export slides → PDF"),
        ("markdownLive.export.slides-html", "MarkDown Live: Export slides → HTML (reveal.js)"),
        ("markdownLive.runCodeChunks", "MarkDown Live: Run code chunks (this session)"),
    ]
}

/// Extra Zed tasks: (label, args JSON, reveal, optional key).
pub fn extra_tasks() -> Vec<(&'static str, &'static str, &'static str, Option<&'static str>)> {
    vec![
        ("MarkDown Live: Insert new slide", r#"["insert-slide", "$ZED_FILE", "--line", "$ZED_ROW"]"#, "never", Some("alt-shift-n")),
        ("MarkDown Live: Export PDF", r#"["export", "$ZED_FILE", "-f", "pdf"]"#, "no_focus", Some("alt-shift-e")),
        ("MarkDown Live: Export Word (.docx)", r#"["export", "$ZED_FILE", "-f", "docx"]"#, "no_focus", None),
        ("MarkDown Live: Export OpenDocument (.odt)", r#"["export", "$ZED_FILE", "-f", "odt"]"#, "no_focus", None),
        ("MarkDown Live: Export HTML", r#"["export", "$ZED_FILE", "-f", "html"]"#, "no_focus", None),
        ("MarkDown Live: Export slides PowerPoint (.pptx)", r#"["export", "$ZED_FILE", "-f", "pptx"]"#, "no_focus", None),
        ("MarkDown Live: Export slides Impress (.odp)", r#"["export", "$ZED_FILE", "-f", "odp"]"#, "no_focus", None),
        ("MarkDown Live: Export slides PDF", r#"["export", "$ZED_FILE", "-f", "slides-pdf"]"#, "no_focus", None),
        ("MarkDown Live: Export slides HTML", r#"["export", "$ZED_FILE", "-f", "slides-html"]"#, "no_focus", None),
    ]
}

pub fn execute(a: &Arc<App>, cmd: &str, path: PathBuf, line: Option<usize>) -> Result<(), String> {
    if let Some(format) = cmd.strip_prefix("markdownLive.export.") {
        let out = a.export(&path, format)?;
        a.send(serde_json::json!({"jsonrpc": "2.0", "method": "window/showMessage", "params": {"type": 3, "message": format!("MarkDown Live: {} saved: {}", crate::export::label(format), out.display())}}));
        return Ok(());
    }
    match cmd {
        "markdownLive.insertSlide" => a.insert_slide(&path, line.unwrap_or(0)),
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
