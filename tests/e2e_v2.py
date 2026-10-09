"""v2 checks: extra diagrams, charts, MathJax/mhchem, code chunks."""


def run(page, env, lsp, inst, check, wait_for, api, uri):
    doc = env.work / "v2.md"
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": doc.read_text()}})
    api(inst, "/api/open", {"path": str(doc)})
    ok = wait_for(lambda: page.locator("h1", has_text="Diagrams and more").count() == 1, 10)
    check("v2 document shown", ok)
    ok = wait_for(lambda: page.locator(".ml-plantuml img.ml-svg").count() == 1, 30)
    check("PlantUML rendered on the server (local Java)", ok, page.locator(".ml-plantuml").first.inner_html()[:200] if page.locator(".ml-plantuml").count() else "none")
    ok = wait_for(lambda: page.locator(".ml-viz svg").count() == 2, 20)
    check("Graphviz dot + neato (Viz.js)", ok)
    ok = wait_for(lambda: page.locator(".ml-vega-lite svg .mark-rect path").count() >= 1 or page.locator(".ml-vega-lite svg").count() >= 1, 20)
    check("Vega-Lite with relaxed JSON + data.csv", ok and page.locator(".ml-vega-lite .ml-error").count() == 0, page.locator(".ml-vega-lite").inner_text()[:200])
    ok = wait_for(lambda: page.locator(".ml-wavedrom svg").count() >= 1, 20)
    check("WaveDrom", ok, page.locator(".ml-wavedrom").inner_text()[:200])
    ok = wait_for(lambda: page.locator(".ml-flow svg").count() == 1, 20)
    check("flowchart.js", ok, page.locator(".ml-flow").inner_text()[:200])
    ok = wait_for(lambda: page.locator(".ml-chart canvas").count() == 2, 20)
    check("Chart.js (block + CSV import)", ok, page.locator(".ml-chart").first.inner_text()[:200])
    ok = wait_for(lambda: page.locator(".ml-plotly .main-svg").count() >= 1, 30)
    check("Plotly block", ok)
    ok = wait_for(lambda: page.locator(".ml-smiles svg").count() == 3 and page.locator(".ml-smiles svg *").count() > 20, 20)
    check("SMILES structures", ok, page.locator(".ml-smiles").inner_text()[:200])
    ok = wait_for(lambda: page.locator(".katex .mord").count() > 5 and "\\ce" not in page.locator("h2:has-text('Chemistry') + p").inner_text(), 10)
    check("mhchem with KaTeX", ok, page.locator("h2:has-text('Chemistry') + p").inner_text())

    # code chunks are off by default
    check("code chunks not run by default", page.locator(".ml-chunk-note").count() == 4)
    check("banner offers to run them", page.locator("#ml-banner button").count() == 1)
    page.locator("#ml-banner button").click()
    ok = wait_for(lambda: page.locator(".ml-chunk-out pre.ml-out", has_text="sum = 45").count() == 1, 30)
    check("python chunk ran (stdout)", ok)
    ok = wait_for(lambda: page.locator(".ml-chunk img.ml-figure").count() == 1, 30)
    check("matplotlib figure captured", ok)
    ok = wait_for(lambda: page.locator(".ml-chunk-out table").count() == 1, 30)
    check("output=markdown + hide=true", ok and page.locator(".ml-chunk", has_text="print(\"| a").count() == 0)
    ok = wait_for(lambda: page.locator(".ml-chunk .ml-plotly .main-svg").count() >= 1, 30)
    check("plotly figure from python", ok)
    ok = wait_for(lambda: page.locator(".ml-chunk .ml-error", has_text="expected failure").count() == 1, 30)
    check("python error shown", ok)
    check("banner gone", wait_for(lambda: page.locator("#ml-banner").is_hidden(), 5))

    # MathJax switch
    page.locator("#ml-btn-math").click()
    ok = wait_for(lambda: page.locator("mjx-container svg").count() >= 3, 30)
    check("MathJax renders (incl. \\ce)", ok)
    page.locator("#ml-btn-math").click()
    wait_for(lambda: page.locator("mjx-container").count() == 0, 10)

    # editing a PlantUML block shows the old picture dimmed, then the new one
    text = doc.read_text().replace("Bob --> Alice : hi", "Bob --> Alice : hi there")
    lsp.send("textDocument/didChange", {"textDocument": {"uri": uri(doc), "version": 2}, "contentChanges": [{"text": text}]})
    stale = wait_for(lambda: page.locator(".ml-plantuml.ml-stale img").count() == 1, 5)
    check("stale PlantUML shown while re-rendering", stale)
    ok = wait_for(lambda: page.locator(".ml-plantuml.ml-stale").count() == 0 and page.locator(".ml-plantuml img").count() == 1, 30)
    check("PlantUML updated", ok)
