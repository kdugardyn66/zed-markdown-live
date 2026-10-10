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
| **0.2.0** | All extra diagrams, Chart.js, Python code with graphics, MathJax, chemistry (mhchem) |
| **0.3.0** | Images (bmp, jpeg, png, svg, heic…) and videos (mp4, mkv, webm, mov, avi) |
| **0.4.0** | Slides (`<!-- slide -->`, live reveal.js view) and exports: pdf, docx, odt, html, pptx, odp, slide pdf/html |
| **0.5.0** | Exports replaced by printing (document or one slide per page); shorter Python error messages |
| **0.6.0** | 3D molecules (Mol*), Graph / Diagram \| Code switch for `hide=true` chunks; on-screen slide view removed |
| **0.6.1** | The preview always uses your Zed settings (also when served by the standalone server) |
| **0.7.0** | Slides in frames with notes; printing: portrait = slide + notes, landscape = notes on the next page |
| **0.7.1** | Manuals (`docs/`), long code lines wrap when printing |

This is **0.7.1**.

**Manuals:** [Writing Markdown in Zed](docs/manual-markdown.md) ([PDF](docs/manual-markdown.pdf)) —
every feature with the Markdown and its result — and [Using `markdown-zed` in the
Terminal](docs/manual-terminal.md) ([PDF](docs/manual-terminal.pdf)). Open the `.md`
versions in Zed with Option-M to see the examples live.

## Install

You need Rust via [rustup](https://rustup.rs) (Zed compiles dev extensions with it).

```bash
./setup.sh            # builds + installs `markdown-zed`, adds the Option-M / Option-Shift-N / Option-Shift-P keys to Zed
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

- `print()` output appears under the code; `{output="markdown"}`, `"html"` or `"none"`.
- `{hide=true}` shows only the result, in a frame with two buttons: **Graph / Diagram**
  (or **Output** for text) and **Code**. The choice is kept while you type; printing shows the result.
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

## 3D molecules (0.6.0)

[Mol*](https://molstar.org), the PDB's viewer, bundled (works offline):

````markdown
@import "protein.pdb" {height=400 title="My protein" spin=true}

```sdf
…paste an SDF / MOL block…
```

```molstar {pdb=1CRN}
```
````

- Files: `.pdb`, `.cif`/`.mmcif`, `.bcif`, `.sdf`, `.mol`, `.mol2`, `.xyz`, `.gro`, `.pdbqt`
  (`@import`, loaded from disk, so large files are fine).
- Blocks: the format as language (```` ```pdb ````, ```` ```xyz ```` …) or ```` ```molstar ````
  (format detected; `{format=mol2}` to choose). `{code=true}` shows the text as code instead.
- `{pdb=ID}` downloads an entry from the RCSB PDB: the **only** molecule block that goes online.
- Drag to rotate, scroll to zoom. Printed as a picture of the current view.
- 2D structures from SMILES: ```` ```smiles ```` (0.2.0).

See `examples/v6-molecules.md`.

## Slides and notes (0.7.0)

When a document has MPE slide markers (**Option-Shift-N** in Zed inserts one, or Cmd-. →
"MarkDown Live: Insert new slide"), every slide is shown in a **frame** (16:9, it grows
when the content is taller), numbered, with its **notes** under it:

```markdown
# First slide

Text on the slide.

Note:
What to say about the first slide.
Everything from "Note:" to the next marker is the note.

<!-- slide -->
## Second slide

<aside class="notes">Notes can also be written like this (reveal.js / MPE style).</aside>

<!-- slide class="center" data-background-color="#fff8c5" -->
## A slide with a background colour
```

The content before the first marker is the first slide. (0.4.0–0.5.0 had a reveal.js
slide view; it was removed in 0.6.0.)

**Printing slides** (pick the orientation in the print dialog):

- **Portrait**: one slide per page, in its frame, with "Note:" and the notes under it.
- **Landscape**: the slide fills the page; its notes are printed on the next page.

## Print

Press **Cmd-P** (Ctrl-P) or the printer icon in the preview, press **Option-Shift-P** in
Zed, use Cmd-. → "MarkDown Live: Print", or the CLI: `markdown-zed print notes.md`.
The browser's print dialog opens; choose a printer, or **Save as PDF** for a PDF file.

- The document as the preview shows it, always in the light theme, without the toolbar.
- Slides: one per page with their notes (see "Slides and notes" for portrait vs landscape).
- **Videos are left out** (a "▶ video: …" line marks where they were). Chart.js and
  Plotly charts and 3D molecules are printed as pictures; `hide=true` chunks print their result.
- From Zed or the CLI, the preview opens first when it isn't open yet.

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
markdown-zed print FILE                                what Option-Shift-P runs (opens the print dialog)
markdown-zed insert-slide FILE --line N                what Option-Shift-N runs
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
  only used for `kroki-…` blocks, the RCSB PDB only for `{pdb=ID}` molecule blocks.
  YouTube/Vimeo players load only when you link a video.

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
- **3D molecules show "WebGL turned off"**: enable hardware acceleration in the browser
  settings (Chrome: Settings → System → "Use graphics acceleration").
- **Option-Shift-P does nothing visible**: the print dialog opens in the preview window;
  bring that window to the front.
- **matplotlib not found**: code chunks use the workspace `.venv` when there is one;
  install packages there, or set `"python"`.

## Development

```bash
cd server && cargo test            # unit tests
cargo build && cd ..
ML_EXTRA_TESTS=e2e_v2,e2e_v3,e2e_v4 python3 tests/e2e.py   # end-to-end: real LSP session + headless Chromium
#   ML_TEST_SETTINGS='{"plantumlJar": "…", "chrome": "…"}' points the tests at local tools
./scripts/vendor.sh                # refresh the bundled browser libraries
```

## License

MIT. See `THIRD_PARTY_NOTICES.md` for the bundled libraries.
