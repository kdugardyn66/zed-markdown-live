# Third-party notices

MarkDown Live bundles these browser libraries in its binary (`server/vendor/`,
see `scripts/vendor.sh` for the exact versions):

| Library | Used for | License |
|---|---|---|
| [Mermaid](https://github.com/mermaid-js/mermaid) | diagrams | MIT |
| [KaTeX](https://github.com/KaTeX/KaTeX) | math | MIT |
| [highlight.js](https://github.com/highlightjs/highlight.js) | code highlighting | BSD-3-Clause |
| [morphdom](https://github.com/patrick-steele-idem/morphdom) | flicker-free updates | MIT |

Rust crates (compiled in): [comrak](https://github.com/kivikakk/comrak) (BSD-2-Clause),
serde / serde_json, ureq, flate2, libc (MIT or Apache-2.0).

Syntax compatible with [Markdown Preview Enhanced](https://shd101wyy.github.io/markdown-preview-enhanced/)
(`@import`, `!!!` admonitions, `[TOC]`, code-fence attributes). No MPE code is included.
