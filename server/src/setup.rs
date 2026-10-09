//! `markdown-zed setup-zed`: adds the preview task and the Option-M / Alt-M key binding
//! to Zed's tasks.json / keymap.json without disturbing existing content
//! (both files are JSON with comments, so we insert text instead of re-serialising).

use std::path::{Path, PathBuf};

pub fn zed_config_dir() -> PathBuf {
    if let Some(x) = std::env::var_os("ZED_CONFIG_DIR") {
        return PathBuf::from(x);
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            return PathBuf::from(a).join("Zed");
        }
    }
    if !cfg!(target_os = "macos") {
        if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(x).join("zed");
        }
    }
    crate::util::home_dir().unwrap_or_default().join(".config").join("zed")
}

/// Byte index just after the first top-level `[`, and whether the array is empty.
fn array_start(text: &str) -> Option<(usize, bool)> {
    let b = text.as_bytes();
    let mut i = 0;
    let skip = |i: &mut usize| loop {
        while *i < b.len() && (b[*i] as char).is_whitespace() {
            *i += 1;
        }
        if b.get(*i) == Some(&b'/') && b.get(*i + 1) == Some(&b'/') {
            while *i < b.len() && b[*i] != b'\n' {
                *i += 1;
            }
        } else if b.get(*i) == Some(&b'/') && b.get(*i + 1) == Some(&b'*') {
            *i += 2;
            while *i + 1 < b.len() && !(b[*i] == b'*' && b[*i + 1] == b'/') {
                *i += 1;
            }
            *i += 2;
        } else {
            break;
        }
    };
    skip(&mut i);
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let start = i + 1;
    let mut j = start;
    skip(&mut j);
    Some((start, b.get(j) == Some(&b']')))
}

const BEGIN: &str = "// >>> markdown-live";
const END: &str = "// <<< markdown-live";
const LEGACY: &str = "// MarkDown Live (added by `markdown-zed setup-zed`)";

/// Index just past the JSON value (object) starting at `i` (which must be `{`), skipping strings.
fn skip_object(b: &[u8], mut i: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// Removes a block previously written by this tool (new marker style, or the
/// first version's comment followed by `legacy_objects` objects).
fn remove_previous(text: &str, legacy_objects: usize) -> String {
    if let (Some(a), Some(b)) = (text.find(BEGIN), text.find(END)) {
        if b > a {
            let line_start = text[..a].rfind('\n').map(|i| i + 1).unwrap_or(a);
            let mut stop = b + END.len();
            if text[stop..].starts_with('\n') {
                stop += 1;
            }
            // drop the newline we added before the block
            let head = &text[..line_start];
            let head = head.strip_suffix('\n').unwrap_or(head);
            return format!("{}{}", head, &text[stop..]);
        }
    }
    if let Some(a) = text.find(LEGACY) {
        let bytes = text.as_bytes();
        let line_start = text[..a].rfind('\n').map(|i| i + 1).unwrap_or(a);
        let mut i = a + LEGACY.len();
        for n in 0..legacy_objects {
            while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                i += 1;
            }
            if bytes.get(i) != Some(&b'{') {
                return text.to_string();
            }
            match skip_object(bytes, i) {
                Some(e) => i = e,
                None => return text.to_string(),
            }
            let mut j = i;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if bytes.get(j) == Some(&b',') {
                i = j + 1;
            } else if n + 1 < legacy_objects {
                return text.to_string();
            }
        }
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if bytes.get(i) == Some(&b'\n') {
            i += 1;
        }
        return format!("{}{}", &text[..line_start], &text[i..]);
    }
    text.to_string()
}

/// Inserts `objects` (JSON object texts) at the start of the top-level array,
/// replacing whatever this tool inserted before.
fn insert_entry(path: &Path, objects: &[String], legacy_objects: usize) -> Result<String, String> {
    let original = std::fs::read_to_string(path).unwrap_or_default();
    let mut existing = remove_previous(&original, legacy_objects);
    if existing.trim().is_empty() {
        existing = "[]\n".into();
    }
    let body: Vec<String> = objects.iter().map(|o| format!("  {o}")).collect();
    let new = if existing.trim().is_empty() {
        format!("[\n  {BEGIN} (managed by `markdown-zed setup-zed`; re-run it to update)\n{}\n  {END}\n]\n", body.join(",\n"))
    } else {
        let (pos, empty) = array_start(&existing).ok_or_else(|| format!("{} is not a JSON array; add the entry by hand (see README)", path.display()))?;
        let sep = if empty { "" } else { "," };
        format!(
            "{}\n  {BEGIN} (managed by `markdown-zed setup-zed`; re-run it to update)\n{}{sep}\n  {END}\n{}",
            &existing[..pos],
            body.join(",\n"),
            &existing[pos..]
        )
    };
    if new == original {
        return Ok(format!("{} is up to date", path.display()));
    }
    if !original.is_empty() {
        let _ = std::fs::write(path.with_extension("json.bak"), &original);
    }
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, new).map_err(|e| e.to_string())?;
    Ok(format!("updated {}", path.display()))
}

/// Human name of the preview key for this platform.
pub fn key_label(key: &str) -> String {
    key.split(' ')
        .map(|stroke| {
            stroke
                .split('-')
                .map(|part| match part {
                    "alt" if cfg!(target_os = "macos") => "Option".to_string(),
                    "alt" => "Alt".to_string(),
                    "cmd" => "Cmd".to_string(),
                    "ctrl" => "Ctrl".to_string(),
                    "shift" => "Shift".to_string(),
                    p => p.to_uppercase(),
                })
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn run(args: &[String]) {
    let custom = args.iter().position(|a| a == "--key").and_then(|i| args.get(i + 1)).cloned();
    // Option-M on macOS is spelled `alt-m` in Zed keymaps. `use_key_equivalents`
    // makes it follow non-US layouts (AZERTY, QWERTZ…).
    let key = custom.unwrap_or_else(|| "alt-m".into());
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "markdown-zed".into());
    let exe_json = serde_json::to_string(&exe).unwrap();
    let dir = zed_config_dir();
    let task = |label: &str, args: &str, reveal: &str| {
        format!(
            "{{\n    \"label\": \"{label}\",\n    \"command\": {exe_json},\n    \"args\": {args},\n    \"reveal\": \"{reveal}\",\n    \"hide\": \"on_success\",\n    \"allow_concurrent_runs\": true,\n    \"use_new_terminal\": false\n  }}"
        )
    };
    let mut tasks = vec![task("MarkDown Live: Preview", r#"["preview", "$ZED_FILE", "--line", "$ZED_ROW"]"#, "never")];
    let mut keys = vec![(key.clone(), "MarkDown Live: Preview".to_string())];
    for (label, args, reveal, k) in crate::lsp_ext::extra_tasks() {
        tasks.push(task(label, args, reveal));
        if let Some(k) = k {
            keys.push((k.to_string(), label.to_string()));
        }
    }
    let bindings: Vec<String> = keys
        .iter()
        .map(|(k, t)| format!("      {}: [\"task::Spawn\", {{ \"task_name\": {} }}]", serde_json::to_string(k).unwrap(), serde_json::to_string(t).unwrap()))
        .collect();
    let keymap = vec![format!(
        "{{\n    \"context\": \"Editor && (extension == md || extension == markdown || extension == mdown || extension == mkd)\",\n    \"use_key_equivalents\": true,\n    \"bindings\": {{\n{}\n    }}\n  }}",
        bindings.join(",\n")
    )];
    for (file, objs, legacy) in [("tasks.json", tasks, 1usize), ("keymap.json", keymap, 1usize)] {
        match insert_entry(&dir.join(file), &objs, legacy) {
            Ok(m) => println!("{m}"),
            Err(e) => eprintln!("error: {e}"),
        }
    }
    println!("Done. In Zed, open a Markdown file and press {} to open the live preview.", key_label(&key));
    for (k, t) in keys.iter().skip(1) {
        println!("  {} → {}", key_label(k), t);
    }
    println!("Or: command palette → \"task: spawn\" → \"MarkDown Live: Preview\".");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn strip(t: &str) -> String {
        t.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
    }
    #[test]
    fn inserts_and_replaces() {
        let d = std::env::temp_dir().join(format!("ml-setup-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // legacy block from the first version + a user binding
        let f = d.join("keymap.json");
        std::fs::write(&f, "// my keys\n[\n  // MarkDown Live (added by `markdown-zed setup-zed`)\n  {\n    \"context\": \"Editor\",\n    \"bindings\": { \"alt-d\": [\"task::Spawn\", { \"task_name\": \"MarkDown Live: Preview\" }] }\n  },\n  { \"bindings\": { \"x\": \"y\" } }\n]\n").unwrap();
        let objs = vec!["{\"a\": \"{not a brace}\"}".to_string()];
        insert_entry(&f, &objs, 1).unwrap();
        let t = std::fs::read_to_string(&f).unwrap();
        assert!(!t.contains("added by"), "{t}");
        assert!(t.contains("\"x\": \"y\""), "{t}");
        let v: serde_json::Value = serde_json::from_str(&strip(&t)).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        // second run replaces, doesn't duplicate
        let objs2 = vec!["{\"b\": 1}".to_string()];
        insert_entry(&f, &objs2, 1).unwrap();
        let t = std::fs::read_to_string(&f).unwrap();
        let v: serde_json::Value = serde_json::from_str(&strip(&t)).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2, "{t}");
        assert!(t.contains("\"b\": 1") && !t.contains("not a brace"));
        // empty array / missing file
        let g = d.join("tasks.json");
        std::fs::write(&g, "[]").unwrap();
        insert_entry(&g, &objs2, 3).unwrap();
        let v: serde_json::Value = serde_json::from_str(&strip(&std::fs::read_to_string(&g).unwrap())).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
        let h = d.join("new.json");
        let _ = std::fs::remove_file(&h);
        insert_entry(&h, &objs2, 1).unwrap();
        let v: serde_json::Value = serde_json::from_str(&strip(&std::fs::read_to_string(&h).unwrap())).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert!(insert_entry(&h, &objs2, 1).unwrap().contains("up to date"));
    }
}
