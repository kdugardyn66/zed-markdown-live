# MarkDown Live — live Markdown preview for Zed

Zed extensions can't open a webview, so MarkDown Live does what PlantUML Live does:
a small Rust language server renders the **unsaved** buffer and shows it in a
Chromium window (or any browser) that follows the editor as you type and move
the cursor.

```
 ┌──────── Zed ────────┐  LSP (stdio)  ┌────── markdown-live (Rust) ───────┐  HTTP, long-poll  ┌──── Chromium ────┐
 │ extension (WASM)    │──────────────▶│ comrak → HTML, @import, CSV, TOC  │──────────────────▶│ viewer: Mermaid, │
 │ starts the server   │ text, cursor  │ PlantUML/Kroki + code chunks (bg) │   rendered HTML   │ KaTeX/MathJax,   │
 └─────────────────────┘               │ preview server on 127.0.0.1+token │                   │ Viz, Vega, Chart │
                                       └───────────────────────────────────┘                   └──────────────────┘
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

This is **0.3.0**.

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

## Diagrams, charts, chemistry and code (0.2.0)

| Block | Rendered by | Notes |
|---|---|---|
| ```` ```plantuml ```` / `puml`, `ditaa`, `salt`, `mindmap`, `gantt`, `wbs` | PlantUML on **your Java** (server side) | `@startuml` is added when missing. Finds `plantuml.jar` from settings, `$PLANTUML_JAR`, Homebrew, or PlantUML Live's copy. `"plantumlRender": "online"` uses `plantumlServer` instead |
| ```` ```dot ```` / `graphviz`, `neato`, `circo`, `fdp`, `twopi` | Viz.js (Graphviz in WebAssembly) | `{engine=neato}` |
| ```` ```vega-lite ```` / ```` ```vega ```` | Vega-Embed | `data.url` is relative to the document |
| ```` ```wavedrom ```` | WaveDrom | timing diagrams |
| ```` ```flow ```` | flowchart.js | MPE's `flow` syntax |
| ```` ```chart ```` | Chart.js | the Chart.js config; also `@import "data.csv" {chart="bar" title="…"}` and ```` ```csv {chart="line"} ```` |
| ```` ```plotly ```` | Plotly | `{data, layout}` |
| ```` ```smiles ```` | SmilesDrawer | one molecule per line: `CCO ethanol` |
| ```` ```kroki-<type> ```` / `{kroki=true}` | [Kroki](https://kroki.io) (**online**, only for these blocks) | d2, bpmn, nomnoml, svgbob, excalidraw, … |
| `$\ce{H2O}$`, `$\pu{12 kJ/mol}$` | mhchem in KaTeX and MathJax | chemical formulas and equations |
| any math | KaTeX (default) or **MathJax** | key **M** or the KaTeX/MathJax button switches; `"math": "mathjax"` as default |

The JSON-based blocks accept relaxed JSON (comments, unquoted keys, single quotes,
trailing commas). Nothing in them is evaluated as code.

**Code chunks** (MPE syntax): ```` ```python {cmd=true} ````. They run in the
document's folder, in the background (a slow chunk never blocks typing; it runs
once the text has been still for 0.7 s).

- `print()` output appears under the code; `{output="markdown"}`, `"html"` or `"none"`; `{hide=true}` hides the code.
- **matplotlib / seaborn** figures (`plt.show()` or left open) appear as SVG; **Plotly** `fig.show()` appears as an interactive chart.
- Python is `.venv/bin/python` from the workspace when present, else `python3` (setting `"python"`).
  `bash`, `node`, `ruby`, `perl`, `Rscript`, `julia`, `php`, `lua` also run; `{cmd="/path/to/tool"}` picks the interpreter, `{args=["-x"]}` adds arguments.
- **Off by default.** A banner offers "Run code chunks for this session"; `"enableScriptExecution": true` makes it permanent.
- ⟳ (key R) runs chunks again and re-renders PlantUML.

See `examples/v2-diagrams.md`.

## Pictures and videos (0.3.0)

```markdown
![logo](img/logo.png){width=200}            pictures: png, jpeg, gif, bmp, svg, webp, avif, heic/heif, tiff
![Holiday](photos/IMG_0042.HEIC "Caption")   alone in a paragraph + title → figure with caption
![demo](videos/demo.mkv){width=640}          videos: mp4, webm, mkv, mov, avi, wmv, flv, mpg, m4v, ogv
![intro](clip.mp4){autoplay loop start=12 poster="cover.jpg"}
![talk](https://youtu.be/VIDEO_ID)           YouTube / Vimeo links become players
![song](audio/theme.mp3)                     audio: mp3, wav, ogg, m4a, flac, opus
@import "photos/IMG_0042.HEIC" {width=300}   MPE @import works for all of these
```

- **HEIC/HEIF and TIFF** are converted to JPEG/PNG on the fly: macOS `sips` (built in);
  elsewhere ImageMagick, `heif-convert`, ffmpeg or Pillow — whichever is installed.
- **Videos**: MarkDown Live checks what's inside with `ffprobe`. Files the browser can
  play are served as they are (with seeking). Others are **remuxed** (seconds, no quality
  loss: e.g. H.264 in MKV → MP4) or **transcoded** in the background, with a
  "⏳ Converting…" placeholder meanwhile. Needs `ffmpeg` (`brew install ffmpeg`).
- **Chrome vs Chromium**: Google Chrome, Edge and Brave play H.264/AAC (MP4, MOV).
  Open-source **Chromium** builds don't, so for Chromium those videos are converted to
  VP9/Opus WebM (the page tells the server what it can play). For big videos, prefer
  WebM files or point `"browser"` at Google Chrome.
- Converted copies are cached in `~/Library/Caches/markdown-live/media/`
  (`~/.cache/markdown-live/media/` on Linux), keyed by file size and date.
- Click a picture to see it full size (Esc closes).

## The viewer

| Key | Action |
|---|---|
| **T** | table-of-contents sidebar |
| **S** | scroll sync on/off (preview follows the editor cursor) |
| **D** | theme: settings → light → dark |
| **+ / − / 0** | text size |
| **R** | re-render: re-reads imported files, re-runs code chunks, re-renders PlantUML |
| **M** | math engine: KaTeX ↔ MathJax |
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
        "math": "katex",            // katex | mathjax
        "plantumlRender": "local",  // local (Java) | online | auto (local when found)
        "plantumlServer": "https://www.plantuml.com/plantuml",   // or a Kroki / self-hosted server
        "plantumlJar": "",          // "" = autodetect
        "java": "",
        "krokiServer": "https://kroki.io",
        "enableScriptExecution": false,  // run ```python {cmd=true} chunks without asking
        "codeTimeout": 30,          // seconds per chunk
        "python": "",               // "" = workspace .venv, else python3
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
- **Code chunks never run unless you allow them** (setting, banner, or the code
  action "MarkDown Live: Run code chunks (this session)").
- Nothing is sent online unless you ask: PlantUML is local by default, and Kroki is
  only used for `kroki-…` blocks. YouTube/Vimeo players load only when you link a video.

## Troubleshooting

- **Nothing happens on Option-M**: run `markdown-zed setup-zed` again, then in
  Zed **dev: open key context view**; check `~/Library/Logs/Zed/Zed.log`.
- **"`markdown-live` was not found"**: set `lsp.markdown-live.binary.path` to the
  output of `which markdown-zed` (`~/.cargo/bin` is often not on Zed's PATH).
- **Wrong browser**: set `"browser"` (see Settings). `markdown-zed doctor` shows the choice.
- **PlantUML says it needs Java**: `brew install plantuml` (installs Java too), or point
  `"plantumlJar"` / `"java"` at your copies. `markdown-zed doctor` shows what it found.
- **A video stays on "Converting…"** or shows a note: install ffmpeg (`brew install ffmpeg`);
  `markdown-zed doctor` shows whether it was found.
- **matplotlib not found**: code chunks use the workspace `.venv` when there is one;
  install packages there, or set `"python"`.

## Development

```bash
cd server && cargo test            # unit tests
cargo build && python3 ../tests/e2e.py   # end-to-end: real LSP session + headless Chromium
./scripts/vendor.sh                # refresh the bundled browser libraries
```

## License

MIT. See `THIRD_PARTY_NOTICES.md` for the bundled libraries.
