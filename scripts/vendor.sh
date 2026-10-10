#!/usr/bin/env bash
# Downloads the browser libraries that MarkDown Live bundles into its binary
# (server/vendor/). The files are committed, so you only need this to update a
# library: change the version below and run ./scripts/vendor.sh
# Needs: npm (only for `npm pack`, nothing is installed), tar.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="server/vendor"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# name|npm package@version|files to copy (relative to package/, globs allowed) -> $OUT/<name>/
LIBS=(
  "mermaid|mermaid@11.17.2|dist/mermaid.min.js"
  "katex|katex@0.16.47|dist/katex.min.js dist/katex.min.css dist/fonts/*.woff2 dist/contrib/mhchem.min.js"
  "hljs|@highlightjs/cdn-assets@11.12.0|highlight.min.js styles/github.min.css styles/github-dark.min.css"
  "morphdom|morphdom@2.7.8|dist/morphdom-umd.min.js"
  # v2: more diagrams, charts, MathJax, chemistry
  "mathjax|mathjax@3.2.2|es5/tex-svg-full.js"
  "chartjs|chart.js@4.5.1|dist/chart.umd.min.js"
  "vega|vega@5.33.1|build/vega.min.js"
  "vega-lite|vega-lite@5.23.0|build/vega-lite.min.js"
  "vega-embed|vega-embed@6.29.0|build/vega-embed.min.js"
  "viz|@viz-js/viz@3.14.0|lib/viz-standalone.js"
  "wavedrom|wavedrom@3.7.0|wavedrom.min.js skins/default.js"
  "smiles|smiles-drawer@2.1.7|dist/smiles-drawer.min.js"
  "plotly|plotly.js-dist-min@3.7.0|plotly.min.js"
  # v6: 3D molecules
  "molstar|molstar@5.11.0|build/viewer/molstar.js build/viewer/molstar.css build/viewer/theme/light.css"
)

# Libraries without a browser build on npm: bundled here with esbuild.
# name|packages to install|entry|global
BUILDS=(
  "flowchart|flowchart.js@1.18.0 raphael@2.3.0|node_modules/flowchart.js/index.js"
)

for spec in "${LIBS[@]}"; do
  IFS='|' read -r name pkg files <<<"$spec"
  echo "==> $name ($pkg)"
  dir="$TMP/$name"; mkdir -p "$dir"
  (cd "$dir" && npm pack --silent "$pkg" >/dev/null && tar -xzf ./*.tgz)
  rm -rf "${OUT:?}/$name"; mkdir -p "$OUT/$name"
  for f in $files; do
    for src in "$dir"/package/$f; do
      rel="${src#"$dir"/package/}"
      rel="${rel#dist/}"; rel="${rel#build/}"; rel="${rel#lib/}"; rel="${rel#es5/}"
      mkdir -p "$OUT/$name/$(dirname "$rel")"
      cp "$src" "$OUT/$name/$rel"
      echo "$rel ${src#"$dir"/package/}" >> "$OUT/$name/SOURCES"
    done
  done
  echo "$pkg" > "$OUT/$name/VERSION"   # SOURCES maps each file to its path in the npm package (used for libraries: "cdn")
done
for spec in "${BUILDS[@]}"; do
  IFS='|' read -r name pkgs entry <<<"$spec"
  echo "==> $name ($pkgs, bundled with esbuild)"
  dir="$TMP/build-$name"; mkdir -p "$dir"
  (cd "$dir" && npm init -y >/dev/null && npm install --silent --no-audit --no-fund $pkgs esbuild@0.25 >/dev/null \
    && npx esbuild "$entry" --bundle --minify --format=iife --outfile=out.min.js --log-level=warning)
  rm -rf "${OUT:?}/$name"; mkdir -p "$OUT/$name"
  cp "$dir/out.min.js" "$OUT/$name/$name.min.js"
  echo "$pkgs" > "$OUT/$name/VERSION"
done
du -sh "$OUT"/* | sort -h
