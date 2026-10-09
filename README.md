# MarkDown Live — live Markdown preview for Zed

Zed extensions can't open a webview, so MarkDown Live does what PlantUML Live does:
a small Rust language server renders the **unsaved** buffer and shows it in a
Chromium window (or any browser) that follows the editor as you type and move
the cursor.

```
 ┌──────── Zed ────────┐  LSP (stdio)  ┌────── markdown-live (Rust) ───────┐  HTTP, long-poll  ┌──── Chromium ────┐
 │ extension (WASM)    │──────────────▶│ comrak → HTML, @import, CSV, TOC  │──────────────────▶│ viewer: Mermaid, │
 │ starts the server   │ text, cursor  │ preview server on 127.0.0.1+token │   rendered HTML   │ KaTeX, hljs      │
 └─────────────────────┘               └───────────────────────────────────┘                   └──────────────────┘
```

Everything runs locally: the browser libraries are **bundled in the binary**, so
the preview works offline and your text never leaves the machine.

## Versions

The repository has one tag per version (`git checkout 0.1.0` …):

| Tag | What's in it |
|---|---|
| **0.1.0** | Core: Markdown (GFM + MPE syntax), Mermaid, LaTeX math with KaTeX, CSV files and blocks as tables |
| 0.2.0 | All extra diagrams, Chart.js, Python code with graphics, MathJax, chemistry (mhchem) |
| 0.3.0 | Images (bmp, jpeg, png, svg, heic…) and videos (mp4, mkv, webm, mov, avi) |
| 0.4.0 | Slides (`<!-- slide -->`) and exports: docx, odt, pdf, html, pptx, odp |

This is **0.1.0**.

## Install

You need Rust via [rustup](https://rustup.rs) (Zed compiles dev extensions with it).

```bash
./setup.sh            # builds + installs `markdown-zed`, adds the Option-M key binding to Zed
```

Then in Zed: **Cmd-Shift-P → "zed: install dev extension"** → choose this folder.
Open a `.md` file and press **Option-M** (Alt-M on Linux/Windows), or **Cmd-.** →
"MarkDown Live: Open preview".

By default the preview opens as a chromeless **Chromium / Chrome / Brave / Edge app
window** (`--app=`), so you can tile it next to Zed. Without a Chromium-based
browser it opens in your default browser.

## What it renders (0.1.0)

| Feature | Syntax |
|---|---|
| CommonMark + GitHub Flavored Markdown | tables, task lists, strikethrough, autolinks, footnotes |
| Extras | `==mark==`, `H~2~O`, `x^2^`, `:emoji:`, definition lists, `[[wiki links]]`, front matter |
| Alerts / admonitions | `> [!NOTE]` … and MPE's `!!! warning "Title"` (indented body) |
| Table of contents | `[TOC]` (and a TOC sidebar in the viewer, key **T**) |
| Math (KaTeX) | `$…$`, `$$…$$`, `\(…\)`, `\[…\]`, ```` ```math ```` |
| Mermaid | ```` ```mermaid ```` (flowchart, sequence, class, state, ER, gantt, pie, mindmap, …) |
| CSV → table | `@import "data.csv"`, ```` ```csv ```` / ```` ```tsv ```` blocks; options `{delimiter=";" header=false caption="…" maxRows=100}` |
| File imports | `@import "other.md"`, `@import "diagram.mmd"`, `@import "code.py" {line_begin=10 line_end=20}`, images |
| Code | highlight.js, `{.line-numbers highlight=[2,4-6]}`, copy button |
| Images & links | relative paths work; links to other `.md` files open them in the preview **and in Zed** |

Delimiters are detected automatically for CSV (`,` `;` tab `|`), quoted cells and
multi-line cells follow RFC 4180, numeric columns are right-aligned, and large
files are cut at `csvMaxRows` (5000).

See `examples/v1-core.md` for a tour.

## The viewer

| Key | Action |
|---|---|
| **T** | table-of-contents sidebar |
| **S** | scroll sync on/off (preview follows the editor cursor) |
| **D** | theme: settings → light → dark |
| **+ / − / 0** | text size |
| **R** | re-render (also re-reads imported files) |
| Cmd/Ctrl-P | print (or "Save as PDF") |

- Only changed blocks are re-rendered (morphdom): typing never makes a diagram flicker.
- One preview window follows the file you're editing. Opening the preview again
  reuses the window when it's visible; an older tab retires itself.

## Settings

In `~/.config/zed/settings.json`:

```jsonc
{
  "lsp": {
    "markdown-live": {
      "settings": {
        "browser": "",              // "" = Chromium app window if found; "default" = system browser;
                                    // or e.g. ["/Applications/Chromium.app/Contents/MacOS/Chromium", "--app={url}"]
        "theme": "auto",            // auto | light | dark
        "scrollSync": true,         // preview follows the cursor
        "followEditor": true,       // preview switches to the Markdown file you're editing
        "reusePreview": true,
        "liveUpdateDelay": 150,     // ms
        "frontMatter": "none",      // none | table | code
        "breaks": false,            // single newline = <br>
        "typographer": false,       // smart quotes/dashes
        "emoji": true,
        "mermaidTheme": "auto",     // auto | default | dark | forest | neutral
        "csvMaxRows": 5000,
        "libraries": "bundled",     // bundled (offline) | cdn (jsDelivr)
        "port": 0                   // 0 = random
      }
      // "binary": { "path": "/Users/you/.cargo/bin/markdown-zed" }
    }
  }
}
```

## CLI

```
markdown-zed preview FILE [--line N] [--new-window]    what the Option-M task runs
markdown-zed render FILE                               print the HTML (debugging)
markdown-zed serve                                     standalone preview server (auto-started)
markdown-zed doctor                                    what was detected
markdown-zed setup-zed [--key alt-m]                   (re)write the Zed task + key binding
```

`markdown-zed preview` talks to the language server that has the file open, so
the preview shows your unsaved text. Without Zed it starts a standalone server
that watches the file on disk.

## Security

- The preview server binds to 127.0.0.1 only and rejects other `Host` headers
  (DNS rebinding).
- Every request needs the random session token (URL once, then an
  `HttpOnly; SameSite=Strict` cookie).
- `/fs/…` only serves files inside the workspace or next to (one folder above)
  an open document; hidden folders (`.ssh`, `.git`, …) are refused.
- The page has a strict Content-Security-Policy: only the viewer's own scripts
  run; `<script>` tags or `onclick=` handlers inside Markdown don't.

## Troubleshooting

- **Nothing happens on Option-M**: run `markdown-zed setup-zed` again, then in
  Zed **dev: open key context view**; check `~/Library/Logs/Zed/Zed.log`.
- **"`markdown-live` was not found"**: set `lsp.markdown-live.binary.path` to the
  output of `which markdown-zed` (`~/.cargo/bin` is often not on Zed's PATH).
- **Wrong browser**: set `"browser"` (see Settings). `markdown-zed doctor` shows the choice.

## Development

```bash
cd server && cargo test            # unit tests
cargo build && python3 ../tests/e2e.py   # end-to-end: real LSP session + headless Chromium
./scripts/vendor.sh                # refresh the bundled browser libraries
```

## License

MIT. See `THIRD_PARTY_NOTICES.md` for the bundled libraries.
