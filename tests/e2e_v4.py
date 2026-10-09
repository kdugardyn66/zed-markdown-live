"""v4/v5 checks: slides view, insert slide, printing."""


def run(page, env, lsp, inst, check, wait_for, api, uri):
    doc = env.work / "v4.md"
    text = doc.read_text()
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": text}})
    api(inst, "/api/open", {"path": str(doc)})
    ok = wait_for(lambda: page.locator("h1", has_text="Slides and printing").count() == 1, 10)
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
    page.keyboard.press("p")
    wait_for(lambda: page.locator("#ml-deck .slides > section").count() == 5, 15)
    page.keyboard.press("Control+p")
    ok = wait_for(lambda: page.locator("#ml-print-frame[data-ready='slides']").count() == 1, 60)
    check("print in slide view (Ctrl-P) → one page per slide (6)", ok and fr.locator("section.ml-page").count() == 6, fr.locator("section.ml-page").count() if ok else 0)
    check("print slides: speaker notes left out", ok and fr.locator("aside.notes").count() == 0)
    page.keyboard.press("p")
    wait_for(lambda: page.locator("#ml-scroller").is_visible(), 5)

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
