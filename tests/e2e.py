#!/usr/bin/env python3
"""End-to-end tests for MarkDown Live.

Starts the real binary (as a language server over stdio, like Zed does, and as a
standalone server), opens the preview in headless Chromium (Playwright) and
checks what the page shows.

    python3 tests/e2e.py [path/to/markdown-live]

Needs: pip install playwright && playwright install chromium
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
BIN = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "server/target/debug/markdown-zed"
FIXTURES = ROOT / "tests/fixtures"
results = []


def check(name, cond, detail=""):
    results.append((name, bool(cond)))
    print(("  ok   " if cond else "  FAIL ") + name + ("" if cond else f"   {detail}"))


def wait_for(fn, timeout=15.0, step=0.1):
    end = time.time() + timeout
    last = None
    while time.time() < end:
        try:
            last = fn()
            if last:
                return last
        except Exception as e:  # noqa: BLE001
            last = e
        time.sleep(step)
    return None


class Env:
    def __init__(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="ml-e2e-"))
        self.work = self.tmp / "work"
        shutil.copytree(FIXTURES, self.work)
        self.env = dict(os.environ)
        self.env["XDG_CACHE_HOME"] = str(self.tmp / "cache")
        self.env["HOME"] = str(self.tmp / "home")
        (self.tmp / "home").mkdir()
        # never open a real browser from the server: the test opens the page itself
        self.env["MARKDOWN_LIVE_SETTINGS"] = json.dumps({"browser": ["true"]})

    def instances(self):
        d = self.tmp / "cache/markdown-live/instances"
        out = []
        if d.is_dir():
            for f in d.iterdir():
                try:
                    out.append(json.loads(f.read_text()))
                except Exception:  # noqa: BLE001
                    pass
        return out


class Lsp:
    def __init__(self, env, settings=None):
        self.p = subprocess.Popen([str(BIN)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env.env)
        self.id = 0
        self.send("initialize", {"processId": None, "rootUri": "file://" + str(env.work), "capabilities": {},
                                 "initializationOptions": settings or {"browser": ["true"]}}, req=True)
        self.read_response()
        self.send("initialized", {})

    def send(self, method, params, req=False):
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        if req:
            self.id += 1
            msg["id"] = self.id
        body = json.dumps(msg).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.p.stdin.flush()

    def read_message(self):
        n = None
        while True:
            line = self.p.stdout.readline()
            if not line:
                return None
            line = line.strip()
            if not line:
                break
            if line.lower().startswith(b"content-length:"):
                n = int(line.split(b":")[1])
        return json.loads(self.p.stdout.read(n))

    def read_response(self):
        while True:
            m = self.read_message()
            if m is None or ("id" in m and "method" not in m):
                return m

    def close(self):
        try:
            self.send("shutdown", {}, req=True)
            self.send("exit", {})
            self.p.wait(timeout=5)
        except Exception:  # noqa: BLE001
            self.p.kill()


def api(inst, path, body=None):
    url = f"http://127.0.0.1:{inst['port']}{path}{'&' if '?' in path else '?'}t={inst['token']}"
    data = json.dumps(body).encode() if body is not None else None
    with urllib.request.urlopen(urllib.request.Request(url, data=data, method="POST" if data else "GET"), timeout=30) as r:
        return r.read()


def uri(p):
    return "file://" + urllib.request.pathname2url(str(p))


def test_v1(page, env, lsp, inst):
    doc = env.work / "demo.md"
    text = doc.read_text()
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": text}})
    api(inst, "/api/open", {"path": str(doc)})
    page.goto(f"http://127.0.0.1:{inst['port']}/?t={inst['token']}", wait_until="commit")
    page.wait_for_selector("#ml-content h1", timeout=15000)
    check("title is set", "Demo document" in page.title(), page.title())
    check("TOC token rendered", page.locator("nav.ml-toc a").count() >= 4)
    check("GFM alert", page.locator(".markdown-alert-tip").count() == 1)
    check("MPE admonition", page.locator(".markdown-alert-warning .markdown-alert-title").inner_text().strip() == "Careful")
    check("emoji shortcode", "🚀" in page.locator("#ml-content").inner_text())
    page.wait_for_selector(".ml-mermaid svg", timeout=20000)
    check("mermaid diagram rendered", page.locator(".ml-mermaid svg").count() == 1)
    page.wait_for_selector(".katex", timeout=10000)
    check("KaTeX inline + display", page.locator(".katex").count() >= 2 and page.locator(".katex-display").count() >= 1)
    check("CSV import table", page.locator(".ml-csv caption").first.inner_text() == "Sales")
    check("CSV quoted cell", page.locator(".ml-csv td", has_text="West, coast").count() == 1)
    check("CSV numbers right-aligned", page.locator(".ml-csv td.num").count() >= 6)
    check("CSV fenced block with ; delimiter", page.locator(".ml-csv th", has_text="price").count() == 1)
    page.wait_for_selector("pre.ml-code code.hljs", timeout=10000)
    check("code highlighted", page.locator("pre.ml-code .hljs-keyword").count() >= 1)
    check("line numbers + highlighted line", page.locator("pre.line-numbers .ln").count() == 3 and page.locator("pre .hl-line").count() == 1)

    # live update from the unsaved buffer, without re-rendering unchanged diagrams
    svg_before = page.locator(".ml-mermaid svg").evaluate("e => (e.dataset.mark = 'kept', 1)")
    new_text = text.replace("## Text", "## Text changed live")
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 2}, "contentChanges": [{"text": new_text}]})
    ok = wait_for(lambda: page.locator("h2", has_text="Text changed live").count() == 1, 5)
    check("live update from editor buffer", ok)
    check("unchanged mermaid kept (no flicker)", page.locator(".ml-mermaid svg[data-mark='kept']").count() == 1)

    # mermaid edit re-renders only that diagram
    t3 = new_text.replace("A[Edit] --> B(Preview)", "A[Edit] --> B(Live preview)")
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 3}, "contentChanges": [{"text": t3}]})
    ok = wait_for(lambda: "Live preview" in page.locator(".ml-mermaid svg").text_content(), 10)
    check("edited mermaid re-rendered", ok, page.locator(".ml-mermaid").evaluate("e => e.outerHTML.slice(0, 300)"))

    # mermaid error is shown, not thrown
    t4 = t3.replace("graph LR", "graph LR\n  A[unclosed --> ((")
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 4}, "contentChanges": [{"text": t4}]})
    ok = wait_for(lambda: page.locator(".ml-mermaid .ml-error").count() == 1, 10)
    check("mermaid syntax error shown", ok, page.locator(".ml-mermaid").evaluate("e => e.outerHTML.slice(0, 400)"))
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 5}, "contentChanges": [{"text": t3}]})
    wait_for(lambda: page.locator(".ml-mermaid svg").count() == 1, 10)

    # scroll sync: cursor on the last line (code block)
    last = next(i for i, l in enumerate(t3.split("\n")) if "println" in l)
    lsp.send("textDocument/documentHighlight", {"textDocument": {"uri": uri(doc)}, "position": {"line": last, "character": 0}}, req=True)
    ok = wait_for(lambda: page.locator(".ml-cursor").count() == 1 and page.evaluate("document.getElementById('ml-scroller').scrollTop") > 100, 5)
    check("scroll sync follows the cursor", ok, page.evaluate("document.getElementById('ml-scroller').scrollTop"))
    check("cursor marker on code block", page.locator("pre.ml-code.ml-cursor").count() == 1)

    # follow the editor to another Markdown file
    other = env.work / "other.md"
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(other), "languageId": "markdown", "version": 1, "text": other.read_text()}})
    lsp.send("textDocument/documentHighlight", {"textDocument": {"uri": uri(other)}, "position": {"line": 0, "character": 0}}, req=True)
    ok = wait_for(lambda: page.locator("h1", has_text="Other page").count() == 1, 5)
    check("preview follows the editor to another file", ok)
    # link back to demo.md
    page.locator("a", has_text="back to the demo").click()
    ok = wait_for(lambda: page.locator("h1", has_text="MarkDown Live demo").count() == 1, 5)
    check("clicking a .md link opens it", ok)

    # images via /fs, outside-folder files refused
    check("relative image served", page.evaluate("fetch(document.querySelector('img[alt=logo]')?.src || '/fs/none').then(r => r.status)") == 200 if page.locator("img[alt=logo]").count() else True)
    status = page.evaluate("fetch('/fs/etc/passwd').then(r => r.status)")
    check("files outside the workspace are refused", status == 404, status)
    status = page.evaluate("fetch('/api/state', {credentials: 'omit'}).then(r => r.status)")
    check("API needs the session token", status == 403, status)

    # theme toggle re-renders mermaid in dark
    page.locator("#ml-btn-theme").click()
    page.locator("#ml-btn-theme").click()
    ok = wait_for(lambda: page.evaluate("document.documentElement.dataset.theme") == "dark", 3)
    check("dark theme", ok)

    # TOC sidebar
    page.keyboard.press("t")
    check("TOC sidebar lists headings", page.locator("#ml-toc a").count() >= 4)


def main():
    if not BIN.exists():
        sys.exit(f"build first: {BIN} not found")
    env = Env()
    version = subprocess.run([str(BIN), "--version"], capture_output=True, text=True).stdout.strip()
    print(f"testing {version}")
    lsp = Lsp(env)
    inst = wait_for(lambda: env.instances() and env.instances()[0], 5)
    if not inst:
        sys.exit("server did not register")
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 1100, "height": 800})
        errors = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        if os.environ.get("DEBUG"):
            page.on("console", lambda m: print("   [console]", m.type, m.text))
            page.on("requestfailed", lambda r: print("   [reqfail]", r.url, r.failure))
        try:
            test_v1(page, env, lsp, inst)
            extra = os.environ.get("ML_EXTRA_TESTS")
            if extra:
                import importlib.util
                for name in extra.split(","):
                    spec = importlib.util.spec_from_file_location(name, ROOT / "tests" / f"{name}.py")
                    mod = importlib.util.module_from_spec(spec)
                    spec.loader.exec_module(mod)
                    mod.run(page, env, lsp, inst, check, wait_for, api, uri)
        finally:
            check("no JavaScript errors", not errors, errors[:3])
            browser.close()
            lsp.close()
    failed = [n for n, ok in results if not ok]
    print(f"\n{len(results) - len(failed)}/{len(results)} checks passed")
    shutil.rmtree(env.tmp, ignore_errors=True)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
