#!/usr/bin/env bash
# One-time setup for the MarkDown Live Zed extension (macOS / Linux).
#   1. builds and installs the `markdown-live` server (cargo install)
#   2. adds the "MarkDown Live: Preview" task + the Option-M key binding to Zed
#      (Zed spells Option as `alt`; pass another key as $1, e.g. ./setup.sh alt-k)
#   3. shows what was detected (browser, libraries)
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(pwd)"
KEY="${1:-alt-m}"   # alt = Option on macOS

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*"; }

command -v cargo >/dev/null || { echo "Rust is required: install it from https://rustup.rs and re-run."; exit 1; }
if command -v rustup >/dev/null; then
  rustup target add wasm32-wasip1 >/dev/null 2>&1 || warn "could not add the wasm32-wasip1 target (Zed will try again)"
else
  warn "Zed builds dev extensions with rustup; Rust installed another way may not work."
fi

say "Building and installing markdown-zed (this takes a minute the first time)"
cargo install --path server --locked --force --quiet
BIN="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin/markdown-zed"
[ -x "$BIN" ] || BIN="$(command -v markdown-zed)"
say "Installed $BIN"
if ! command -v markdown-zed >/dev/null; then
  warn "$(dirname "$BIN") is not on your PATH. The extension finds it anyway; to use the CLI, add:"
  echo "    echo 'export PATH=\"\$HOME/.cargo/bin:\$PATH\"' >> ~/.zshrc"
fi

say "Adding the preview task and key binding to Zed"
"$BIN" setup-zed --key "$KEY"

say "Checking the setup"
"$BIN" doctor || true
for t in pandoc ffmpeg plantuml; do
  command -v "$t" >/dev/null || warn "optional: $t is not installed (brew install $t) — see the README for what it adds"
done

cat <<MSG

All set. Last step, in Zed:
  1. Cmd/Ctrl-Shift-P  →  "zed: install dev extension"
  2. choose this folder: $ROOT
  3. open a .md file and press Option-M (Alt-M on Linux)

MSG
