"""v6 checks: 3D molecules (Mol*) and the Graph / Diagram | Code switch."""


def run(page, env, lsp, inst, check, wait_for, api, uri):
    doc = env.work / "v6.md"
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": doc.read_text()}})
    api(inst, "/api/open", {"path": str(doc)})
    ok = wait_for(lambda: page.locator("h1", has_text="Molecules and code tabs").count() == 1, 10)
    check("v6 document shown", ok)

    mol = page.locator(".ml-molstar")
    ok = wait_for(lambda: page.locator(".ml-molstar[data-ready]").count() == 4, 60)
    check("Mol*: 4 viewers ready", ok, page.locator(".ml-molstar[data-ready]").count())
    check("Mol*: WebGL canvases", page.locator(".ml-molstar canvas").count() >= 4)
    errs = page.locator(".ml-molstar .ml-error").all_inner_texts()
    check("Mol*: @import sdf, ```xyz, @import pdb load without errors", len(errs) == 1 and "could not read" in errs[0], errs)
    check("Mol*: title under the viewer", mol.first.locator(".ml-molstar-title").inner_text() == "Caffeine")
    check("Mol*: height attribute", abs(mol.first.locator(".ml-molstar-box").bounding_box()["height"] - 280) < 3)
    check("```pdb {code=true} stays code", page.locator("pre.ml-code code.language-pdb").count() == 1)
    n_pixels = page.evaluate("""async () => { const el = document.querySelector('.ml-molstar'); const u = await ML.molstarImage(el);
        return u ? u.length : 0; }""")
    check("Mol*: picture of the view (for printing)", n_pixels > 5000, n_pixels)

    # dark theme keeps the viewers (only the background changes)
    page.evaluate("document.querySelector('.ml-molstar canvas').dataset.mark = 'kept'")
    page.locator("#ml-btn-theme").click()
    wait_for(lambda: False, 2)
    check("theme switch keeps the 3D viewer", page.locator(".ml-molstar canvas[data-mark='kept']").count() == 1)
    page.locator("#ml-btn-theme").click()

    # live edit elsewhere keeps the viewers
    text = doc.read_text().replace("# Molecules and code tabs", "# Molecules and code tabs!")
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 2}, "contentChanges": [{"text": text}]})
    wait_for(lambda: page.locator("h1", has_text="tabs!").count() == 1, 5)
    check("typing elsewhere keeps the 3D viewer", page.locator(".ml-molstar canvas[data-mark='kept']").count() == 1)

    # Graph / Diagram | Code
    tab = page.locator(".ml-tabbed", has=page.locator("img.ml-figure"))
    ok = wait_for(lambda: tab.count() == 1, 30)
    check("hide=true plot: Graph / Diagram | Code", ok and tab.locator(".ml-chunk-tabs button").all_inner_texts() == ["Graph / Diagram", "Code"], tab.locator(".ml-chunk-tabs button").all_inner_texts() if ok else "")
    check("plot shown, code hidden", ok and tab.locator("img.ml-figure").is_visible() and tab.locator("pre.ml-code").is_hidden())
    tab.locator("button[data-tab=code]").click()
    check("Code tab", tab.locator("pre.ml-code").is_visible() and tab.locator("img.ml-figure").is_hidden())
    text2 = text.replace("tabs!", "tabs!!")
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 3}, "contentChanges": [{"text": text2}]})
    wait_for(lambda: page.locator("h1", has_text="tabs!!").count() == 1, 5)
    check("Code tab stays open while typing", tab.locator("pre.ml-code").is_visible())

    # printing: molecules as pictures, the plot (not the code) printed
    page.evaluate("document.getElementById('ml-print-frame')?.remove()")
    page.locator("#ml-btn-print").click()
    ok = wait_for(lambda: page.locator("#ml-print-frame[data-ready='document']").count() == 1, 90)
    fr = page.frame_locator("#ml-print-frame")
    check("print: molecules as pictures", ok and fr.locator(".ml-molstar img").count() >= 3 and fr.locator(".ml-molstar canvas").count() == 0, fr.locator(".ml-molstar img").count() if ok else 0)
    check("print: plot printed, code and tabs not", ok and fr.locator("img.ml-figure").is_visible() and fr.locator(".ml-chunk-tabs").count() == 0 and fr.locator(".ml-tabbed pre.ml-code").is_hidden())
    check("Code tab still open after printing", tab.locator("pre.ml-code").is_visible())
