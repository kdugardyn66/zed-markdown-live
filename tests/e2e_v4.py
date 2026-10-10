"""v4-v6 checks: slide markers, insert slide, printing."""


def run(page, env, lsp, inst, check, wait_for, api, uri):
    doc = env.work / "v4.md"
    text = doc.read_text()
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": text}})
    api(inst, "/api/open", {"path": str(doc)})
    ok = wait_for(lambda: page.locator("h1", has_text="Slides and printing").count() == 1, 10)
    check("v4 document shown", ok)
    check("each slide in a frame (6)", page.locator(".ml-slide-wrap > .ml-slide").count() == 6, page.locator(".ml-slide").count())
    check("slide numbers", page.locator(".ml-slide .ml-slide-num").all_inner_texts() == [str(i) for i in range(1, 7)])
    notes = page.locator(".ml-slide-wrap .ml-notes")
    check("notes under their slide (aside + Note: paragraph)", notes.count() == 2 and "Say hello first." in notes.nth(0).inner_text() and "Euler" in notes.nth(1).inner_text(), notes.all_inner_texts())
    check("notes start with 'Note:' on its own line", notes.nth(0).locator(".ml-notes-label").inner_text() == "Note:")
    check("Note: paragraph is not in the slide", "Euler" not in page.locator(".ml-slide").nth(2).inner_text())
    check("slide background attribute", "rgb(255, 248, 197)" in page.locator(".ml-slide").nth(4).evaluate("e => getComputedStyle(e).backgroundColor"))
    check("no slide view (removed in 0.6.0)", page.locator("#ml-btn-slides").count() == 0 and page.locator("#ml-deck").count() == 0)
    page.keyboard.press("p")
    check("P does nothing", wait_for(lambda: False, 1) is None and page.locator("#ml-deck").count() == 0)

    # printing: a hidden frame with a static copy, then the print dialog
    page.locator("#ml-btn-print").click()
    ok = wait_for(lambda: page.locator("#ml-print-frame[data-ready='document']").count() == 1, 60)
    check("print document → print frame ready", ok)
    fr = page.frame_locator("#ml-print-frame")
    body = fr.locator("body").inner_text() if ok else ""
    check("print: text, table and math", "Made with MarkDown Live" in body and "North" in body and fr.locator(".katex").count() >= 1, body[:200])
    check("print: diagram and picture", fr.locator("svg").count() >= 1 and fr.locator("img").count() >= 1)
    check("print: video left out", fr.locator("video").count() == 0 and "a clip" in body)
    check("print: light theme", fr.locator("html[data-theme='light']").count() == 1)
    check("print: no toolbar in the copy", fr.locator("#ml-bar").count() == 0)
    check("print: slides in frames with their notes", fr.locator(".ml-slide").count() == 6 and fr.locator(".ml-notes").count() == 2)
    check("print: notes printed", fr.locator(".ml-notes").first.is_visible())
    # lay the printed copy out on paper: portrait = slide + notes per page, landscape = notes on their own page
    html = page.evaluate("document.getElementById('ml-print-frame').srcdoc")
    p2 = page.context.browser.new_page()
    p2.goto(page.url, wait_until="commit")
    p2.evaluate("h => { document.open(); document.write(h); document.close(); }", html)
    p2.wait_for_load_state("load")
    wait_for(lambda: False, 1)
    import subprocess, tempfile, os
    def pages(**kw):
        f = tempfile.mktemp(suffix=".pdf")
        p2.pdf(path=f, format="A4", **kw)
        out = subprocess.run(["pdfinfo", f], capture_output=True, text=True).stdout
        os.unlink(f)
        return next((int(l.split()[1]) for l in out.splitlines() if l.startswith("Pages:")), 0)
    check("print portrait: one page per slide, notes under it (6 pages)", pages() == 6, pages())
    check("print landscape: notes on the next page (6 + 2 pages)", pages(landscape=True) == 8, pages(landscape=True))
    p2.close()

    # print from Zed (code action): the open preview prints
    page.evaluate("document.getElementById('ml-print-frame')?.remove()")
    lsp.send("workspace/executeCommand", {"command": "markdownLive.print", "arguments": [uri(doc), 0]}, req=True)
    ok = wait_for(lambda: page.locator("#ml-print-frame[data-ready='document']").count() == 1, 60)
    check("print from Zed (code action)", ok)
    page.evaluate("document.getElementById('ml-print-frame')?.remove()")
    wait_for(lambda: False, 2)
    check("print request taken once (not repeated)", page.locator("#ml-print-frame").count() == 0)
    check("export menu removed", page.locator("#ml-btn-export").count() == 0)

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
