# Changelog

## 0.6.1 — previews always use your Zed settings

- `markdown-zed preview` (Option-M) prefers Zed's language server, even when an older
  standalone server still has the file open. Before, a standalone server started once
  (e.g. while Zed was restarting the language server) kept serving the preview without
  your settings: "code execution is off", or the system Python instead of `"python"`.
- The standalone server, the CLI and `doctor` read the `lsp.markdown-live.settings`
  block from Zed's settings.json (comments and trailing commas allowed);
  `MARKDOWN_LIVE_SETTINGS` still overrides it. `doctor` shows the Python that will be used.

## 0.6.0 — 3D molecules, code switch, no slide view

- 3D molecules with Mol* 5.11 (bundled): `@import` of .pdb/.cif/.bcif/.sdf/.mol/.mol2/.xyz/.gro/.pdbqt,
  fenced blocks with the format as language or ```` ```molstar ```` (format detected),
  `{pdb=ID}` from the RCSB PDB (online, per block). Options `height`, `title`, `spin`, `format`;
  `{code=true}` keeps a block as code. Printed as a picture; theme switches and typing keep the viewer.
- Code chunks with `hide=true`: the result in a frame with **Graph / Diagram** (or **Output**) and
  **Code** buttons; the choice survives live updates; printing shows the result.
- Removed the on-screen slide view (P, slides button, reveal.js) and the `slideTheme` setting /
  `presentation:` front matter. `<!-- slide -->` markers stay: page breaks when printing.
- Examples: `v6-molecules.md`; the plots in `v2-diagrams.md` use `hide=true`.

## 0.5.0 — printing instead of exports

- Removed the exports (docx, odt, html, pptx, odp, slide html/pdf, PDF via headless Chrome),
  the export menu (E), Option-Shift-E, `markdown-zed export` and the settings `exportDir`,
  `pandoc`, `chrome`, `referenceDocx`, `referenceOdt`, `referencePptx`. pandoc and
  LibreOffice are no longer used.
- Print: Cmd/Ctrl-P or the printer icon prints the document (light theme, no toolbar,
  videos left out); in slide view one slide per page in the reveal.js theme.
  "Save as PDF" in the print dialog makes a PDF.
- Print from Zed: Option-Shift-P, code action "MarkDown Live: Print", `markdown-zed print FILE`.
- Python errors in code chunks only show your own code's lines (with the source line),
  not MarkDown Live's wrapper.

## 0.4.0 — slides and exports

- Slides with MPE's `<!-- slide -->` (attributes, `vertical=true`, speaker notes) and
  `presentation:` front matter; "Insert new slide" (Option-Shift-N, code action, CLI → workspace/applyEdit).
- Live reveal.js slide view in the preview (P), follows the editor cursor.
- Exports built from the rendered page: PDF (headless Chrome), Word/ODT (pandoc, real equations),
  self-contained HTML, PowerPoint (pandoc, one slide per marker, notes, two-column picture slides),
  Impress (via LibreOffice, equations as pictures), slide PDF (one page per slide) and reveal.js HTML.
  Videos are left out.
- Exports from Zed tasks / code actions / CLI; a hidden headless page renders when no preview is open.

## 0.3.0 — pictures and videos

- Pictures: png, jpeg, gif, bmp, svg, webp, avif natively; HEIC/HEIF and TIFF converted
  (sips / ImageMagick / heif-convert / ffmpeg / Pillow), cached.
- `{width=… height=… .class style=…}` on pictures; a titled picture alone in a paragraph becomes a figure.
- Videos: mp4, webm, mkv, mov, avi, wmv, flv, mpg…; ffprobe decides between playing as is,
  remuxing or transcoding (background job, cached). H.264 → VP9 WebM for Chromium builds
  without proprietary codecs.
- Audio files, YouTube/Vimeo embeds, `@import` for all media, HTTP range requests (seeking), lightbox.

## 0.2.0 — diagrams, charts, chemistry, code

- PlantUML (also ditaa/salt/mindmap/gantt/wbs) rendered with your local Java, or online on request.
- Graphviz (Viz.js), Vega and Vega-Lite, WaveDrom, flowchart.js, Chart.js, Plotly, SMILES — bundled.
- Charts straight from CSV: `@import "data.csv" {chart="bar"}`.
- Kroki for any other diagram type (online, opt-in per block).
- MathJax next to KaTeX (switch with M); mhchem (`\ce`, `\pu`) in both.
- Code chunks (`{cmd=true}`): Python with matplotlib/seaborn figures and Plotly charts,
  bash, node, R, …; background jobs, debounced, off until you allow them.
- Background job queue: slow blocks never block typing; the previous picture stays (dimmed)
  while a block re-renders; PlantUML results are cached on disk.

## 0.1.0 — core

- The server binary is `markdown-zed` (in `~/.cargo/bin`); everything else is named `markdown-live`. Preview key: Option-M (Alt-M).
- Zed extension (WASM) that attaches the `markdown-live` language server to Markdown.
- Rust server: LSP (live text, cursor, code actions), preview web server, CLI
  (`preview`, `render`, `serve`, `doctor`, `setup-zed`).
- Markdown via comrak: GFM, footnotes, alerts, `==mark==`, sub/superscript, emoji,
  definition lists, wiki links, front matter; MPE syntax: `@import`, `[TOC]`, `!!!` admonitions,
  code-fence attributes.
- Mermaid diagrams, KaTeX math, highlight.js code — bundled, offline.
- CSV/TSV as tables: `@import "x.csv"` and fenced blocks, delimiter detection, RFC 4180 quoting.
- Viewer: Chromium app window, scroll sync, follows the active file, TOC sidebar, themes,
  zoom, flicker-free updates (morphdom), clickable `.md` links (opens them in Zed too).
- Own small HTTP server (one thread per connection): tiny_http's pool could leave a
  keep-alive connection queued, so Chromium waited forever for a script.
