//! Embeds `assets/` (viewer) and `vendor/` (browser libraries) into the binary,
//! gzip-compressed. The HTTP server serves them as `/assets/...` and `/vendor/...`.

use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.file_name().map(|n| n != ".DS_Store" && n != "VERSION" && n != "SOURCES").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut table = String::from("pub static EMBEDDED: &[(&str, &[u8])] = &[\n");
    for root in ["assets", "vendor"] {
        let base = manifest.join(root);
        println!("cargo:rerun-if-changed={}", base.display());
        let mut files = Vec::new();
        walk(&base, &mut files);
        for f in files {
            println!("cargo:rerun-if-changed={}", f.display());
            let rel = f.strip_prefix(&manifest).unwrap().to_string_lossy().replace('\\', "/");
            let data = std::fs::read(&f).unwrap();
            let mut enc = GzEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&data).unwrap();
            let gz = enc.finish().unwrap();
            let name = rel.replace(['/', '.', '-', '@'], "_");
            let target = out_dir.join(format!("{name}.gz"));
            std::fs::write(&target, gz).unwrap();
            table.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", rel, target.to_string_lossy()));
        }
    }
    table.push_str("];\n");
    // CDN equivalents: vendor/<lib>/<rel> -> https://cdn.jsdelivr.net/npm/<pkg@ver>/<path>
    table.push_str("pub static CDN: &[(&str, &str)] = &[\n");
    if let Ok(rd) = std::fs::read_dir(manifest.join("vendor")) {
        let mut libs: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        libs.sort();
        for lib in libs {
            let name = lib.file_name().unwrap().to_string_lossy().to_string();
            let ver = std::fs::read_to_string(lib.join("VERSION")).unwrap_or_default().trim().to_string();
            for line in std::fs::read_to_string(lib.join("SOURCES")).unwrap_or_default().lines() {
                if let Some((rel, orig)) = line.split_once(' ') {
                    table.push_str(&format!("    ({:?}, {:?}),\n", format!("vendor/{name}/{rel}"), format!("https://cdn.jsdelivr.net/npm/{ver}/{orig}")));
                }
            }
        }
    }
    table.push_str("];\n");
    std::fs::write(out_dir.join("embedded.rs"), table).unwrap();
}
