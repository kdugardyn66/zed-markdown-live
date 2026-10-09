"""v4 checks: slides view, insert slide, exports (html, pdf, docx, odt, pptx, odp, slides)."""
import os
import subprocess
import zipfile


def pdf_pages(p):
    out = subprocess.run(["pdfinfo", str(p)], capture_output=True, text=True).stdout
    for line in out.splitlines():
        if line.startswith("Pages:"):
            return int(line.split()[1])
    return 0


def run(page, env, lsp, inst, check, wait_for, api, uri):
    doc = env.work / "v4.md"
    text = doc.read_text()
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": text}})
    api(inst, "/api/open", {"path": str(doc)})
    ok = wait_for(lambda: page.locator("h1", has_text="Slides and exports").count() == 1, 10)
    check("v4 document shown", ok)
    check("slide breaks in the document view", page.locator(".ml-slide-break").count() == 5)
    check("slides button shown", wait_for(lambda: page.locator("#ml-btn-slides").is_visible(), 5))

    # live slides view
    page.keyboard.press("p")
    ok = wait_for(lambda: page.locator("#ml-deck .slides > section").count() == 5, 15)
    check("slides view: 5 horizontal slides (one vertical stack)", ok, page.locator("#ml-deck .slides > section").count())
    check("vertical slide nested", page.locator("#ml-deck .slides > section > section").count() == 2)
    check("slide background attribute kept", page.locator("#ml-deck section[data-background-color='#fff8c5']").count() == 1)
    check("mermaid diagram inside a slide", wait_for(lambda: page.locator("#ml-deck section svg").count() >= 1, 10))
    code_line = next(i for i, l in enumerate(text.split("\n")) if "def hello" in l)
    lsp.send("textDocument/documentHighlight", {"textDocument": {"uri": uri(doc)}, "position": {"line": code_line, "character": 0}}, req=True)
    ok = wait_for(lambda: page.locator("#ml-deck .slides > section.present h2", has_text="Code").count() == 1, 5)
    check("cursor in the editor moves the slides", ok)
    page.keyboard.press("p")
    check("back to the document view", wait_for(lambda: page.locator("#ml-scroller").is_visible(), 5))

    # exports from the menu
    outputs = {
        "html": "v4.html", "pdf": "v4.pdf", "docx": "v4.docx", "odt": "v4.odt",
        "pptx": "v4.pptx", "odp": "v4.odp", "slides-pdf": "v4.slides.pdf", "slides-html": "v4.slides.html",
    }
    for fmt, name in outputs.items():
        out = env.work / name
        page.locator("#ml-btn-export").click()
        page.locator(f"#ml-export-menu button[data-format='{fmt}']").click()
        ok = wait_for(lambda: out.exists() and out.stat().st_size > 0 and page.locator("#ml-toast", has_text="Saved " + name).is_visible(), 240)
        check(f"export {fmt} → {name}", ok, page.locator("#ml-toast").inner_text())
    html = (env.work / "v4.html").read_text()
    check("html: self-contained (pictures inlined, no /fs/ links)", "/fs/" not in html and "data:image/png;base64" in html)
    check("html: diagrams and math inside", "<svg" in html and "katex" in html)
    check("html: video left out", "<video" not in html and "a clip" in html)
    check("pdf has pages", pdf_pages(env.work / "v4.pdf") >= 1)
    txt = subprocess.run(["pdftotext", str(env.work / "v4.pdf"), "-"], capture_output=True, text=True).stdout
    check("pdf has the text", "Made with MarkDown Live" in txt and "North" in txt, txt[:200])
    with zipfile.ZipFile(env.work / "v4.docx") as z:
        xml = z.read("word/document.xml").decode()
        media = [n for n in z.namelist() if n.startswith("word/media/")]
    check("docx: equations (OMML)", "<m:oMath" in xml)
    check("docx: diagram + picture as images", len(media) >= 2, media)
    check("docx: table from CSV", "<w:tbl>" in xml and "North" in xml)
    check("docx: no video", "clip.webm" not in "".join(media))
    with zipfile.ZipFile(env.work / "v4.pptx") as z:
        slides = [n for n in z.namelist() if n.startswith("ppt/slides/slide") and n.endswith(".xml")]
        notes = [n for n in z.namelist() if n.startswith("ppt/notesSlides/")]
    check("pptx: one slide per <!-- slide --> (6)", len(slides) == 6, len(slides))
    check("pptx: speaker notes", len(notes) >= 1)
    with zipfile.ZipFile(env.work / "v4.odp") as z:
        check("odp: valid OpenDocument presentation", "content.xml" in z.namelist())
    check("slides pdf: one page per slide (6)", pdf_pages(env.work / "v4.slides.pdf") == 6, pdf_pages(env.work / "v4.slides.pdf"))
    stxt = subprocess.run(["pdftotext", str(env.work / "v4.slides.pdf"), "-"], capture_output=True, text=True).stdout
    check("slides pdf has the text", "Markdown" in stxt and "hello" in stxt, stxt[:200])
    deck = (env.work / "v4.slides.html").read_text()
    check("slides html: reveal.js inlined", "Reveal.initialize" in deck and deck.count("<section") >= 7)

    # export from the editor (code action) while the preview is open
    (env.work / "v4.pdf").unlink()
    lsp.send("workspace/executeCommand", {"command": "markdownLive.export.pdf", "arguments": [uri(doc), 0]}, req=True)
    msg = None
    end = __import__("time").time() + 120
    while __import__("time").time() < end:
        m = lsp.read_message()
        if m and m.get("method") == "window/showMessage":
            msg = m["params"]["message"]
            break
    check("export from Zed (code action) → message", msg and "saved" in msg and (env.work / "v4.pdf").exists(), msg)

    # insert a slide through the language server
    lsp.send("workspace/executeCommand", {"command": "markdownLive.insertSlide", "arguments": [uri(doc), 3]}, req=True)
    edit = None
    end = __import__("time").time() + 10
    while __import__("time").time() < end:
        m = lsp.read_message()
        if m and m.get("method") == "workspace/applyEdit":
            edit = m["params"]["edit"]
            break
    ch = list(edit["changes"].values())[0][0] if edit else {}
    check("insert slide → workspace/applyEdit", ch.get("newText", "").startswith("<!-- slide -->") and ch["range"]["start"]["line"] == 3, edit)

    # export without any preview open: a headless browser renders it
    page.goto("about:blank")
    wait_for(lambda: False, 5)  # let the server notice the page is gone
    (env.work / "v4.docx").unlink()
    out = api(inst, "/api/request-export", {"path": str(doc), "format": "docx"}, timeout=200)
    check("export with no preview open (headless browser)", (env.work / "v4.docx").exists(), out[:200])
