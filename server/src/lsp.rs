//! Language Server Protocol over stdio. The server doesn't lint Markdown; it only
//! follows the editor (text, cursor, file switches) and offers code actions.

use crate::app::{App, Mode};
use crate::config::Settings;
use crate::{http, log, util};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

type Out = Arc<Mutex<std::io::Stdout>>;

fn send(out: &Out, v: &Value) {
    let body = v.to_string();
    let mut o = out.lock().unwrap();
    let _ = write!(o, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = o.flush();
}

fn read_message(r: &mut impl BufRead) -> Option<Value> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let l = line.trim_end();
        if l.is_empty() {
            if len.is_some() {
                break;
            }
            continue;
        }
        if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().ok();
        }
    }
    let mut buf = vec![0u8; len?];
    r.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

/// (command, title) — shown in the code-actions menu (Cmd-. / Ctrl-.).
pub fn commands() -> Vec<(&'static str, &'static str)> {
    let mut v = vec![("markdownLive.preview", "MarkDown Live: Open preview"), ("markdownLive.refresh", "MarkDown Live: Refresh preview")];
    v.extend(crate::lsp_ext::extra_commands());
    v
}

pub fn run() {
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut app: Option<Arc<App>> = None;

    while let Some(msg) = read_message(&mut reader) {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        if method.is_empty() {
            continue; // a response to a request we sent (showDocument, applyEdit)
        }
        let reply = |result: Value| {
            if let Some(id) = &id {
                send(&out, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
            }
        };
        match method.as_str() {
            "initialize" => {
                let opts = params.get("initializationOptions").cloned().unwrap_or(Value::Null);
                let a = App::new(Mode::Lsp, Settings::from_json(&opts));
                {
                    let mut st = a.state.lock().unwrap();
                    if let Some(folders) = params.get("workspaceFolders").and_then(|f| f.as_array()) {
                        for f in folders {
                            if let Some(p) = f["uri"].as_str().and_then(util::uri_to_path) {
                                st.roots.push(p);
                            }
                        }
                    }
                    if st.roots.is_empty() {
                        if let Some(p) = params.get("rootUri").and_then(|u| u.as_str()).and_then(util::uri_to_path) {
                            st.roots.push(p);
                        }
                    }
                }
                let o2 = out.clone();
                *a.notify.lock().unwrap() = Some(Box::new(move |v: Value| send(&o2, &v)));
                if let Err(e) = http::start(&a) {
                    log!("{e}");
                }
                a.start_worker();
                app = Some(a);
                reply(json!({
                    "capabilities": {
                        "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": false}},
                        "codeActionProvider": {"codeActionKinds": ["", "source"]},
                        "executeCommandProvider": {"commands": commands().iter().map(|c| c.0).collect::<Vec<_>>()},
                        "documentHighlightProvider": true,
                    },
                    "serverInfo": {"name": "markdown-live", "version": env!("CARGO_PKG_VERSION")}
                }));
            }
            "initialized" => {}
            "shutdown" => {
                if let Some(a) = &app {
                    a.shutdown();
                }
                reply(Value::Null);
            }
            "exit" => break,
            "workspace/didChangeConfiguration" => {
                if let Some(a) = &app {
                    let s = params.get("settings").cloned().unwrap_or(Value::Null);
                    if s.is_object() && !s.as_object().unwrap().is_empty() {
                        a.set_settings(Settings::from_json(&s));
                    }
                }
            }
            "workspace/didChangeWorkspaceFolders" => {
                if let Some(a) = &app {
                    let mut st = a.state.lock().unwrap();
                    for f in params["event"]["added"].as_array().cloned().unwrap_or_default() {
                        if let Some(p) = f["uri"].as_str().and_then(util::uri_to_path) {
                            st.roots.push(p);
                        }
                    }
                }
            }
            "textDocument/didOpen" => {
                if let (Some(a), Some(p)) = (&app, doc_path(&params)) {
                    let text = params["textDocument"]["text"].as_str().unwrap_or("").to_string();
                    a.doc_opened(p, text);
                }
            }
            "textDocument/didChange" => {
                if let (Some(a), Some(p)) = (&app, doc_path(&params)) {
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(last) = changes.iter().rev().find(|c| c.get("range").is_none()) {
                            a.doc_changed(p, last["text"].as_str().unwrap_or("").to_string());
                        }
                    }
                }
            }
            "textDocument/didSave" => {
                if let (Some(a), Some(p)) = (&app, doc_path(&params)) {
                    a.doc_saved(p);
                }
            }
            "textDocument/didClose" => {
                if let (Some(a), Some(p)) = (&app, doc_path(&params)) {
                    a.doc_closed(p);
                }
            }
            "textDocument/documentHighlight" => {
                // Zed asks for highlights whenever the cursor moves: that's our scroll-sync signal.
                if let (Some(a), Some(p)) = (&app, doc_path(&params)) {
                    let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
                    a.cursor(p, line);
                }
                reply(Value::Null);
            }
            "textDocument/codeAction" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_string();
                let line = params["range"]["start"]["line"].as_u64().unwrap_or(0);
                let actions: Vec<Value> = commands()
                    .iter()
                    .map(|(cmd, title)| {
                        json!({
                            "title": title,
                            "kind": "source",
                            "command": {"title": title, "command": cmd, "arguments": [uri, line]}
                        })
                    })
                    .collect();
                reply(json!(actions));
            }
            "workspace/executeCommand" => {
                let cmd = params["command"].as_str().unwrap_or("").to_string();
                let args = params["arguments"].as_array().cloned().unwrap_or_default();
                let path = args.first().and_then(|u| u.as_str()).and_then(util::uri_to_path).map(|p| util::norm_path(&p));
                let line = args.get(1).and_then(|l| l.as_u64()).map(|l| l as usize);
                reply(Value::Null);
                if let (Some(a), Some(p)) = (&app, path) {
                    let a = a.clone();
                    let out = out.clone();
                    std::thread::spawn(move || execute(&a, &out, &cmd, p, line));
                }
            }
            _ => {
                if id.is_some() {
                    send(&out, &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("unhandled: {method}")}}));
                }
            }
        }
    }
    if let Some(a) = &app {
        a.shutdown();
    }
}

fn doc_path(params: &Value) -> Option<PathBuf> {
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    util::uri_to_path(uri).map(|p| util::norm_path(&p))
}

pub fn show_message(out: &Out, kind: u8, msg: &str) {
    send(out, &json!({"jsonrpc": "2.0", "method": "window/showMessage", "params": {"type": kind, "message": msg}}));
}

fn execute(a: &Arc<App>, out: &Out, cmd: &str, path: PathBuf, line: Option<usize>) {
    match cmd {
        "markdownLive.preview" => {
            let (url, _) = a.open_preview(path, line, false);
            log!("preview: {url}");
        }
        "markdownLive.refresh" => a.refresh(),
        other => {
            if let Err(e) = crate::lsp_ext::execute(a, other, path, line) {
                show_message(out, 1, &format!("MarkDown Live: {e}"));
            } else if let Some(msg) = crate::lsp_ext::done_message(other) {
                show_message(out, 3, &msg);
            }
        }
    }
}
