/* MarkDown Live — printing.
 *
 * Printing starts from what the page shows: a static copy of the rendered document
 * (charts and 3D molecules as pictures, videos left out) is laid out in a hidden
 * frame, always in the light theme, and handed to the browser's print dialog (where
 * "Save as PDF" is one of the printers). <!-- slide --> markers are page breaks. */
(function (ML) {
  'use strict';
  const $ = (id) => document.getElementById('ml-' + id);
  const content = $('content');
  const { escapeHtml, toast, post } = ML;

  // ------------------------------------------------------------------ small helpers
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const link = (href) => '<link rel="stylesheet" href="' + href + '">';

  /** Waits until every block is rendered (browser plugins and server jobs). */
  async function settled(timeoutMs) {
    const end = Date.now() + (timeoutMs || 60000);
    for (;;) {
      let p;
      do { p = ML.renderPromise; await (p || Promise.resolve()); } while (p !== ML.renderPromise);
      const s = ML.lastState || {};
      const busy = (s.pending || 0) > 0 || content.querySelector('.ml-pending, .ml-stale, .ml-media-pending');
      if (!busy) { await sleep(150); return; }
      if (Date.now() > end) return;
      await sleep(250);
    }
  }

  // ------------------------------------------------------------------ the printed copy
  /** A clone of the rendered document, with live things turned into static ones. */
  async function cloneRendered() {
    const clone = content.cloneNode(true);
    // pairs: original element ↔ clone element (same document order)
    const pairs = (sel) => { const a = [...content.querySelectorAll(sel)], b = [...clone.querySelectorAll(sel)]; return a.map((o, i) => [o, b[i]]).filter((p) => p[1]); };

    // canvases (Chart.js) → pictures
    for (const [o, c] of pairs('canvas')) {
      try { const img = document.createElement('img'); img.src = o.toDataURL('image/png'); img.style.maxWidth = '100%'; img.width = o.clientWidth; c.replaceWith(img); } catch (e) { c.remove(); }
    }
    // Plotly → pictures
    for (const [o, c] of pairs('.ml-plotly-box')) {
      try {
        const url = await ML.lib('Plotly').toImage(o, { format: 'png', scale: 2, width: o.clientWidth || 700, height: o.clientHeight || 450 });
        const img = document.createElement('img'); img.src = url; img.width = o.clientWidth || 700; c.replaceWith(img);
      } catch (e) { /* keep */ }
    }
    // 3D molecules (WebGL) → pictures
    for (const [o, c] of pairs('.ml-molstar')) {
      const url = ML.molstarImage ? await ML.molstarImage(o) : null;
      if (url) { const img = document.createElement('img'); img.src = url; img.style.maxWidth = '100%'; img.width = o.clientWidth || 600; c.replaceChildren(img); c.style.height = 'auto'; }
    }
    // code chunks with a Graph | Code switch: print the output
    clone.querySelectorAll('.ml-chunk-tabs').forEach((e) => e.remove());
    clone.querySelectorAll('.ml-show-code').forEach((e) => e.classList.remove('ml-show-code'));
    // videos, audio, embedded players: paper can't play them
    for (const v of clone.querySelectorAll('video, audio, iframe.ml-embed, .ml-media-pending')) {
      const p = document.createElement('p'); p.className = 'ml-video-note';
      const src = v.getAttribute('src') || '';
      const name = decodeURIComponent(src.split('#')[0].split('/').pop() || '');
      p.textContent = (v.tagName === 'AUDIO' ? '♪ audio: ' : '▶ video: ') + (v.getAttribute('title') || v.textContent.trim() || name);
      v.replaceWith(p);
    }
    // UI leftovers
    clone.querySelectorAll('.ml-copy, .ml-chunk-note, .ml-src, script').forEach((e) => e.remove());
    clone.querySelectorAll('.ml-cursor').forEach((e) => e.classList.remove('ml-cursor'));
    return clone;
  }

  /** The document as a page: same styles as the preview, light theme. */
  async function documentPage(title) {
    const clone = await cloneRendered();
    return '<!doctype html>\n<html lang="en" data-theme="light"><head><meta charset="utf-8"><title>' + escapeHtml(title) + '</title>'
      + link('/assets/markdown.css') + link('/vendor/hljs/styles/github.min.css')
      + (clone.querySelector('.katex') ? link('/vendor/katex/katex.min.css') : '')
      + '<style>body{margin:0;background:#fff}.markdown-body{max-width:none;padding:0}@page{margin:18mm 16mm}'
      + '.ml-video-note{color:#59636e;font-style:italic}aside.notes,a.anchor{display:none}</style></head><body>'
      + '<article class="markdown-body">' + clone.innerHTML + '</article></body></html>';
  }

  // ------------------------------------------------------------------ print
  let printing = false;
  /** Lays the document out in a hidden frame and opens the browser's print dialog
   *  for it ("Save as PDF" is one of its printers). */
  async function doPrint() {
    if (printing) return;
    printing = true;
    toast('Preparing to print…', 120000);
    let frame = null;
    try {
      const title = (ML.lastState && (ML.lastState.title || ML.lastState.fileName)) || 'document';
      const html = await ML.withTheme('light', async () => { await settled(90000); return documentPage(title); });
      document.getElementById('ml-print-frame')?.remove();
      frame = document.createElement('iframe');
      frame.id = 'ml-print-frame';
      frame.setAttribute('aria-hidden', 'true');
      frame.style.cssText = 'position:fixed;right:0;bottom:0;width:800px;height:600px;border:0;visibility:hidden;pointer-events:none';
      const loaded = new Promise((res) => { frame.onload = res; });
      frame.srcdoc = html;
      document.body.appendChild(frame);
      await loaded;
      const doc = frame.contentDocument;
      await (doc.fonts ? doc.fonts.ready : Promise.resolve());
      await Promise.all([...doc.images].map((i) => (i.complete ? null : new Promise((r) => { i.onload = i.onerror = r; }))));
      toast('Opening the print dialog…', 1500);
      frame.dataset.ready = 'document';
      // the title is the suggested file name for "Save as PDF"
      const prevTitle = document.title;
      document.title = title;
      try { frame.contentWindow.focus(); frame.contentWindow.print(); } finally { document.title = prevTitle; window.focus(); }
    } catch (e) {
      toast('Printing failed: ' + (e.message || e), 10000);
    } finally {
      printing = false;
      // keep the frame a while: some browsers print it after print() returns
      const f = frame;
      if (f) setTimeout(() => { if (f.isConnected && document.getElementById('ml-print-frame') === f) f.remove(); }, 60000);
    }
  }
  ML.print = doPrint;

  // ------------------------------------------------------------------ toolbar + requests
  // the toolbar's print button (v1: window.print) prints the copy
  const printBtn = $('btn-print');
  printBtn.title = 'Print (' + (/Mac/.test(navigator.platform) ? '⌘P' : 'Ctrl+P') + ')';
  printBtn.onclick = () => doPrint();
  // Cmd-P / Ctrl-P prints the document, not the preview window
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && (e.key === 'p' || e.key === 'P')) { e.preventDefault(); e.stopPropagation(); doPrint(); }
  }, true);

  const prevUpdate = ML.onUpdate;
  const handled = new Set();
  ML.onUpdate = (s) => {
    prevUpdate && prevUpdate(s);
    ML.lastState = s;
    // a print asked for from Zed (Option-Shift-P) or the CLI
    const req = s.printRequest;
    if (req && !handled.has(req.id)) { handled.add(req.id); post('/api/print-taken', { id: req.id }); doPrint(); }
  };
})(window.ML);
