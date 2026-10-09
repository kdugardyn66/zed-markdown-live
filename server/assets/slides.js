/* MarkDown Live — slides (reveal.js) and printing.
 *
 * Printing starts from what the page shows: a static copy of the rendered document
 * (charts as pictures, videos left out) is laid out in a hidden frame, always in
 * the light theme, and handed to the browser's print dialog (where "Save as PDF"
 * is one of the printers). In slide view it prints one slide per page. */
(function (ML) {
  'use strict';
  const $ = (id) => document.getElementById('ml-' + id);
  const content = $('content');
  const { escapeHtml, toast, post, loadScript, loadCss } = ML;

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

  // ------------------------------------------------------------------ slides
  /** Splits a rendered document into slides: at <!-- slide --> markers, else at h1/h2. */
  function splitSlides(root) {
    const kids = [...root.childNodes];
    const markers = root.querySelectorAll(':scope > .ml-slide-break').length;
    const slides = [];
    let cur = { attrs: {}, nodes: [], line: 1 };
    const push = () => { if (cur.nodes.some((n) => n.nodeType === 1 || (n.textContent || '').trim())) slides.push(cur); };
    for (const n of kids) {
      const isBreak = n.nodeType === 1 && n.classList.contains('ml-slide-break');
      const isHeading = !markers && n.nodeType === 1 && /^H[12]$/.test(n.tagName) && cur.nodes.some((x) => x.nodeType === 1);
      if (isBreak || isHeading) {
        push();
        let attrs = {};
        if (isBreak) { try { attrs = JSON.parse(n.dataset.slide || '{}'); } catch (e) { /* none */ } }
        const sp = n.getAttribute && n.getAttribute('data-sourcepos');
        cur = { attrs, nodes: isBreak ? [] : [n], line: sp ? +sp.split(':')[0] : 1 };
      } else {
        cur.nodes.push(n);
      }
    }
    push();
    return slides;
  }

  function sectionHtml(slide) {
    const a = slide.attrs || {};
    let attrs = '';
    for (const [k, v] of Object.entries(a)) {
      if (k === 'vertical') continue;
      if (/^(data-[\w-]+|class|id|style)$/.test(k)) attrs += ' ' + k + '="' + escapeHtml(v) + '"';
    }
    const div = document.createElement('div');
    slide.nodes.forEach((n) => div.appendChild(n.cloneNode(true)));
    // speaker notes: <aside class="notes"> stays (reveal.js uses it)
    return '<section' + attrs + ' data-line="' + slide.line + '">' + div.innerHTML + '</section>';
  }

  function slidesHtml(slides) {
    // vertical=true slides stack under the previous one
    const groups = [];
    for (const s of slides) {
      if (s.attrs && /^(true|1)$/.test(String(s.attrs.vertical)) && groups.length) groups[groups.length - 1].push(s);
      else groups.push([s]);
    }
    return groups.map((g) => (g.length === 1 ? sectionHtml(g[0]) : '<section>' + g.map(sectionHtml).join('') + '</section>')).join('\n');
  }

  const DARK_THEMES = ['black', 'black-contrast', 'night', 'league', 'moon', 'dracula', 'blood'];
  function deckTheme() {
    const fm = (ML.lastState && ML.lastState.deck) || {};
    let t = fm.theme || ML.state.settings.slideTheme || 'auto';
    if (t === 'auto') t = ML.theme() === 'dark' ? 'black' : 'white';
    return String(t).replace(/\.css$/, '');
  }
  function deckOptions(extra) {
    const fm = Object.assign({}, (ML.lastState && ML.lastState.deck) || {});
    delete fm.theme;
    return Object.assign({ hash: false, controls: true, progress: true, center: true, transition: 'slide', width: 960, height: 700, margin: 0.06, slideNumber: false }, fm, extra || {});
  }

  /** The slides as pages, one per slide, styled by the reveal.js theme (no reveal.js
   *  at print time: static pages can't be caught half set up). */
  async function slidesPage(title) {
    const clone = await cloneRendered();
    const theme = deckTheme();
    const opts = deckOptions();
    const W = Number(opts.width) || 960, H = Number(opts.height) || 700;
    const pages = splitSlides(clone).map((s, i) => {
      const a = s.attrs || {};
      const bg = a['data-background-color'] ? ' style="background:' + escapeHtml(a['data-background-color']) + '"' : '';
      const cls = a.class ? ' ' + escapeHtml(a.class) : '';
      const div = document.createElement('div');
      s.nodes.forEach((n) => div.appendChild(n.cloneNode(true)));
      div.querySelectorAll('aside.notes').forEach((n) => n.remove());
      const num = opts.slideNumber ? '<div class="ml-num">' + (i + 1) + '</div>' : '';
      return '<section class="ml-page' + cls + '"' + bg + '><div class="ml-inner">' + div.innerHTML + '</div>' + num + '</section>';
    }).join('\n');
    const pageCss = '@page{size:' + W + 'px ' + H + 'px;margin:0}html,body{margin:0;padding:0;background:var(--r-background-color,#fff)}'
      + '.reveal{position:static!important;width:auto!important;height:auto!important;overflow:visible!important}'
      + '.reveal .slides{position:static!important;width:auto!important;height:auto!important;transform:none!important;pointer-events:auto;text-align:left}'
      + '.reveal .slides section.ml-page{display:flex!important;position:relative!important;visibility:visible!important;opacity:1!important;transform:none!important;'
      + 'box-sizing:border-box;width:' + W + 'px;height:' + H + 'px;padding:40px 56px;overflow:hidden;flex-direction:column;justify-content:center;'
      + 'break-after:page;page-break-after:always;background-color:var(--r-background-color,#fff)}'
      + '.reveal .slides section.ml-page .ml-inner{transform-origin:top left;width:100%}'
      + '.reveal .ml-num{position:absolute;right:18px;bottom:12px;font-size:14px;opacity:.6}'
      + '.ml-video-note{font-style:italic;opacity:.7}';
    return '<!doctype html>\n<html lang="en" class="reveal-viewport"><head><meta charset="utf-8"><title>' + escapeHtml(title) + '</title>'
      + link('/vendor/reveal/theme/' + theme + '.css') + link('/assets/deck.css')
      + link('/vendor/hljs/styles/' + (DARK_THEMES.includes(theme) ? 'github-dark' : 'github') + '.min.css')
      + (clone.querySelector('.katex') ? link('/vendor/katex/katex.min.css') : '')
      + '<style>' + pageCss + '</style></head><body class="reveal-viewport"><div class="reveal"><div class="slides">' + pages
      + '</div></div></body></html>';
  }

  /** Shrinks slides whose content is taller than the page. */
  function fitSlides(doc) {
    for (const s of doc.querySelectorAll('section.ml-page')) {
      const inner = s.querySelector('.ml-inner');
      const avail = s.clientHeight - 80, h = inner.scrollHeight;
      if (h > avail) { const k = avail / h; inner.style.transform = 'scale(' + k + ')'; inner.style.width = (100 / k) + '%'; }
    }
  }

  // ------------------------------------------------------------------ print
  let printing = false;
  /** Lays the document (or the slides, in slide view) out in a hidden frame and opens
   *  the browser's print dialog for it ("Save as PDF" is one of its printers). */
  async function doPrint() {
    if (printing) return;
    printing = true;
    toast('Preparing to print…', 120000);
    let frame = null;
    try {
      const title = (ML.lastState && (ML.lastState.title || ML.lastState.fileName)) || 'document';
      const slides = !!ML.deckActive;
      const html = await ML.withTheme('light', async () => { await settled(90000); return slides ? slidesPage(title) : documentPage(title); });
      document.getElementById('ml-print-frame')?.remove();
      frame = document.createElement('iframe');
      frame.id = 'ml-print-frame';
      frame.setAttribute('aria-hidden', 'true');
      frame.style.cssText = 'position:fixed;right:0;bottom:0;width:' + (slides ? 1000 : 800) + 'px;height:600px;border:0;visibility:hidden;pointer-events:none';
      const loaded = new Promise((res) => { frame.onload = res; });
      frame.srcdoc = html;
      document.body.appendChild(frame);
      await loaded;
      const doc = frame.contentDocument;
      await (doc.fonts ? doc.fonts.ready : Promise.resolve());
      await Promise.all([...doc.images].map((i) => (i.complete ? null : new Promise((r) => { i.onload = i.onerror = r; }))));
      if (slides) fitSlides(doc);
      toast('Opening the print dialog…', 1500);
      frame.dataset.ready = slides ? 'slides' : 'document';
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

  // ------------------------------------------------------------------ toolbar: slides + print
  const extra = $('extra-buttons');
  const slidesBtn = document.createElement('button');
  slidesBtn.className = 'icon'; slidesBtn.id = 'ml-btn-slides'; slidesBtn.hidden = true;
  slidesBtn.title = 'Slides view (P)';
  slidesBtn.innerHTML = '<svg class="i" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="12" rx="1"/><path d="M12 16v4M8 20h8"/></svg>';
  extra.prepend(slidesBtn);
  // the toolbar's print button (v1: window.print) now prints the copy, slides included
  const printBtn = $('btn-print');
  printBtn.title = 'Print (' + (/Mac/.test(navigator.platform) ? '⌘P' : 'Ctrl+P') + '); in slide view one slide per page';
  printBtn.onclick = () => doPrint();
  // Cmd-P / Ctrl-P prints the document, not the preview window
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && (e.key === 'p' || e.key === 'P')) { e.preventDefault(); e.stopPropagation(); doPrint(); }
  }, true);

  // ------------------------------------------------------------------ live slides view
  let deck = null, deckEl = null, rebuildTimer = null, lines = [];
  async function showDeck(on) {
    ML.deckActive = on;
    slidesBtn.classList.toggle('on', on);
    try { localStorage.setItem('ml.slides', on ? '1' : '0'); } catch (e) { /* ignore */ }
    if (!on) {
      if (deckEl) deckEl.hidden = true;
      $('scroller').hidden = false;
      return;
    }
    await loadScript('/vendor/reveal/reveal.js', 'Reveal');
    loadCss('/vendor/reveal/reveal.css');
    loadCss('/assets/deck.css');
    if (!deckEl) {
      deckEl = document.createElement('div');
      deckEl.id = 'ml-deck';
      deckEl.innerHTML = '<div class="reveal"><div class="slides"></div></div>';
      $('main').appendChild(deckEl);
    }
    deckEl.hidden = false;
    $('scroller').hidden = true;
    await rebuildDeck();
  }
  function setDeckTheme() {
    const t = deckTheme();
    let l = document.getElementById('ml-deck-theme');
    const href = '/vendor/reveal/theme/' + t + '.css';
    if (!l) { l = document.createElement('link'); l.rel = 'stylesheet'; l.id = 'ml-deck-theme'; document.head.appendChild(l); }
    if (l.getAttribute('href') !== href) l.setAttribute('href', href);
    deckEl.classList.toggle('dark', DARK_THEMES.includes(t));
  }
  async function rebuildDeck() {
    if (!ML.deckActive || !deckEl) return;
    setDeckTheme();
    const clone = content.cloneNode(true);
    // charts are canvases: copy their pixels
    const oc = [...content.querySelectorAll('canvas')], cc = [...clone.querySelectorAll('canvas')];
    cc.forEach((c, i) => { try { const img = document.createElement('img'); img.src = oc[i].toDataURL(); img.style.maxWidth = '100%'; c.replaceWith(img); } catch (e) { /* ignore */ } });
    clone.querySelectorAll('.ml-cursor').forEach((e) => e.classList.remove('ml-cursor'));
    const slides = splitSlides(clone);
    lines = slides.map((s) => s.line);
    const container = deckEl.querySelector('.slides');
    const Reveal = ML.lib('Reveal');
    const idx = deck ? deck.getIndices() : { h: 0, v: 0 };
    container.innerHTML = slidesHtml(slides);
    if (!deck) {
      deck = new Reveal(deckEl.querySelector('.reveal'), deckOptions({ embedded: true, keyboardCondition: 'focused' }));
      await deck.initialize();
      deckEl.querySelector('.reveal').setAttribute('tabindex', '0');
      deckEl.querySelector('.reveal').focus();
    } else {
      deck.configure(deckOptions({ embedded: true }));
      deck.sync();
      deck.layout();
    }
    deck.slide(idx.h || 0, idx.v || 0);
  }
  ML.deckCursor = (line0) => {
    if (!deck || !lines.length) return;
    const L = line0 + 1;
    let i = 0;
    lines.forEach((l, k) => { if (l <= L) i = k; });
    // horizontal index of slide i, accounting for vertical stacks
    const sections = [...deckEl.querySelectorAll('.slides > section')];
    let n = 0;
    for (let h = 0; h < sections.length; h++) {
      const inner = sections[h].querySelectorAll(':scope > section');
      const count = inner.length || 1;
      if (i < n + count) { deck.slide(h, inner.length ? i - n : 0); return; }
      n += count;
    }
  };
  slidesBtn.onclick = () => showDeck(!ML.deckActive);

  // ------------------------------------------------------------------ updates + keys
  const prevUpdate = ML.onUpdate;
  let handled = new Set();
  ML.onUpdate = (s) => {
    prevUpdate && prevUpdate(s);
    ML.lastState = s;
    slidesBtn.hidden = !(s.slides > 0) && !ML.deckActive;
    if (s.slides > 0 && !ML.deckActive && !slidesBtn.dataset.auto) {
      slidesBtn.dataset.auto = '1';
      let pref = null; try { pref = localStorage.getItem('ml.slides'); } catch (e) { /* ignore */ }
      if (pref === '1') showDeck(true);
    }
    if (ML.deckActive) {
      clearTimeout(rebuildTimer);
      rebuildTimer = setTimeout(() => (ML.renderPromise || Promise.resolve()).then(rebuildDeck), 350);
    }
    // a print asked for from Zed (Option-Shift-P) or the CLI
    const req = s.printRequest;
    if (req && !handled.has(req.id)) { handled.add(req.id); post('/api/print-taken', { id: req.id }); doPrint(); }
  };
  const prevKey = ML.onKey;
  ML.onKey = (e) => {
    if (e.key === 'p' || e.key === 'P') { if (!slidesBtn.hidden || ML.deckActive) showDeck(!ML.deckActive); }
    else if (prevKey) prevKey(e);
  };
})(window.ML);
