# Changelog

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
