//! Server-side renderers: PlantUML (local Java or a server), Kroki, and code chunks.

use crate::config::Settings;
use crate::util;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Output {
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub timed_out: bool,
}

/// Runs a process with optional stdin and a timeout.
pub fn run_process(mut cmd: Command, stdin: Option<&[u8]>, timeout: Duration) -> Result<Output, String> {
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let prog = format!("{:?}", cmd.get_program());
    let mut child = cmd.spawn().map_err(|e| format!("cannot run {prog}: {e}"))?;
    if let Some(data) = stdin {
        let mut si = child.stdin.take().unwrap();
        let data = data.to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.code(),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    timed_out = true;
                    break child.wait().ok().and_then(|s| s.code());
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let stdout = t_out.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).to_string();
    Ok(Output { status, stdout, stderr, timed_out })
}

// ------------------------------------------------------------------ PlantUML

pub fn find_java(s: &Settings) -> Option<PathBuf> {
    if !s.java.is_empty() {
        return Some(PathBuf::from(&s.java));
    }
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        let p = PathBuf::from(h).join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        if p.is_file() {
            return Some(p);
        }
    }
    for p in ["/opt/homebrew/opt/openjdk/bin/java", "/usr/local/opt/openjdk/bin/java"] {
        if Path::new(p).is_file() {
            return Some(PathBuf::from(p));
        }
    }
    // macOS has a /usr/bin/java stub that only prints "install Java"; check it works.
    let j = util::which("java")?;
    let ok = Command::new(&j).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    ok.then_some(j)
}

pub fn find_jar(s: &Settings) -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if !s.plantuml_jar.is_empty() {
        c.push(PathBuf::from(&s.plantuml_jar));
    }
    if let Some(v) = std::env::var_os("PLANTUML_JAR") {
        c.push(PathBuf::from(v));
    }
    if let Some(h) = util::home_dir() {
        // the copy PlantUML Live downloads
        c.push(h.join("Library/Caches/plantuml-zed/plantuml.jar"));
        c.push(h.join(".cache/plantuml-zed/plantuml.jar"));
    }
    for p in [
        "/opt/homebrew/opt/plantuml/libexec/plantuml.jar",
        "/usr/local/opt/plantuml/libexec/plantuml.jar",
        "/usr/share/plantuml/plantuml.jar",
        "/usr/share/java/plantuml.jar",
        "/usr/share/java/plantuml/plantuml.jar",
    ] {
        c.push(PathBuf::from(p));
    }
    c.into_iter().find(|p| p.is_file())
}

/// How PlantUML would run now: "local (…)", "online (host)" or an error.
pub fn plantuml_backend(s: &Settings) -> Result<String, String> {
    let local = match (find_java(s), find_jar(s)) {
        (Some(j), Some(jar)) => Some(format!("local ({} -jar {})", j.display(), jar.display())),
        _ => util::which("plantuml").map(|p| format!("local ({})", p.display())),
    };
    match s.plantuml_render.as_str() {
        "online" => Ok(format!("online ({})", s.plantuml_server)),
        "auto" => Ok(local.unwrap_or_else(|| format!("online ({})", s.plantuml_server))),
        _ => local.ok_or_else(|| {
            "PlantUML needs Java and plantuml.jar (macOS: `brew install plantuml`), or set \"plantumlRender\": \"online\" in the settings.".to_string()
        }),
    }
}

/// Renders PlantUML source to SVG text.
pub fn plantuml_svg(s: &Settings, src: &str, dir: &Path) -> Result<String, String> {
    let backend = plantuml_backend(s)?;
    if backend.starts_with("online") {
        return plantuml_online(s, src);
    }
    let mut cmd = match (find_java(s), find_jar(s)) {
        (Some(j), Some(jar)) => {
            let mut c = Command::new(j);
            c.arg("-Djava.awt.headless=true").arg("-jar").arg(jar);
            c
        }
        _ => Command::new(util::which("plantuml").ok_or("plantuml not found")?),
    };
    cmd.args(["-tsvg", "-pipe", "-charset", "UTF-8", "-nometadata"]).current_dir(dir);
    let out = run_process(cmd, Some(src.as_bytes()), Duration::from_secs(60))?;
    if out.timed_out {
        return Err("PlantUML timed out".into());
    }
    let svg = String::from_utf8_lossy(&out.stdout).to_string();
    let start = svg.find("<svg").ok_or_else(|| format!("PlantUML failed: {}", out.stderr.trim()))?;
    Ok(svg[start..].to_string())
}

const PUML_ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_";

/// PlantUML's text encoding: raw DEFLATE + its own base64 alphabet.
pub fn plantuml_encode(src: &str) -> String {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(src.as_bytes());
    let data = e.finish().unwrap_or_default();
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            out.push(PUML_ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).redirects(3).build()
}

fn plantuml_online(s: &Settings, src: &str) -> Result<String, String> {
    if s.plantuml_server.contains("kroki") {
        return kroki(&s.plantuml_server, "plantuml", src);
    }
    let url = format!("{}/svg/{}", s.plantuml_server, plantuml_encode(src));
    match agent().get(&url).call() {
        Ok(r) => r.into_string().map_err(|e| e.to_string()),
        // PlantUML servers answer 400 with an SVG that shows the error.
        Err(ureq::Error::Status(_, r)) => r.into_string().map_err(|e| e.to_string()),
        Err(e) => Err(format!("{}: {e}", s.plantuml_server)),
    }
}

/// Renders any Kroki diagram type to SVG (online).
pub fn kroki(server: &str, kind: &str, src: &str) -> Result<String, String> {
    let url = format!("{}/{}/svg", server.trim_end_matches('/'), kind);
    match agent().post(&url).set("Content-Type", "text/plain").send_string(src) {
        Ok(r) => r.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, r)) => Err(format!("Kroki {code}: {}", r.into_string().unwrap_or_default().trim())),
        Err(e) => Err(format!("{server}: {e}")),
    }
}

// ------------------------------------------------------------------ code chunks

/// Interpreter command for a code-chunk language (None = not runnable).
pub fn interpreter(s: &Settings, lang: &str, cmd_attr: &str, roots: &[PathBuf], dir: &Path) -> Option<(Vec<String>, &'static str)> {
    if cmd_attr != "true" && !cmd_attr.is_empty() {
        let ext = match lang {
            "python" | "py" => "py",
            "javascript" | "js" | "node" => "js",
            "r" => "R",
            _ => "txt",
        };
        return Some((util::shell_split(cmd_attr), ext));
    }
    let python = || -> String {
        if !s.python.is_empty() {
            return s.python.clone();
        }
        for base in std::iter::once(dir.to_path_buf()).chain(roots.iter().cloned()) {
            for v in [".venv", "venv"] {
                let p = base.join(v).join(if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" });
                if p.is_file() {
                    return p.to_string_lossy().to_string();
                }
            }
        }
        if util::which("python3").is_some() { "python3".into() } else { "python".into() }
    };
    let (argv, ext): (Vec<String>, &'static str) = match lang {
        "python" | "py" | "python3" => (vec![python(), "-u".into()], "py"),
        "javascript" | "js" | "node" => (vec!["node".into()], "js"),
        "bash" | "sh" | "zsh" | "shell" => (vec![if lang == "zsh" { "zsh".into() } else { "bash".into() }], "sh"),
        "ruby" | "rb" => (vec!["ruby".into()], "rb"),
        "perl" => (vec!["perl".into()], "pl"),
        "php" => (vec!["php".into()], "php"),
        "lua" => (vec!["lua".into()], "lua"),
        "r" => (vec!["Rscript".into()], "R"),
        "julia" => (vec!["julia".into()], "jl"),
        _ => return None,
    };
    Some((argv, ext))
}

/// Python preamble: captures matplotlib figures (SVG) and Plotly figures (JSON) on
/// `show()` and at the end, and prints them as marker lines the server picks up.
const PY_PRELUDE: &str = r#"
import sys as __ml_sys, io as __ml_io, base64 as __ml_b64, json as __ml_json, os as __ml_os
def __ml_emit(kind, data):
    __ml_sys.stdout.flush()
    print("\x1eML-" + kind + " " + data, flush=True)
try:
    import matplotlib as __ml_mpl
    __ml_mpl.use("Agg")
    import matplotlib.pyplot as __ml_plt
    def __ml_flush_figs(*a, **k):
        for n in __ml_plt.get_fignums():
            f = __ml_plt.figure(n)
            b = __ml_io.BytesIO()
            f.savefig(b, format=__ml_os.environ.get("ML_FIG_FORMAT", "svg"), bbox_inches="tight", dpi=150)
            __ml_emit("FIG-" + __ml_os.environ.get("ML_FIG_FORMAT", "svg"), __ml_b64.b64encode(b.getvalue()).decode())
        __ml_plt.close("all")
    __ml_plt.show = __ml_flush_figs
except Exception:
    __ml_flush_figs = None
try:
    import plotly.io as __ml_pio
    import plotly.basedatatypes as __ml_pbd
    def __ml_plotly_show(fig, *a, **k):
        __ml_emit("PLOTLY", __ml_b64.b64encode(__ml_pio.to_json(fig).encode()).decode())
    __ml_pio.show = __ml_plotly_show
    __ml_pbd.BaseFigure.show = __ml_plotly_show
except Exception:
    pass
__ml_g = {"__name__": "__main__"}
__ml_file = __ml_sys.argv[1]
__ml_sys.argv = __ml_sys.argv[1:]
try:
    with open(__ml_file, encoding="utf-8") as __ml_f:
        exec(compile(__ml_f.read(), "chunk", "exec"), __ml_g)
finally:
    if __ml_flush_figs:
        __ml_flush_figs()
"#;

/// Runs a code chunk. Returns JSON: {stdout, stderr, status, timedOut, figures: [{kind, data}]}.
pub fn run_chunk(argv: &[String], ext: &str, code: &str, dir: &Path, timeout: Duration, args: &[String]) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("markdown-live-{}-{}", std::process::id(), crate::markdown::hash(code)));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let file = tmp.join(format!("chunk.{ext}"));
    std::fs::write(&file, code).map_err(|e| e.to_string())?;
    let is_python = ext == "py" && argv.first().map(|p| p.contains("python")).unwrap_or(false);
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).current_dir(dir);
    if is_python {
        let prelude = tmp.join("ml_prelude.py");
        std::fs::write(&prelude, PY_PRELUDE).map_err(|e| e.to_string())?;
        cmd.arg(&prelude);
        cmd.env("MPLBACKEND", "Agg").env("PYTHONIOENCODING", "utf-8");
    }
    cmd.arg(&file).args(args);
    let out = run_process(cmd, None, timeout);
    let _ = std::fs::remove_dir_all(&tmp);
    let out = out?;
    let mut text = String::new();
    let mut figures = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("\u{1e}ML-") {
            let rest = rest.trim_end();
            if let Some((kind, data)) = rest.split_once(' ') {
                figures.push(serde_json::json!({"kind": kind, "data": data}));
                continue;
            }
        }
        text.push_str(line);
    }
    let stderr = out.stderr.replace(&file.to_string_lossy().to_string(), "chunk");
    Ok(serde_json::json!({
        "stdout": text,
        "stderr": stderr,
        "status": out.status,
        "timedOut": out.timed_out,
        "figures": figures,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encode_known() {
        // value from plantuml.com for "Bob -> Alice : hello"
        let e = plantuml_encode("Bob -> Alice : hello");
        assert!(e.starts_with("SyfFKj2rKt3CoKnELR1Io4ZDoSa7"), "{e}");
    }
    #[test]
    fn python_chunk_with_figure() {
        if util::which("python3").is_none() {
            return;
        }
        let code = "print('hi')\ntry:\n    import matplotlib.pyplot as plt\n    plt.plot([1,2,3])\n    plt.show()\nexcept ImportError:\n    pass\n";
        let r = run_chunk(&["python3".into(), "-u".into()], "py", code, &std::env::temp_dir(), Duration::from_secs(60), &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["stdout"], "hi\n");
        assert_eq!(v["status"], 0);
        if v["figures"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
            eprintln!("(matplotlib not installed)");
        } else {
            assert_eq!(v["figures"][0]["kind"], "FIG-svg");
        }
    }
    #[test]
    fn timeout_and_error() {
        if util::which("python3").is_none() {
            return;
        }
        let r = run_chunk(&["python3".into()], "py", "import time\ntime.sleep(5)", &std::env::temp_dir(), Duration::from_millis(500), &[]).unwrap();
        assert!(r.contains("\"timedOut\":true"));
        let r = run_chunk(&["python3".into()], "py", "raise ValueError('boom')", &std::env::temp_dir(), Duration::from_secs(10), &[]).unwrap();
        assert!(r.contains("ValueError: boom"));
    }
}
