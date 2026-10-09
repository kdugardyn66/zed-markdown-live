# Third-party notices

MarkDown Live bundles these browser libraries in its binary (`server/vendor/`,
see `scripts/vendor.sh` for the exact versions):

| Library | Used for | License |
|---|---|---|
| [Mermaid](https://github.com/mermaid-js/mermaid) | diagrams | MIT |
| [KaTeX](https://github.com/KaTeX/KaTeX) | math | MIT |
| [highlight.js](https://github.com/highlightjs/highlight.js) | code highlighting | BSD-3-Clause |
| [morphdom](https://github.com/patrick-steele-idem/morphdom) | flicker-free updates | MIT |
| [MathJax](https://github.com/mathjax/MathJax) | math (alternative engine) | Apache-2.0 |
| [Chart.js](https://github.com/chartjs/Chart.js) | charts | MIT |
| [Vega](https://github.com/vega/vega), [Vega-Lite](https://github.com/vega/vega-lite), [Vega-Embed](https://github.com/vega/vega-embed) | charts | BSD-3-Clause |
| [Viz.js](https://github.com/mdaines/viz-js) (Graphviz) | graphs | MIT (Graphviz: EPL-1.0) |
| [WaveDrom](https://github.com/wavedrom/wavedrom) | timing diagrams | MIT |
| [flowchart.js](https://github.com/adrai/flowchart.js) + [Raphaël](https://github.com/DmitryBaranovskiy/raphael) | flowcharts | MIT |
| [SmilesDrawer](https://github.com/reymond-group/smilesDrawer) | chemical structures | MIT |
| [Plotly.js](https://github.com/plotly/plotly.js) | interactive charts | MIT |
| [reveal.js](https://github.com/hakimel/reveal.js) | slides | MIT (theme fonts: SIL OFL) |

Exports call external tools when installed: pandoc (GPL-2.0+), LibreOffice (MPL-2.0),
Chrome/Chromium, ffmpeg. They are not bundled.

Rust crates (compiled in): [comrak](https://github.com/kivikakk/comrak) (BSD-2-Clause),
serde / serde_json, ureq, flate2, libc (MIT or Apache-2.0).

Syntax compatible with [Markdown Preview Enhanced](https://shd101wyy.github.io/markdown-preview-enhanced/)
(`@import`, `!!!` admonitions, `[TOC]`, code-fence attributes). No MPE code is included.
