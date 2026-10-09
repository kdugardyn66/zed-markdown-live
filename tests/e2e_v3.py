"""v3 checks: pictures (incl. HEIC/TIFF conversion) and videos (remux/transcode)."""


def run(page, env, lsp, inst, check, wait_for, api, uri):
    doc = env.work / "v3.md"
    lsp.send("textDocument/didOpen", {"textDocument": {"uri": uri(doc), "languageId": "markdown", "version": 1, "text": doc.read_text()}})
    api(inst, "/api/open", {"path": str(doc)})
    ok = wait_for(lambda: page.locator("h1", has_text="Pictures and videos").count() == 1, 10)
    check("v3 document shown", ok)

    def loaded(alt):
        return page.evaluate("a => { const i = document.querySelector('img[alt=\"' + a + '\"]'); return !!i && i.complete && i.naturalWidth > 0; }", alt)

    for alt in ["png", "jpeg", "bmp", "svg"]:
        check(f"{alt} picture shown", wait_for(lambda: loaded(alt), 10))
    check("HEIC converted and shown", wait_for(lambda: loaded("heic"), 20), page.evaluate("document.querySelector('img[alt=heic]')?.src"))
    check("TIFF converted and shown", wait_for(lambda: loaded("tiff"), 20))
    check("{width=120} applied", page.evaluate("document.querySelector('img[alt=jpeg]').getAttribute('width')") == "120")
    check("title → figure caption", page.locator("figure.ml-figure figcaption", has_text="Figure 1").count() == 1)
    check("@import of a HEIC picture", wait_for(lambda: page.evaluate("[...document.querySelectorAll('.ml-media img')].some(i => i.complete && i.naturalWidth > 0)"), 20))
    h264 = page.evaluate("document.createElement('video').canPlayType('video/mp4; codecs=\"avc1.42E01E, mp4a.40.2\"') !== ''")
    print(f"   (this Chromium plays H.264: {h264})")

    def playable(sel):
        return page.evaluate("s => { const v = document.querySelector(s); return !!v && v.readyState >= 1 && v.duration > 1; }", sel)

    n = page.locator("video.ml-video").count()
    ok = wait_for(lambda: page.locator(".ml-media-pending").count() == 0 and page.locator("video.ml-video").count() == 6, 90)
    check("all videos ready (conversions finished)", ok, f"{n} videos, pending {page.locator('.ml-media-pending').count()}")
    for i, name in enumerate(["mp4", "webm", "mkv", "mov", "avi"]):
        sel = f"video.ml-video >> nth={i}"
        ok = wait_for(lambda: page.evaluate("i => { const v = document.querySelectorAll('video.ml-video')[i]; return !!v && v.readyState >= 1 && v.duration > 1; }", i), 30)
        check(f"{name} video plays (metadata loaded)", ok, page.evaluate("i => document.querySelectorAll('video.ml-video')[i]?.src", i))
    check("autoplay video is muted", page.evaluate("[...document.querySelectorAll('video[autoplay]')].every(v => v.muted)"))
    check("audio element", page.locator("audio.ml-audio").count() == 1)
    check("YouTube embed", page.locator("iframe.ml-embed[src*='youtube-nocookie.com/embed/aqz-KE-bpKQ']").count() == 1)
    # range requests (seeking)
    st = page.evaluate("fetch(document.querySelectorAll('video.ml-video')[1].src, {headers: {Range: 'bytes=0-99'}}).then(r => r.status + ' ' + r.headers.get('content-range'))")
    check("HTTP range requests for seeking", st.startswith("206 bytes 0-99/"), st)
    # lightbox
    page.locator("img[alt=png]").click()
    check("lightbox opens", page.locator("#ml-lightbox:not([hidden]) img").count() == 1)
    page.keyboard.press("Escape")
    check("lightbox closes", page.locator("#ml-lightbox[hidden]").count() == 1)
