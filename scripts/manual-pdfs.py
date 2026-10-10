#!/usr/bin/env python3
"""Makes docs/*.pdf from docs/*.md with MarkDown Live itself: the real language server
renders each manual, headless Chromium lays out the print copy (the same one Cmd-P
prints) and saves it as A4 PDF.

    python3 scripts/manual-pdfs.py [path/to/markdown-zed]

Needs: pip install playwright && playwright install chromium; Python with matplotlib
(code chunks); Java + plantuml.jar for the PlantUML example (MARKDOWN_LIVE_SETTINGS
can point "plantumlJar" at it).
"""
import json
import os
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tests"))
import e2e  # noqa: E402  (reuses the test helpers: Lsp, api, wait_for)
from playwright.sync_api import sync_playwright  # noqa: E402

BIN = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "server/target/debug/markdown-zed"
e2e.BIN = BIN
MANUALS = ["manual-markdown.md", "manual-terminal.md"]


def main():
    env = e2e.Env()
    env.work = ROOT  # render the real files (they refer to ../examples)
    settings = {"browser": ["true"], "enableScriptExecution": True}
    settings.update(json.loads(os.environ.get("MARKDOWN_LIVE_SETTINGS", "{}")))
    lsp = e2e.Lsp(env, settings)
    inst = e2e.wait_for(lambda: env.instances() and env.instances()[0], 10)
    with sync_playwright() as p:
        browser = p.chromium.launch(args=["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"])
        page = browser.new_page(viewport={"width": 1100, "height": 900})
        for name in MANUALS:
            doc = ROOT / "docs" / name
            lsp.send("textDocument/didOpen", {"textDocument": {"uri": e2e.uri(doc), "languageId": "markdown", "version": 1, "text": doc.read_text()}})
            e2e.api(inst, "/api/open", {"path": str(doc)})
            h1 = next(l[2:].strip() for l in doc.read_text().splitlines() if l.startswith("# ")).replace("`", "")
            page.goto(f"http://127.0.0.1:{inst['port']}/?t={inst['token']}")
            page.wait_for_function("t => (document.querySelector('#ml-content h1')?.textContent || '').includes(t)", arg=h1[:20], timeout=30000)
            page.wait_for_selector("#ml-content h1", timeout=20000)
            page.evaluate("document.getElementById('ml-print-frame')?.remove()")
            page.evaluate("ML.print()")
            page.wait_for_selector("#ml-print-frame[data-ready]", state="attached", timeout=180000)
            html = page.evaluate("document.getElementById('ml-print-frame').srcdoc")
            out = browser.new_page()
            out.goto(page.url, wait_until="commit")
            out.evaluate("h => { document.open(); document.write(h); document.close(); }", html)
            out.wait_for_load_state("load")
            # keep each ### section (an example and its result) on one page when it fits
            out.evaluate("""() => {
                const body = document.querySelector('article.markdown-body');
                let group = null;
                for (const el of [...body.children]) {
                    // an h3 right after its h2 stays with it
                    const joins = el.tagName === 'H3' && group && group.children.length === 1 && group.firstElementChild.tagName === 'H2';
                    if (/^H[1-3]$/.test(el.tagName) && !joins) group = null;
                    if ((el.tagName === 'H2' || el.tagName === 'H3') && !joins) {
                        group = document.createElement('div');
                        group.style.breakInside = 'avoid-page';
                        el.before(group);
                    }
                    if (group) group.appendChild(el);
                }
                // headings never at the bottom of a page
                for (const h of body.querySelectorAll('h1, h2, h3')) h.style.breakAfter = 'avoid-page';
            }""")
            time.sleep(1.5)
            pdf = ROOT / "docs" / name.replace(".md", ".pdf")
            out.pdf(path=str(pdf), format="A4", print_background=True, margin={"top": "16mm", "bottom": "16mm", "left": "15mm", "right": "15mm"},
                    display_header_footer=True, header_template="<span></span>",
                    footer_template='<div style="font-size:8px;width:100%;text-align:center;color:#888"><span class="pageNumber"></span> / <span class="totalPages"></span></div>')
            out.close()
            print("wrote", pdf.relative_to(ROOT))
        browser.close()
    lsp.close()


if __name__ == "__main__":
    main()
