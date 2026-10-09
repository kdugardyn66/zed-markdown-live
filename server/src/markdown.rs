//! Markdown → HTML.
//!
//! Pipeline:
//! 1. a line-preserving pre-pass (`@import`, `[TOC]`, `!!! admonitions`) so that
//!    `data-sourcepos` still points at the user's lines (scroll sync);
//! 2. comrak parses GFM + extensions into an AST;
//! 3. the AST is rewritten: special code fences (mermaid, math, csv…) become
//!    HTML blocks, relative image/link URLs point at the preview server;
//! 4. comrak renders HTML. Mermaid, math and code highlighting are finished in the browser.

use crate::config::Settings;
use crate::csv::{self, CsvOptions};
use crate::util;
use comrak::nodes::{AstNode, NodeHtmlBlock, NodeValue};
use comrak::{Anchorizer, Arena, Options};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, serde::Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub id: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub html: String,
    pub title: Option<String>,
    pub headings: Vec<Heading>,
}

/// Everything the renderer needs to know besides the text.
pub struct Ctx<'s> {
    pub settings: &'s Settings,
    /// The Markdown file (may not exist on disk yet).
    pub file: PathBuf,
    /// Workspace roots: `@import "/x"` resolves against the first one.
    pub roots: Vec<PathBuf>,
    /// Emit `data-sourcepos` (off for imported files).
    pub sourcepos: bool,
    /// Nesting level of `@import "x.md"`.
    pub depth: usize,
}

impl<'s> Ctx<'s> {
    pub fn base_dir(&self) -> PathBuf {
        self.file.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir)
    }
    fn child(&self, file: PathBuf) -> Ctx<'s> {
        Ctx { settings: self.settings, file, roots: self.roots.clone(), sourcepos: false, depth: self.depth + 1 }
    }
}

// ------------------------------------------------------------------ helpers

pub fn escape_html(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// FNV-1a, used to key client-side caches of rendered diagrams.
pub fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn has_scheme(url: &str) -> bool {
    let u = url.trim();
    if u.starts_with('#') || u.starts_with("//") {
        return true;
    }
    match u.find(':') {
        Some(i) if i > 1 => u[..i].chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)),
        _ => false,
    }
}

/// Resolves a path written in the document (relative to `base`, or `/x` = workspace root).
pub fn resolve(ctx: &Ctx, raw: &str) -> PathBuf {
    let raw = raw.trim();
    let raw = util::percent_decode(raw.split(['?', '#']).next().unwrap_or(raw));
    let raw = if let Some(rest) = raw.strip_prefix("file://") { rest.to_string() } else { raw };
    let raw = if let Some(rest) = raw.strip_prefix("~/") {
        util::home_dir().map(|h| h.join(rest).to_string_lossy().to_string()).unwrap_or(raw.clone())
    } else {
        raw
    };
    let p = Path::new(&raw);
    if p.is_absolute() {
        if p.exists() {
            return p.to_path_buf();
        }
        if let Some(root) = ctx.roots.first() {
            let cand = root.join(raw.trim_start_matches('/'));
            if cand.exists() {
                return cand;
            }
        }
        return p.to_path_buf();
    }
    normalize(&ctx.base_dir().join(p))
}

/// Removes `.` and `..` without touching the file system.
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// URL under which the preview server serves a local file.
pub fn fs_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = if s.starts_with('/') { s } else { format!("/{s}") };
    format!("/fs{}", util::percent_encode_path(&s))
}

/// Rewrites a URL from the document so the browser can load it.
pub fn rewrite_url(ctx: &Ctx, url: &str) -> String {
    if url.trim().is_empty() || has_scheme(url) {
        return url.to_string();
    }
    let frag = url.find('#').map(|i| url[i..].to_string()).unwrap_or_default();
    format!("{}{}", fs_url(&resolve(ctx, url)), frag)
}

/// Rewrites relative `src="…"` attributes inside raw HTML.
fn rewrite_raw_html(ctx: &Ctx, html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(i) = ["src=\"", "src='"].iter().filter_map(|p| lower.find(p).map(|i| (i, p.len()))).min() else {
            out.push_str(rest);
            break;
        };
        let (start, plen) = i;
        // only real attributes (preceded by whitespace)
        let ok = start > 0 && rest.as_bytes()[start - 1].is_ascii_whitespace();
        let q = rest.as_bytes()[start + plen - 1] as char;
        out.push_str(&rest[..start + plen]);
        rest = &rest[start + plen..];
        let end = rest.find(q).unwrap_or(rest.len());
        let val = &rest[..end];
        if ok {
            out.push_str(&rewrite_url(ctx, &util::html_unescape(val)).replace('"', "%22"));
        } else {
            out.push_str(val);
        }
        rest = &rest[end..];
    }
    out
}

/// Parses `lang {a=1 b="x y" .cls #id flag}` (MPE / Pandoc style) into (lang, attributes).
pub fn parse_info(info: &str) -> (String, Vec<(String, String)>) {
    let info = info.trim();
    let (lang, rest) = if info.starts_with('{') {
        // ```{python cmd=true}
        let inner = info.trim_start_matches('{').trim_end_matches('}');
        let mut it = inner.splitn(2, char::is_whitespace);
        (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string())
    } else {
        let mut it = info.splitn(2, char::is_whitespace);
        (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string())
    };
    let rest = rest.trim().trim_start_matches('{').trim_end_matches('}');
    (lang.trim_matches(|c| c == '{' || c == '}').to_ascii_lowercase(), parse_attrs(rest))
}

pub fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    loop {
        while chars.peek().map(|c| c.is_whitespace() || *c == ',').unwrap_or(false) {
            chars.next();
        }
        let Some(&c0) = chars.peek() else { break };
        let mut key = String::new();
        if c0 == '.' || c0 == '#' {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == ',' {
                    break;
                }
                key.push(c);
                chars.next();
            }
            out.push((if c0 == '.' { "class".into() } else { "id".into() }, key));
            continue;
        }
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == '=' || c == ',' || c == ':' {
                break;
            }
            key.push(c);
            chars.next();
        }
        if key.is_empty() {
            chars.next();
            continue;
        }
        if matches!(chars.peek(), Some('=') | Some(':')) {
            chars.next();
            let mut val = String::new();
            match chars.peek() {
                Some(&q) if q == '"' || q == '\'' => {
                    chars.next();
                    for c in chars.by_ref() {
                        if c == q {
                            break;
                        }
                        val.push(c);
                    }
                }
                Some('[') => {
                    for c in chars.by_ref() {
                        val.push(c);
                        if c == ']' {
                            break;
                        }
                    }
                }
                _ => {
                    while let Some(&c) = chars.peek() {
                        if c.is_whitespace() || c == ',' {
                            break;
                        }
                        val.push(c);
                        chars.next();
                    }
                }
            }
            out.push((key, val));
        } else {
            out.push((key, "true".into()));
        }
    }
    out
}

pub fn attr<'a>(attrs: &'a [(String, String)], k: &str) -> Option<&'a str> {
    attrs.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
}

pub fn attr_true(attrs: &[(String, String)], k: &str) -> bool {
    matches!(attr(attrs, k), Some("true") | Some("1") | Some("yes"))
}

fn classes(attrs: &[(String, String)]) -> Vec<String> {
    attrs.iter().filter(|(k, _)| k == "class").map(|(_, v)| v.clone()).collect()
}

// ------------------------------------------------------------------ pre-pass

#[derive(Debug, Clone)]
struct Import {
    path: String,
    attrs: Vec<(String, String)>,
}

// HTML comments interrupt paragraphs, so these always become their own block.
const IMPORT_TOKEN: &str = "<!--mlimport:";
const TOC_TOKEN: &str = "<!--mltoc-->";

const ADMONITIONS: &[&str] = &[
    "note", "abstract", "summary", "tldr", "info", "todo", "tip", "hint", "important", "success", "check", "done", "question", "help", "faq",
    "warning", "caution", "attention", "failure", "fail", "missing", "danger", "error", "bug", "example", "quote", "cite",
];

fn alert_kind(t: &str) -> &'static str {
    match t {
        "tip" | "hint" | "success" | "check" | "done" => "TIP",
        "important" | "question" | "help" | "faq" | "abstract" | "summary" | "tldr" | "example" => "IMPORTANT",
        "warning" | "attention" | "todo" => "WARNING",
        "caution" | "danger" | "error" | "failure" | "fail" | "missing" | "bug" => "CAUTION",
        _ => "NOTE",
    }
}

fn parse_import_line(line: &str) -> Option<Import> {
    let t = line.trim();
    let rest = t.strip_prefix("@import")?.trim_start();
    let q = rest.chars().next()?;
    if q != '"' && q != '\'' {
        return None;
    }
    let body = &rest[1..];
    let end = body.find(q)?;
    let path = body[..end].to_string();
    let after = body[end + 1..].trim();
    let attrs = if after.starts_with('{') { parse_attrs(after.trim_start_matches('{').trim_end_matches('}')) } else { vec![] };
    Some(Import { path, attrs })
}

/// Rewrites MPE-only syntax into what comrak understands, keeping the line count.
fn prepass(text: &str) -> (String, Vec<Import>) {
    let mut out: Vec<String> = Vec::new();
    let mut imports = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut in_admonition = false;
    let lines: Vec<&str> = text.split('\n').collect();
    let mut i = 0;
    // front matter is left alone
    if lines.first().map(|l| l.trim_end() == "---").unwrap_or(false) {
        if let Some(end) = lines.iter().skip(1).position(|l| matches!(l.trim_end(), "---" | "...")) {
            for l in &lines[..end + 2] {
                out.push(l.to_string());
            }
            i = end + 2;
        }
    }
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        // fenced code: copy verbatim
        let fc = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
        if let Some((ch, n)) = fence {
            out.push(line.to_string());
            let run = trimmed.chars().take_while(|c| *c == ch).count();
            if run >= n && trimmed[run * ch.len_utf8()..].trim().is_empty() {
                fence = None;
            }
            i += 1;
            continue;
        }
        if let Some(ch) = fc {
            let run = trimmed.chars().take_while(|c| *c == ch).count();
            if run >= 3 && !in_admonition {
                fence = Some((ch, run));
                out.push(line.to_string());
                i += 1;
                continue;
            }
        }
        if in_admonition {
            if line.trim().is_empty() {
                // a blank line continues the admonition only if indented text follows
                let next_indented = lines[i + 1..].iter().find(|l| !l.trim().is_empty()).map(|l| l.starts_with("    ") || l.starts_with('\t')).unwrap_or(false);
                if next_indented {
                    out.push(">".into());
                    i += 1;
                    continue;
                }
                in_admonition = false;
            } else if line.starts_with("    ") || line.starts_with('\t') {
                let content = line.strip_prefix("    ").or_else(|| line.strip_prefix('\t')).unwrap_or(line);
                out.push(format!("> {content}"));
                i += 1;
                continue;
            } else {
                in_admonition = false;
            }
        }
        if let Some(rest) = line.strip_prefix("!!!").or_else(|| line.strip_prefix("???")) {
            let rest = rest.trim();
            let mut it = rest.splitn(2, char::is_whitespace);
            let kind = it.next().unwrap_or("note").to_ascii_lowercase();
            if ADMONITIONS.contains(&kind.as_str()) || !kind.is_empty() {
                let title = it.next().unwrap_or("").trim().trim_matches('"').to_string();
                let title = if title.is_empty() {
                    let mut c = kind.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                } else {
                    title
                };
                out.push(format!("> [!{}] {}", alert_kind(&kind), title));
                in_admonition = true;
                i += 1;
                continue;
            }
        }
        if trimmed.starts_with("@import") {
            if let Some(imp) = parse_import_line(trimmed) {
                let indent = &line[..line.len() - trimmed.len()];
                out.push(format!("{indent}{IMPORT_TOKEN}{}-->", imports.len()));
                imports.push(imp);
                i += 1;
                continue;
            }
        }
        if matches!(line.trim(), "[TOC]" | "[[TOC]]" | "[toc]" | "<!-- toc -->") {
            out.push(TOC_TOKEN.into());
            i += 1;
            continue;
        }
        out.push(line.to_string());
        i += 1;
    }
    (out.join("\n"), imports)
}

// ------------------------------------------------------------------ options

pub fn options(s: &Settings, sourcepos: bool) -> Options<'static> {
    let mut o = Options::default();
    let e = &mut o.extension;
    e.strikethrough = true;
    e.table = true;
    e.autolink = true;
    e.tasklist = true;
    e.superscript = true;
    e.subscript = true;
    e.highlight = true;
    e.footnotes = true;
    e.description_lists = true;
    e.front_matter_delimiter = Some("---".into());
    e.alerts = true;
    e.math_dollars = true;
    e.math_code = true;
    e.math_latex = true;
    e.shortcodes = s.emoji;
    e.wikilinks_title_after_pipe = true;
    e.header_id_prefix = Some(String::new());
    e.link_attributes = true;
    e.header_attributes = false;
    o.parse.smart = s.typographer;
    o.parse.relaxed_tasklist_matching = true;
    o.render.r#unsafe = true;
    o.render.hardbreaks = s.breaks;
    o.render.sourcepos = sourcepos;
    o.render.github_pre_lang = false;
    o.render.tasklist_classes = true;
    o
}

// ------------------------------------------------------------------ render

fn sp_attr(ctx: &Ctx, node: &AstNode) -> String {
    if !ctx.sourcepos {
        return String::new();
    }
    let sp = node.data().sourcepos;
    format!(" data-sourcepos=\"{}:{}-{}:{}\"", sp.start.line, sp.start.column, sp.end.line, sp.end.column)
}

/// Turns a fenced code block into HTML. Returns `None` to leave it to comrak.
pub fn render_fence(ctx: &Ctx, lang: &str, attrs: &[(String, String)], code: &str, sp: &str) -> Option<String> {
    let cls = classes(attrs).join(" ");
    match lang {
        "mermaid" => Some(diagram_div("mermaid", code, sp, &cls)),
        "math" | "latex" | "katex" | "tex-math" => {
            Some(format!("<div class=\"ml-math-block {cls}\"{sp}><span data-math-style=\"display\">{}</span></div>", escape_html(code)))
        }
        "csv" | "tsv" => {
            let mut o = CsvOptions { max_rows: ctx.settings.csv_max_rows, ..Default::default() };
            if lang == "tsv" {
                o.delimiter = Some('\t');
            }
            o.apply(attrs);
            Some(format!("<div class=\"ml-block {cls}\"{sp}>{}</div>", csv::to_html(code, &o)))
        }
        _ => None,
    }
}

fn diagram_div(kind: &str, code: &str, sp: &str, cls: &str) -> String {
    format!(
        "<div class=\"ml-diagram ml-{kind} {cls}\" data-kind=\"{kind}\" data-h=\"{}\"{sp}><pre class=\"ml-src\">{}</pre></div>",
        hash(&format!("{kind}\n{code}")),
        escape_html(code)
    )
}

fn code_block_html(lang: &str, attrs: &[(String, String)], code: &str, sp: &str) -> String {
    let mut cls = classes(attrs);
    if attr_true(attrs, "line-numbers") || attr_true(attrs, "lineNumbers") {
        cls.push("line-numbers".into());
    }
    let lang_cls = if lang.is_empty() { "nohighlight".to_string() } else { format!("language-{}", escape_html(lang)) };
    let hl = attr(attrs, "highlight").map(|h| format!(" data-highlight=\"{}\"", escape_html(h))).unwrap_or_default();
    format!("<pre class=\"ml-code {}\"{sp}{hl}><code class=\"{lang_cls}\">{}</code></pre>", cls.join(" "), escape_html(code))
}

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif", "bmp", "ico", "apng"];

/// `@import "file"`: tables, images, Markdown, diagrams, code.
fn render_import(ctx: &Ctx, imp: &Import, sp: &str) -> String {
    let p = imp.path.trim();
    if has_scheme(p) {
        let ext = ext_of(p);
        if IMAGE_EXT.contains(&ext.as_str()) {
            return format!("<p{sp}><img src=\"{}\" alt=\"\"></p>", escape_html(p));
        }
        return format!("<p class=\"ml-note\"{sp}>@import of URLs is only supported for images: <a href=\"{0}\">{0}</a></p>", escape_html(p));
    }
    let path = resolve(ctx, p);
    let ext = ext_of(&path.to_string_lossy());
    if IMAGE_EXT.contains(&ext.as_str()) {
        let mut extra = String::new();
        for k in ["width", "height", "title", "alt"] {
            if let Some(v) = attr(&imp.attrs, k) {
                extra.push_str(&format!(" {k}=\"{}\"", escape_html(v)));
            }
        }
        return format!("<p{sp}><img src=\"{}\"{extra}></p>", fs_url(&path));
    }
    let text = match std::fs::read(&path) {
        Ok(b) => String::from_utf8_lossy(&b).to_string(),
        Err(e) => return error_box(&format!("@import \"{}\": {e}", imp.path), sp),
    };
    match ext.as_str() {
        "csv" | "tsv" => {
            let mut o = CsvOptions { max_rows: ctx.settings.csv_max_rows, ..Default::default() };
            if ext == "tsv" {
                o.delimiter = Some('\t');
            }
            o.apply(&imp.attrs);
            format!("<div class=\"ml-block\"{sp}>{}</div>", csv::to_html(&text, &o))
        }
        "md" | "markdown" | "mdown" | "mkd" => {
            if ctx.depth >= 4 {
                return error_box("@import nested too deeply", sp);
            }
            let child = ctx.child(path.clone());
            format!("<div class=\"ml-import\"{sp}>{}</div>", render(&text, &child).html)
        }
        "mermaid" | "mmd" => diagram_div("mermaid", &text, sp, ""),
        "html" | "htm" => format!("<div class=\"ml-import\"{sp}>{}</div>", rewrite_raw_html(&ctx.child(path.clone()), &text)),
        _ => {
            let lines: Vec<&str> = text.lines().collect();
            let begin = attr(&imp.attrs, "line_begin").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(lines.len());
            let end = attr(&imp.attrs, "line_end").and_then(|v| v.parse::<isize>().ok()).map(|v| if v < 0 { (lines.len() as isize + v).max(0) as usize } else { v as usize }).unwrap_or(lines.len()).clamp(begin, lines.len());
            let snippet = lines[begin..end].join("\n");
            let lang = attr(&imp.attrs, "as").map(|s| s.to_string()).unwrap_or_else(|| lang_for_ext(&ext).to_string());
            if let Some(h) = render_fence(ctx, &lang, &imp.attrs, &snippet, sp) {
                return h;
            }
            code_block_html(&lang, &imp.attrs, &snippet, sp)
        }
    }
}

pub fn ext_of(p: &str) -> String {
    let p = p.split(['?', '#']).next().unwrap_or(p);
    Path::new(p).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn lang_for_ext(ext: &str) -> &str {
    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "sh" | "bash" | "zsh" => "bash",
        "yml" => "yaml",
        "h" | "c" => "c",
        "hpp" | "cc" | "cpp" => "cpp",
        "kt" => "kotlin",
        "rb" => "ruby",
        "puml" | "plantuml" | "pu" | "iuml" => "plantuml",
        "gv" => "dot",
        other => other,
    }
}

pub fn error_box(msg: &str, sp: &str) -> String {
    format!("<div class=\"ml-error\"{sp}>{}</div>", escape_html(msg))
}

/// Front matter `key: value` pairs (top level only, enough for title / tables).
pub fn front_matter_pairs(fm: &str) -> Vec<(String, String)> {
    fm.lines()
        .filter(|l| !l.starts_with(' ') && !l.starts_with('\t') && !matches!(l.trim(), "---" | "..." | ""))
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_string(), v.trim().trim_matches('"').trim_matches('\'').to_string()))
        })
        .collect()
}

fn toc_html(headings: &[Heading]) -> String {
    if headings.is_empty() {
        return "<nav class=\"ml-toc\"><p class=\"ml-note\">(no headings)</p></nav>".into();
    }
    let min = headings.iter().map(|h| h.level).min().unwrap_or(1);
    let mut h = String::from("<nav class=\"ml-toc\">");
    let mut depth = 0u8;
    for hd in headings {
        let lvl = hd.level - min + 1;
        while depth < lvl {
            h.push_str("<ul>");
            depth += 1;
        }
        while depth > lvl {
            h.push_str("</ul>");
            depth -= 1;
        }
        h.push_str(&format!("<li><a href=\"#{}\">{}</a></li>", escape_html(&hd.id), escape_html(&hd.text)));
    }
    while depth > 0 {
        h.push_str("</ul>");
        depth -= 1;
    }
    h.push_str("</nav>");
    h
}

pub fn render(text: &str, ctx: &Ctx) -> Rendered {
    let (pre, imports) = prepass(text);
    let opts = options(ctx.settings, ctx.sourcepos);
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &pre, &opts);

    let mut headings = Vec::new();
    let mut anchorizer = Anchorizer::new();
    let mut title = None;
    let mut toc_nodes = Vec::new();
    let nodes: Vec<_> = root.descendants().collect();
    for node in nodes {
        let value = node.data().value.clone();
        match value {
            NodeValue::FrontMatter(fm) => {
                let pairs = front_matter_pairs(&fm);
                title = pairs.iter().find(|(k, _)| k == "title").map(|(_, v)| v.clone());
                let html = match ctx.settings.front_matter.as_str() {
                    "table" if !pairs.is_empty() => {
                        let rows: String = pairs.iter().map(|(k, v)| format!("<tr><th>{}</th><td>{}</td></tr>", escape_html(k), escape_html(v))).collect();
                        format!("<table class=\"ml-front-matter\">{rows}</table>\n")
                    }
                    "code" => format!("<pre class=\"ml-code ml-front-matter\"><code class=\"language-yaml\">{}</code></pre>\n", escape_html(fm.trim().trim_start_matches("---").trim_end_matches("---").trim())),
                    _ => String::new(),
                };
                replace_with_html(node, html);
            }
            NodeValue::Heading(h) => {
                let text = node.collect_text();
                let id = anchorizer.anchorize(&text);
                if title.is_none() && h.level == 1 {
                    title = Some(text.clone());
                }
                headings.push(Heading { level: h.level, text, id, line: node.data().sourcepos.start.line });
            }
            NodeValue::CodeBlock(cb) => {
                let (lang, attrs) = parse_info(&cb.info);
                let sp = sp_attr(ctx, node);
                let html = match render_fence(ctx, &lang, &attrs, &cb.literal, &sp) {
                    Some(h) => h,
                    None => code_block_html(&lang, &attrs, &cb.literal, &sp),
                };
                replace_with_html(node, html + "\n");
            }
            NodeValue::Image(link) => {
                let url = rewrite_url(ctx, &link.url);
                if let NodeValue::Image(ref mut l) = node.data_mut().value {
                    l.url = url;
                }
            }
            NodeValue::Link(link) => {
                let url = rewrite_url(ctx, &link.url);
                if let NodeValue::Link(ref mut l) = node.data_mut().value {
                    l.url = url;
                }
            }
            NodeValue::WikiLink(link) => {
                let mut target = link.url.clone();
                if ext_of(&target).is_empty() {
                    target.push_str(".md");
                }
                let url = rewrite_url(ctx, &target);
                if let NodeValue::WikiLink(ref mut l) = node.data_mut().value {
                    l.url = url;
                }
            }
            NodeValue::HtmlBlock(hb) => {
                let t = hb.literal.trim();
                if let Some(n) = t.strip_prefix(IMPORT_TOKEN).and_then(|r| r.strip_suffix("-->")).and_then(|n| n.parse::<usize>().ok()) {
                    if let Some(imp) = imports.get(n) {
                        let sp = sp_attr(ctx, node);
                        replace_with_html(node, render_import(ctx, imp, &sp) + "\n");
                    }
                    continue;
                }
                if t == TOC_TOKEN {
                    toc_nodes.push(node);
                    continue;
                }
                let lit = rewrite_raw_html(ctx, &hb.literal);
                if let NodeValue::HtmlBlock(ref mut b) = node.data_mut().value {
                    b.literal = lit;
                }
            }
            NodeValue::HtmlInline(s) => {
                let lit = rewrite_raw_html(ctx, &s);
                node.data_mut().value = NodeValue::HtmlInline(lit);
            }
            _ => {}
        }
    }
    for node in toc_nodes {
        let sp = sp_attr(ctx, node);
        let toc = toc_html(&headings).replacen("<nav class=\"ml-toc\"", &format!("<nav class=\"ml-toc\"{sp}"), 1);
        replace_with_html(node, toc + "\n");
    }
    let mut html = String::new();
    let _ = comrak::format_html(root, &opts, &mut html);
    Rendered { html, title, headings }
}

/// Replaces a block node (and its children) by a raw HTML block.
fn replace_with_html<'a>(node: &'a AstNode<'a>, html: String) {
    while let Some(c) = node.first_child() {
        c.detach();
    }
    node.data_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock { block_type: 6, literal: html });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(s: &Settings) -> Ctx<'_> {
        Ctx { settings: s, file: std::env::temp_dir().join("t.md"), roots: vec![], sourcepos: true, depth: 0 }
    }

    #[test]
    fn info_strings() {
        let (l, a) = parse_info("python {cmd=true args=[\"-u\"] .line-numbers #x title=\"a b\"}");
        assert_eq!(l, "python");
        assert_eq!(attr(&a, "cmd"), Some("true"));
        assert_eq!(attr(&a, "args"), Some("[\"-u\"]"));
        assert_eq!(attr(&a, "class"), Some("line-numbers"));
        assert_eq!(attr(&a, "title"), Some("a b"));
        let (l, a) = parse_info("{r cmd=true}");
        assert_eq!(l, "r");
        assert!(attr_true(&a, "cmd"));
    }

    #[test]
    fn gfm_math_mermaid_csv() {
        let s = Settings::default();
        let md = "# Title\n\nText with $x^2$ and ==mark== H~2~O :smile:\n\n- [x] done\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```mermaid\ngraph TD; A-->B\n```\n\n```csv\nx,y\n1,2\n```\n\n$$\n\\int_0^1 x\\,dx\n$$\n\n```rust\nfn main() {}\n```\n";
        let r = render(md, &ctx(&s));
        assert_eq!(r.title.as_deref(), Some("Title"));
        assert!(r.html.contains("data-math-style=\"inline\"") && r.html.contains(">x^2</span>"), "{}", r.html);
        assert!(r.html.contains(">mark</mark>"));
        assert!(r.html.contains(">2</sub>"));
        assert!(r.html.contains("😄"));
        assert!(r.html.contains("class=\"ml-diagram ml-mermaid"));
        assert!(r.html.contains("data-sourcepos=\"11:1-13:3\""), "{}", r.html);
        assert!(r.html.contains(">x</th>"));
        assert!(r.html.contains("data-math-style=\"display\""));
        assert!(r.html.contains("language-rust"));
        assert!(r.html.contains("<table"));
        assert!(r.html.contains("checked"));
    }

    #[test]
    fn prepass_keeps_lines() {
        let md = "---\ntitle: T\n---\n!!! warning \"Careful\"\n    body line\n\n    more\nafter\n@import \"x.csv\"\n[TOC]\n```\n@import \"no\"\n```\n";
        let (p, imps) = prepass(md);
        assert_eq!(p.split('\n').count(), md.split('\n').count());
        assert!(p.contains("> [!WARNING] Careful\n> body line\n>\n> more\nafter"), "{p}");
        assert_eq!(imps.len(), 1);
        assert!(p.contains("@import \"no\""));
        let s = Settings::default();
        let r = render(md, &ctx(&s));
        assert_eq!(r.title.as_deref(), Some("T"));
        assert!(r.html.contains("markdown-alert-warning"), "{}", r.html);
        assert!(r.html.contains("Careful"));
        assert!(r.html.contains("ml-error"), "missing file should show an error");
    }

    #[test]
    fn imports_and_urls() {
        let dir = std::env::temp_dir().join(format!("ml-md-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("data.csv"), "name;qty\nA;1\nB;2\n").unwrap();
        std::fs::write(dir.join("sub/part.md"), "## Part\n\n![p](pic.png)\n").unwrap();
        std::fs::write(dir.join("code.py"), "a=1\nb=2\nc=3\n").unwrap();
        let s = Settings::default();
        let c = Ctx { settings: &s, file: dir.join("doc.md"), roots: vec![dir.clone()], sourcepos: true, depth: 0 };
        let md = "# Doc\n\n[TOC]\n\n@import \"data.csv\" {caption=\"Stock\"}\n\n@import \"sub/part.md\"\n\n@import \"code.py\" {line_begin=1 line_end=2}\n\n![img](<images/a b.png>) [link](other.md#x) [web](https://x.org)\n\n<img src=\"rel.png\">\n";
        let r = render(md, &c);
        assert!(r.html.contains("<caption>Stock</caption>"), "{}", r.html);
        assert!(r.html.contains("<td class=\"num\">2</td>"));
        assert!(r.html.contains("<h2"));
        assert!(r.html.contains(&format!("src=\"{}\"", fs_url(&dir.join("sub/pic.png")))), "{}", r.html);
        assert!(r.html.contains("b=2") && !r.html.contains("a=1"), "{}", r.html);
        assert!(r.html.contains("images/a%20b.png"));
        assert!(r.html.contains("other.md#x"));
        assert!(r.html.contains("href=\"https://x.org\""));
        assert!(r.html.contains(&format!("<img src=\"{}\"", fs_url(&dir.join("rel.png")))));
        assert!(r.html.contains("<nav class=\"ml-toc\""));
        assert!(r.html.contains("href=\"#doc\""));
    }
}
