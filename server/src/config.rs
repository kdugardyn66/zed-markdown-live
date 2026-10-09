//! User settings. In Zed they live under `lsp.markdown-live.settings`.

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(untagged)]
pub enum BrowserSetting {
    #[default]
    Default,
    One(String),
    Many(Vec<String>),
}

impl BrowserSetting {
    pub fn argv(&self) -> Vec<String> {
        match self {
            BrowserSetting::Default => vec![],
            BrowserSetting::One(s) if s.trim().is_empty() => vec![],
            BrowserSetting::One(s) => vec![s.clone()],
            BrowserSetting::Many(v) => v.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Browser command. Empty = Chromium/Chrome in app mode when found, else the system default.
    /// String or array; `{url}` is replaced by the preview URL. `"default"` = system browser.
    pub browser: BrowserSetting,
    /// Don't open a new browser tab when a visible preview tab is already connected.
    pub reuse_preview: bool,
    /// The preview scrolls to the editor's cursor.
    pub scroll_sync: bool,
    /// The preview switches to whichever Markdown file you're editing.
    pub follow_editor: bool,
    /// Debounce for live updates, in milliseconds.
    pub live_update_delay: u64,
    /// Preview server port. 0 = random free port.
    pub port: u16,
    /// `auto` (follow the OS), `light` or `dark`.
    pub theme: String,
    /// Front matter: `none` (hidden), `table` or `code`.
    pub front_matter: String,
    /// Treat single newlines as line breaks.
    pub breaks: bool,
    /// Smart quotes and dashes.
    pub typographer: bool,
    /// `:smile:` style emoji shortcodes.
    pub emoji: bool,
    /// Mermaid theme: `auto` (follows the preview theme), `default`, `dark`, `forest`, `neutral`.
    pub mermaid_theme: String,
    /// Maximum number of CSV rows rendered in a table (the rest is summarised).
    pub csv_max_rows: usize,
    /// `bundled` (default, offline) or `cdn` (jsDelivr; smaller pages, needs internet).
    pub libraries: String,
    /// Math engine: `katex` (fast) or `mathjax` (more complete LaTeX).
    pub math: String,
    /// PlantUML: `local` (Java + plantuml.jar, nothing leaves the machine), `online`
    /// (the `plantumlServer`), or `auto` (local when found, else online).
    pub plantuml_render: String,
    pub plantuml_server: String,
    /// Path to plantuml.jar. Empty = autodetect (also finds PlantUML Live's copy).
    pub plantuml_jar: String,
    /// Path to java. Empty = autodetect.
    pub java: String,
    /// Kroki server for ```kroki-<type>``` blocks and `{kroki=true}` (online, explicit opt-in per block).
    pub kroki_server: String,
    /// Run code chunks (```python {cmd=true}```). Off by default; the preview can allow it per session.
    pub enable_script_execution: bool,
    /// Timeout for one code chunk, in seconds.
    pub code_timeout: u64,
    /// Python interpreter. Empty = `.venv/bin/python` in the workspace, else `python3`.
    pub python: String,
    /// reveal.js theme when the front matter doesn't set one: `auto` (white/black by preview theme), `white`, `league`, …
    pub slide_theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            browser: BrowserSetting::Default,
            reuse_preview: true,
            scroll_sync: true,
            follow_editor: true,
            live_update_delay: 150,
            port: 0,
            theme: "auto".into(),
            front_matter: "none".into(),
            breaks: false,
            typographer: false,
            emoji: true,
            mermaid_theme: "auto".into(),
            csv_max_rows: 5000,
            libraries: "bundled".into(),
            math: "katex".into(),
            plantuml_render: "local".into(),
            plantuml_server: "https://www.plantuml.com/plantuml".into(),
            plantuml_jar: String::new(),
            java: String::new(),
            kroki_server: "https://kroki.io".into(),
            enable_script_execution: false,
            code_timeout: 30,
            python: String::new(),
            slide_theme: "auto".into(),
        }
    }
}

impl Settings {
    /// Builds settings from a JSON value; unknown keys are ignored, bad values fall back to defaults.
    pub fn from_json(v: &Value) -> Settings {
        let mut v = v.clone();
        if let Some(inner) = v.get("markdownLive").cloned() {
            v = inner;
        }
        let mut s: Settings = serde_json::from_value(v.clone()).unwrap_or_else(|e| {
            crate::log!("invalid settings ({e}); using defaults where needed");
            // Take every key that parses on its own.
            let mut d = serde_json::to_value(SettingsShadow::from(&Settings::default())).unwrap();
            if let (Some(o), Some(dm)) = (v.as_object(), d.as_object_mut()) {
                for (k, val) in o {
                    let mut probe = serde_json::Map::new();
                    probe.insert(k.clone(), val.clone());
                    if serde_json::from_value::<Settings>(Value::Object(probe)).is_ok() {
                        dm.insert(k.clone(), val.clone());
                    }
                }
            }
            serde_json::from_value(d).unwrap_or_default()
        });
        s.normalise();
        s
    }

    fn normalise(&mut self) {
        if self.live_update_delay < 20 {
            self.live_update_delay = 20;
        }
        if self.plantuml_render == "server" {
            self.plantuml_render = "online".into();
        }
        self.plantuml_server = self.plantuml_server.trim_end_matches('/').to_string();
        self.kroki_server = self.kroki_server.trim_end_matches('/').to_string();
        if self.code_timeout == 0 {
            self.code_timeout = 30;
        }
        if self.csv_max_rows == 0 {
            self.csv_max_rows = 5000;
        }
        for (field, allowed, default) in [
            (&mut self.theme, &["auto", "light", "dark"][..], "auto"),
            (&mut self.front_matter, &["none", "table", "code"][..], "none"),
            (&mut self.libraries, &["bundled", "cdn"][..], "bundled"),
            (&mut self.mermaid_theme, &["auto", "default", "dark", "forest", "neutral", "base"][..], "auto"),
            (&mut self.math, &["katex", "mathjax"][..], "katex"),
            (&mut self.plantuml_render, &["local", "online", "auto"][..], "local"),
        ] {
            let v = field.trim().to_ascii_lowercase();
            *field = if allowed.contains(&v.as_str()) { v } else { default.into() };
        }
    }

    /// The settings the browser needs.
    pub fn client_json(&self) -> Value {
        serde_json::json!({
            "theme": self.theme,
            "mermaidTheme": self.mermaid_theme,
            "scrollSync": self.scroll_sync,
            "math": self.math,
            "slideTheme": self.slide_theme,
        })
    }
}

/// Serialisable mirror used to rebuild settings key by key.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsShadow {
    reuse_preview: bool,
    scroll_sync: bool,
    follow_editor: bool,
    live_update_delay: u64,
    port: u16,
    theme: String,
    front_matter: String,
    breaks: bool,
    typographer: bool,
    emoji: bool,
    mermaid_theme: String,
    csv_max_rows: usize,
    libraries: String,
    math: String,
    plantuml_render: String,
    plantuml_server: String,
    plantuml_jar: String,
    java: String,
    kroki_server: String,
    enable_script_execution: bool,
    code_timeout: u64,
    python: String,
    slide_theme: String,
}

impl From<&Settings> for SettingsShadow {
    fn from(s: &Settings) -> Self {
        SettingsShadow {
            reuse_preview: s.reuse_preview,
            scroll_sync: s.scroll_sync,
            follow_editor: s.follow_editor,
            live_update_delay: s.live_update_delay,
            port: s.port,
            theme: s.theme.clone(),
            front_matter: s.front_matter.clone(),
            breaks: s.breaks,
            typographer: s.typographer,
            emoji: s.emoji,
            mermaid_theme: s.mermaid_theme.clone(),
            csv_max_rows: s.csv_max_rows,
            libraries: s.libraries.clone(),
            math: s.math.clone(),
            plantuml_render: s.plantuml_render.clone(),
            plantuml_server: s.plantuml_server.clone(),
            plantuml_jar: s.plantuml_jar.clone(),
            java: s.java.clone(),
            kroki_server: s.kroki_server.clone(),
            enable_script_execution: s.enable_script_execution,
            code_timeout: s.code_timeout,
            python: s.python.clone(),
            slide_theme: s.slide_theme.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse() {
        let s = Settings::from_json(&serde_json::json!({"browser":["chromium","--app={url}"],"liveUpdateDelay":100,"theme":"DARK"}));
        assert_eq!(s.browser.argv().len(), 2);
        assert_eq!(s.live_update_delay, 100);
        assert_eq!(s.theme, "dark");
        let bad = Settings::from_json(&serde_json::json!({"theme": 3, "port": 1234, "frontMatter": "table"}));
        assert_eq!(bad.port, 1234);
        assert_eq!(bad.theme, "auto");
        assert_eq!(bad.front_matter, "table");
    }
}
