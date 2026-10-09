//! Zed extension: MarkDown Live.
//!
//! The extension itself is tiny (Zed extensions run as WebAssembly and can't
//! draw UI or open webviews). It attaches the `markdown-live` language server
//! to Zed's built-in Markdown language. That server renders the *unsaved*
//! buffer and serves the live preview in Chromium / your browser.

use std::fs;
use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

const SERVER_ID: &str = "markdown-live";
const BINARY: &str = "markdown-zed";
/// GitHub repository that publishes prebuilt `markdown-live` binaries
/// (see `.github/workflows/release.yml`). Set this when you publish your fork;
/// while it is empty the binary must be on PATH (`cargo install --path server`).
const GITHUB_REPO: &str = "";

struct MarkdownLiveExtension {
    cached_binary: Option<String>,
}

impl MarkdownLiveExtension {
    fn binary_path(&mut self, id: &LanguageServerId, worktree: &zed::Worktree) -> Result<(String, Vec<String>)> {
        // 1. Explicit setting: "lsp": { "markdown-live": { "binary": { "path": "...", "arguments": [] } } }
        if let Ok(settings) = LspSettings::for_worktree(SERVER_ID, worktree) {
            if let Some(binary) = settings.binary {
                if let Some(path) = binary.path {
                    return Ok((path, binary.arguments.unwrap_or_default()));
                }
            }
        }
        // 2. On PATH (cargo install puts it in ~/.cargo/bin).
        if let Some(path) = worktree.which(BINARY) {
            return Ok((path, vec![]));
        }
        // 3. `cargo install` location. ~/.cargo/bin is often not on PATH (e.g. Rust from
        //    Homebrew, or Zed started from the Dock). The extension runs in a WASM sandbox
        //    and can't check whether the file exists, so when no release download is
        //    configured we use this path directly; Zed reports an error if it's missing.
        let env = worktree.shell_env();
        let get = |k: &str| env.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
        let (os, _) = zed::current_platform();
        let exe = if os == zed::Os::Windows { format!("{BINARY}.exe") } else { BINARY.to_string() };
        let sep = if os == zed::Os::Windows { "\\" } else { "/" };
        let cargo_bin = get("CARGO_HOME")
            .map(|c| format!("{c}{sep}bin{sep}{exe}"))
            .or_else(|| get("HOME").map(|h| format!("{h}/.cargo/bin/{exe}")))
            .or_else(|| get("USERPROFILE").map(|h| format!("{h}\\.cargo\\bin\\{exe}")));
        if GITHUB_REPO.is_empty() {
            if let Some(path) = cargo_bin {
                return Ok((path, vec![]));
            }
        }
        // 4. Previously downloaded copy / download a release.
        if let Some(path) = &self.cached_binary {
            if fs::metadata(path).map(|m| m.is_file()).unwrap_or(false) {
                return Ok((path.clone(), vec![]));
            }
        }
        if !GITHUB_REPO.is_empty() {
            let path = self.download(id)?;
            self.cached_binary = Some(path.clone());
            return Ok((path, vec![]));
        }
        Err(format!(
            "`{BINARY}` was not found. Install it with `cargo install --path server` \
             (from the extension folder) or set lsp.{SERVER_ID}.binary.path in your Zed settings."
        ))
    }

    fn download(&self, id: &LanguageServerId) -> Result<String> {
        zed::set_language_server_installation_status(id, &zed::LanguageServerInstallationStatus::CheckingForUpdate);
        let release = zed::latest_github_release(
            GITHUB_REPO,
            zed::GithubReleaseOptions { require_assets: true, pre_release: false },
        )?;
        let (os, arch) = zed::current_platform();
        let target = match (os, arch) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
            (zed::Os::Mac, _) => "x86_64-apple-darwin",
            (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            (zed::Os::Linux, _) => "x86_64-unknown-linux-gnu",
            (zed::Os::Windows, _) => "x86_64-pc-windows-msvc",
        };
        let (ext, kind) = if os == zed::Os::Windows {
            ("zip", zed::DownloadedFileType::Zip)
        } else {
            ("tar.gz", zed::DownloadedFileType::GzipTar)
        };
        let asset_name = format!("{BINARY}-{target}.{ext}");
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("no release asset {asset_name}"))?;
        let dir = format!("{BINARY}-{}", release.version);
        let exe = if os == zed::Os::Windows { format!("{BINARY}.exe") } else { BINARY.to_string() };
        let path = format!("{dir}/{exe}");
        if !fs::metadata(&path).map(|m| m.is_file()).unwrap_or(false) {
            zed::set_language_server_installation_status(id, &zed::LanguageServerInstallationStatus::Downloading);
            zed::download_file(&asset.download_url, &dir, kind).map_err(|e| format!("download failed: {e}"))?;
            zed::make_file_executable(&path)?;
            // remove older versions
            if let Ok(entries) = fs::read_dir(".") {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with(&format!("{BINARY}-")) && name != dir {
                        let _ = fs::remove_dir_all(e.path());
                    }
                }
            }
        }
        zed::set_language_server_installation_status(id, &zed::LanguageServerInstallationStatus::None);
        // Zed runs the command relative to the extension's work dir; make it absolute.
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        Ok(cwd.join(path).to_string_lossy().to_string())
    }

    fn user_settings(worktree: &zed::Worktree) -> zed::serde_json::Value {
        LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.settings)
            .unwrap_or_else(|| zed::serde_json::json!({}))
    }
}

impl zed::Extension for MarkdownLiveExtension {
    fn new() -> Self {
        MarkdownLiveExtension { cached_binary: None }
    }

    fn language_server_command(&mut self, id: &LanguageServerId, worktree: &zed::Worktree) -> Result<zed::Command> {
        let (command, args) = self.binary_path(id, worktree)?;
        // Pass the user's shell environment so `chromium`, `python3`, `java` etc. are found
        // even when Zed was started from the Dock / Finder.
        let env = worktree.shell_env();
        Ok(zed::Command { command, args, env })
    }

    fn language_server_initialization_options(
        &mut self,
        _id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(Some(Self::user_settings(worktree)))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(Some(Self::user_settings(worktree)))
    }
}

zed::register_extension!(MarkdownLiveExtension);
