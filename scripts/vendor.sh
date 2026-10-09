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
  "katex|katex@0.16.47|dist/katex.min.js dist/katex.min.css dist/fonts/*.woff2"
  "hljs|@highlightjs/cdn-assets@11.12.0|highlight.min.js styles/github.min.css styles/github-dark.min.css"
  "morphdom|morphdom@2.7.8|dist/morphdom-umd.min.js"
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
      rel="${rel#dist/}"
      mkdir -p "$OUT/$name/$(dirname "$rel")"
      cp "$src" "$OUT/$name/$rel"
      echo "$rel ${src#"$dir"/package/}" >> "$OUT/$name/SOURCES"
    done
  done
  echo "$pkg" > "$OUT/$name/VERSION"   # SOURCES maps each file to its path in the npm package (used for libraries: "cdn")
done
du -sh "$OUT"/* | sort -h
