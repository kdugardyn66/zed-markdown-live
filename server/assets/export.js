/* MarkDown Live v4 — slides (reveal.js) and exports.
 *
 * Exports start from what the page shows: the browser builds a self-contained
 * snapshot (pictures inlined, diagrams/charts as SVG or PNG, math as MathML for
 * Word/LibreOffice, videos left out) and the server turns it into html, pdf,
 * docx, odt, pptx, odp or a reveal.js deck. */
(function (ML) {
  'use strict';
  const $ = (id) => document.getElementById('ml-' + id);
  const content = $('content');
  const { escapeHtml, toast, post, loadScript, loadCss } = ML;

  // ------------------------------------------------------------------ small helpers
  const blobToDataUrl = (b) => new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(r.result); r.onerror = rej; r.readAsDataURL(b); });
  async function urlToDataUrl(url) {
    try { const r = await fetch(url); if (!r.ok) return null; return await blobToDataUrl(await r.blob()); } catch (e) { return null; }
  }
  const textOf = async (url) => { try { const r = await fetch(url); return r.ok ? await r.text() : ''; } catch (e) { return ''; } };
  const b64utf8 = (s) => btoa(unescape(encodeURIComponent(s)));
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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

  /** SVG element or SVG text → PNG data URL (null if the browser refuses, e.g. foreignObject). */
  async function svgToPng(svg, scale, sizeFrom) {
    let text = typeof svg === 'string' ? svg : new XMLSerializer().serializeToString(svg);
    if (!/xmlns=/.test(text.slice(0, 300))) text = text.replace('<svg', '<svg xmlns="http://www.w3.org/2000/svg"');
    const img = new Image();
    img.src = 'data:image/svg+xml;base64,' + b64utf8(text);
    try { await img.decode(); } catch (e) { return null; }
    const box = sizeFrom ? sizeFrom.getBoundingClientRect() : null;
    const w = Math.max(1, Math.round((box && box.width) || img.naturalWidth || 600));
    const h = Math.max(1, Math.round((box && box.height) || img.naturalHeight || 400));
    const c = document.createElement('canvas');
    c.width = w * scale; c.height = h * scale;
    const g = c.getContext('2d');
    g.fillStyle = '#ffffff'; g.fillRect(0, 0, c.width, c.height);
    g.drawImage(img, 0, 0, c.width, c.height);
    try { return { url: c.toDataURL('image/png'), w, h }; } catch (e) { return null; }
  }
  function pngImg(doc, png, alt) {
    const i = doc.createElement('img');
    i.src = png.url; i.alt = alt || ''; i.width = png.w; i.height = png.h;
    return i;
  }

  // ------------------------------------------------------------------ the snapshot
  /** A clone of the rendered document, with live things turned into static ones. */
  async function cloneRendered(mode) {
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
    // videos, audio, embedded players: documents can't play them
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
    clone.querySelectorAll('[data-done],[data-src],[data-code],[data-opts],[data-h]').forEach((e) => {
      for (const a of ['data-done', 'data-src', 'data-code', 'data-opts', 'data-h', 'data-sourcepos']) e.removeAttribute(a);
    });
    clone.querySelectorAll('[data-sourcepos]').forEach((e) => e.removeAttribute('data-sourcepos'));
    // pictures from the server → data URLs (the file must stand on its own)
    await Promise.all([...clone.querySelectorAll('img')].map(async (img) => {
      const src = img.getAttribute('src') || '';
      if (src.startsWith('/fs/') || src.startsWith('/media/') || src.startsWith('/vendor/')) {
        const d = await urlToDataUrl(src);
        if (d) img.setAttribute('src', d);
      }
    }));
    // links to local files: plain text links (they wouldn't work elsewhere)
    clone.querySelectorAll('a[href^="/fs/"]').forEach((a) => a.setAttribute('href', decodeURIComponent(a.getAttribute('href').slice(3)).split('/').pop()));
    return clone;
  }

  /** CSS for a stand-alone page: document styles, KaTeX (fonts inlined), code colours. */
  async function inlineCss(withKatex) {
    let css = await textOf('/assets/markdown.css');
    css += '\n' + await textOf('/vendor/hljs/styles/github.min.css');
    if (withKatex) {
      let k = await textOf('/vendor/katex/katex.min.css');
      k = k.replace(/,\s*url\(fonts\/[^)]+\.(woff|ttf)\)\s*format\("(woff|truetype)"\)/g, '');
      const fonts = [...new Set([...k.matchAll(/url\((fonts\/[^)]+\.woff2)\)/g)].map((m) => m[1]))];
      for (const f of fonts) {
        const d = await urlToDataUrl('/vendor/katex/' + f);
        if (d) k = k.split('url(' + f + ')').join('url(' + d + ')');
      }
      css += '\n' + k;
    }
    return css;
  }

  function page(title, css, body) {
    return '<!doctype html>\n<html lang="en" data-theme="light"><head><meta charset="utf-8">'
      + '<meta name="viewport" content="width=device-width, initial-scale=1">'
      + '<meta name="generator" content="MarkDown Live ' + escapeHtml(window.ML_VERSION || '') + '">'
      + '<title>' + escapeHtml(title) + '</title><style>' + css
      + '\nbody{margin:0;background:#fff}.markdown-body{padding:40px 48px}'
      + '@media print{.markdown-body{padding:0}@page{margin:18mm 16mm}}'
      + '.ml-video-note{color:#59636e;font-style:italic}aside.notes{display:none}</style></head><body>' + body + '</body></html>';
  }

  /** html / pdf: the rendered page as one self-contained file. */
  async function webSnapshot(title) {
    const clone = await cloneRendered('web');
    const css = await inlineCss(!!clone.querySelector('.katex'));
    return page(title, css, '<article class="markdown-body">' + clone.innerHTML + '</article>');
  }

  /** docx / odt / pptx: plain HTML pandoc understands (MathML equations, PNG diagrams). */
  async function pandocSnapshot(title, opt) {
    opt = opt || {};
    // Mermaid draws labels with HTML (foreignObject), which can't become a PNG:
    // render again with SVG text labels.
    const mermaidPng = new Map();
    const merms = [...content.querySelectorAll('.ml-diagram[data-kind="mermaid"]')];
    if (merms.length && ML.lib('mermaid')) {
      const m = ML.lib('mermaid');
      m.initialize({ startOnLoad: false, theme: 'default', securityLevel: 'strict', htmlLabels: false, flowchart: { htmlLabels: false }, class: { htmlLabels: false }, state: { htmlLabels: false } });
      let n = 0;
      for (const el of merms) {
        try {
          const { svg } = await m.render('ml-export-' + (++n), el.dataset.src || '');
          const holder = document.createElement('div'); holder.style.cssText = 'position:absolute;left:-99999px;top:0'; holder.innerHTML = svg; document.body.appendChild(holder);
          const png = await svgToPng(holder.querySelector('svg'), 2, holder.querySelector('svg'));
          holder.remove();
          if (png) mermaidPng.set(el, png);
        } catch (e) { /* fall back to the on-screen SVG */ }
      }
    }
    const origDiagrams = [...content.querySelectorAll('.ml-diagram')];
    const clone = await cloneRendered('pandoc');
    const doc = document;
    // diagrams → PNG
    const cloneDiagrams = [...clone.querySelectorAll('.ml-diagram')];
    for (let i = 0; i < cloneDiagrams.length; i++) {
      const c = cloneDiagrams[i], o = origDiagrams[i];
      if (!o) continue;
      let png = mermaidPng.get(o) || null;
      if (!png) {
        const svg = o.querySelector('svg');
        const img = o.querySelector('img');
        if (svg) png = await svgToPng(svg, 2, svg);
        else if (img && /^data:image\/svg/.test(img.getAttribute('src') || '')) png = await svgToPng(atob(img.src.split(',')[1]), 2, img);
      }
      if (png) { const p = doc.createElement('p'); p.appendChild(pngImg(doc, png, '')); c.replaceWith(p); }
    }
    // SVG pictures → PNG (Word handles PNG best)
    for (const img of clone.querySelectorAll('img')) {
      const src = img.getAttribute('src') || '';
      if (/^data:image\/svg\+xml/.test(src)) {
        let text = src.includes(';base64,') ? atob(src.split(',')[1]) : decodeURIComponent(src.split(',')[1]);
        const png = await svgToPng(text, 2, null);
        if (png) img.setAttribute('src', png.url);
      }
    }
    // math → MathML (pandoc turns it into Word/LibreOffice equations; mhchem included).
    // Impress can't show equations from PowerPoint files, so for .odp they become pictures.
    await ML.math.loadKatex();
    const katex = ML.lib('katex');
    if (opt.mathImages) await ML.math.loadMathJax();
    for (const el of clone.querySelectorAll('[data-math-style]')) {
      const tex = el.dataset.tex ?? el.textContent;
      const display = el.dataset.mathStyle === 'display';
      if (opt.mathImages && !display) {
        // pictures inside a line are dropped from slides: use text
        el.replaceWith(doc.createTextNode(texToText(tex)));
        continue;
      }
      if (opt.mathImages) {
        try {
          const node = window.MathJax.tex2svg(tex, { display: false });
          const svg = node.querySelector('svg');
          svg.setAttribute('color', '#000');
          const holder = document.createElement('div'); holder.style.cssText = 'position:absolute;left:-99999px;top:0;font-size:' + (display ? 28 : 20) + 'px'; holder.appendChild(node); document.body.appendChild(holder);
          const png = await svgToPng(svg, 3, svg);
          holder.remove();
          if (png) { const img = pngImg(doc, png, ''); if (display) { const p = doc.createElement('p'); p.appendChild(img); el.replaceWith(p); } else el.replaceWith(img); continue; }
        } catch (e) { /* fall back to MathML */ }
      }
      let mathml = '';
      try { mathml = katex.renderToString(tex, { displayMode: display, output: 'mathml', throwOnError: false, strict: 'ignore' }); } catch (e) { mathml = escapeHtml(tex); }
      const holder = doc.createElement(display ? 'p' : 'span');
      holder.innerHTML = mathml;
      const math = holder.querySelector('math');
      if (math && display) math.setAttribute('display', 'block');
      el.replaceWith(...(math ? [math] : holder.childNodes));
    }
    // code: plain text with the language (pandoc highlights it)
    for (const pre of clone.querySelectorAll('pre.ml-code')) {
      const code = pre.querySelector('code');
      const lang = (/language-([\w+#.-]+)/.exec(code?.className || '') || [])[1];
      const text = (code?.textContent ?? '').replace(/^ $/gm, '');
      const np = doc.createElement('pre'); const nc = doc.createElement('code');
      if (lang) nc.className = 'language-' + lang;
      nc.textContent = text; np.appendChild(nc); pre.replaceWith(np);
    }
    // alerts → quotes with a bold title
    for (const a of clone.querySelectorAll('.markdown-alert')) {
      const bq = doc.createElement('blockquote');
      const t = a.querySelector('.markdown-alert-title');
      if (t) { const p = doc.createElement('p'); const b = doc.createElement('strong'); b.textContent = t.textContent; p.appendChild(b); bq.appendChild(p); t.remove(); }
      bq.append(...a.childNodes); a.replaceWith(bq);
    }
    // heading anchors, task boxes, csv wrappers
    clone.querySelectorAll('a.anchor').forEach((e) => e.remove());
    clone.querySelectorAll('input[type=checkbox]').forEach((e) => e.replaceWith(doc.createTextNode(e.checked ? '☑ ' : '☐ ')));
    clone.querySelectorAll('.ml-csv, .ml-block, .ml-import, .ml-media, .ml-chunk, .ml-chunk-out').forEach((e) => e.replaceWith(...e.childNodes));
    return clone;
  }

  /** Simple TeX → Unicode text (H₂O, x², a/b, α…), for places that can't show equations. */
  function texToText(tex) {
    const SUB = { 0: '₀', 1: '₁', 2: '₂', 3: '₃', 4: '₄', 5: '₅', 6: '₆', 7: '₇', 8: '₈', 9: '₉', '+': '₊', '-': '₋', '=': '₌', '(': '₍', ')': '₎', a: 'ₐ', e: 'ₑ', i: 'ᵢ', n: 'ₙ', x: 'ₓ' };
    const SUP = { 0: '⁰', 1: '¹', 2: '²', 3: '³', 4: '⁴', 5: '⁵', 6: '⁶', 7: '⁷', 8: '⁸', 9: '⁹', '+': '⁺', '-': '⁻', '=': '⁼', '(': '⁽', ')': '⁾', n: 'ⁿ', i: 'ⁱ' };
    const GREEK = { alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ε', varepsilon: 'ε', theta: 'θ', lambda: 'λ', mu: 'μ', nu: 'ν', pi: 'π', rho: 'ρ', sigma: 'σ', tau: 'τ', phi: 'φ', varphi: 'φ', chi: 'χ', psi: 'ψ', omega: 'ω', Delta: 'Δ', Gamma: 'Γ', Theta: 'Θ', Lambda: 'Λ', Pi: 'Π', Sigma: 'Σ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω', infty: '∞', cdot: '·', times: '×', pm: '±', leq: '≤', geq: '≥', neq: '≠', approx: '≈', to: '→', rightarrow: '→', leftarrow: '←', sum: 'Σ', int: '∫', partial: '∂', nabla: '∇', sqrt: '√', degree: '°' };
    const map = (str, t) => [...str].map((c) => t[c] || c).join('');
    let s = tex.trim();
    s = s.replace(/\\ce\{([^{}]*)\}/g, (m, x) => x.replace(/([A-Za-z)\]])(\d+)/g, (m2, a, d) => a + map(d, SUB)).replace(/\^(\d*[+-])|(?<=[A-Za-z\d)])([+-])(?=\s|$)/g, (m2, a, b) => map(a ?? b, SUP)).replace(/<=>/g, '⇌').replace(/->/g, '→'));
    s = s.replace(/\\pu\{([^{}]*)\}/g, '$1');
    s = s.replace(/\\frac\{([^{}]*)\}\{([^{}]*)\}/g, '($1)/($2)').replace(/\((\w)\)/g, '$1');
    s = s.replace(/\\(?:mathrm|text|mathbf|mathit|operatorname)\{([^{}]*)\}/g, '$1');
    s = s.replace(/\^\{([^{}]*)\}|\^(\w)/g, (m, a, b) => map(a ?? b, SUP));
    s = s.replace(/_\{([^{}]*)\}|_(\w)/g, (m, a, b) => map(a ?? b, SUB));
    s = s.replace(/\\([A-Za-z]+)/g, (m, w) => GREEK[w] ?? w);
    return s.replace(/[{}]/g, '').replace(/\\[,;! ]/g, ' ').replace(/\s+/g, ' ');
  }

  function pandocPage(title, clone) {
    return '<!doctype html><html><head><meta charset="utf-8"><title>' + escapeHtml(title) + '</title></head><body>' + clone.innerHTML + '</body></html>';
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

  /** slides-html / slides-pdf: a stand-alone reveal.js deck. */
  async function deckSnapshot(title, print) {
    const clone = await cloneRendered('web');
    const theme = deckTheme();
    let css = print ? '' : await textOf('/vendor/reveal/reveal.css');
    css += '\n' + await textOf('/vendor/reveal/theme/' + theme + '.css');
    css += '\n' + await textOf('/assets/deck.css');
    css += '\n' + await textOf('/vendor/hljs/styles/' + (DARK_THEMES.includes(theme) ? 'github-dark' : 'github') + '.min.css');
    if (clone.querySelector('.katex')) {
      let k = await textOf('/vendor/katex/katex.min.css');
      k = k.replace(/,\s*url\(fonts\/[^)]+\.(woff|ttf)\)\s*format\("(woff|truetype)"\)/g, '');
      for (const f of [...new Set([...k.matchAll(/url\((fonts\/[^)]+\.woff2)\)/g)].map((m) => m[1]))]) {
        const d = await urlToDataUrl('/vendor/katex/' + f); if (d) k = k.split('url(' + f + ')').join('url(' + d + ')');
      }
      css += '\n' + k;
    }
    // theme fonts (league gothic, source sans) → data URLs
    css = css.replace(/@import url\(\.\/fonts\/([^)]+)\);?/g, '');
    const opts = deckOptions();
    if (print) return printDeck(title, css, splitSlides(clone), opts);
    const js = await textOf('/vendor/reveal/reveal.js');
    const slides = slidesHtml(splitSlides(clone));
    return '<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">'
      + '<title>' + escapeHtml(title) + '</title><style>' + css + '</style></head><body>'
      + '<div class="reveal"><div class="slides">' + slides + '</div></div>'
      + '<script>' + js.replace(/<\/script/gi, '<\\/script') + '</script>'
      + '<script>Reveal.initialize(' + JSON.stringify(opts) + ');</script></body></html>';
  }

  /** slides-pdf: static pages, one per slide (no reveal.js at print time, so Chrome's
   *  --print-to-pdf can't catch it half set up). The reveal theme still styles them. */
  function printDeck(title, css, slides, opts) {
    const W = Number(opts.width) || 960, H = Number(opts.height) || 700;
    css = css.replace(/\.reveal \.slides section,\s*\.reveal \.slides section>section\s*\{[^}]*\}/g, '');
    const pages = slides.map((s, i) => {
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
      + '.reveal .ml-num{position:absolute;right:18px;bottom:12px;font-size:14px;opacity:.6}';
    // shrink slides whose content is taller than the page (runs before the load event)
    const fit = 'for(const s of document.querySelectorAll("section.ml-page")){const i=s.querySelector(".ml-inner");'
      + 'const avail=s.clientHeight-80;const h=i.scrollHeight;if(h>avail){const k=avail/h;i.style.transform="scale("+k+")";i.style.width=(100/k)+"%";}}';
    return '<!doctype html>\n<html lang="en" class="reveal-viewport"><head><meta charset="utf-8"><title>' + escapeHtml(title) + '</title><style>'
      + css + pageCss + '</style></head><body class="reveal-viewport"><div class="reveal"><div class="slides">' + pages
      + '</div></div><script>' + fit + '</script></body></html>';
  }

  /** pptx / odp: one <h2> per slide (its title), so pandoc makes exactly one slide each. */
  async function pandocSlidesSnapshot(title, format) {
    const clone = await pandocSnapshot(title, { mathImages: format === 'odp' });
    const slides = splitSlides(clone);
    const out = [];
    for (const s of slides) {
      const box = document.createElement('div');
      s.nodes.forEach((n) => box.appendChild(n.cloneNode(true)));
      const h = box.querySelector(':scope > h1, :scope > h2, :scope > h3, :scope > h4');
      let t = '';
      if (h) { t = h.textContent.trim(); h.remove(); }
      box.querySelectorAll('h1, h2, h3, h4, h5, h6').forEach((x) => { const p = document.createElement('p'); const b = document.createElement('strong'); b.textContent = x.textContent; p.appendChild(b); x.replaceWith(p); });
      const notes = [...box.querySelectorAll('aside.notes')].map((x) => { x.remove(); return '<div class="notes">' + x.innerHTML + '</div>'; }).join('');
      // pandoc puts a picture that shares a slide with other content on a slide of its own:
      // use two columns instead (text left, pictures right) so each slide stays one slide.
      const pics = [...box.children].filter((e) => e.tagName === 'P' && e.querySelector('img') && !e.textContent.trim());
      const rest = [...box.children].filter((e) => !pics.includes(e));
      let body = box.innerHTML;
      if (pics.length && rest.length) {
        body = '<div class="columns"><div class="column">' + rest.map((e) => e.outerHTML).join('') + '</div><div class="column">' + pics.map((e) => e.outerHTML).join('') + '</div></div>';
      }
      out.push((t ? '<h2>' + escapeHtml(t) + '</h2>' : '<hr>') + body + notes);
    }
    // no <title>: pandoc would add a title slide
    return '<!doctype html><html><head><meta charset="utf-8"></head><body>' + out.join('\n') + '</body></html>';
  }

  // ------------------------------------------------------------------ export
  const LABELS = { html: 'HTML', pdf: 'PDF', docx: 'Word', odt: 'OpenDocument', 'slides-html': 'slides (HTML)', 'slides-pdf': 'slides (PDF)', pptx: 'PowerPoint', odp: 'Impress' };
  let exporting = false;
  async function buildSnapshot(format) {
    const title = (ML.lastState && (ML.lastState.title || ML.lastState.fileName)) || 'document';
    return ML.withTheme('light', async () => {
      await settled(90000);
      if (format === 'html' || format === 'pdf') return webSnapshot(title);
      if (format === 'docx' || format === 'odt') {
        const c = await pandocSnapshot(title);
        c.querySelectorAll('aside.notes').forEach((e) => e.remove()); // speaker notes are for slides
        return pandocPage(title, c);
      }
      if (format === 'pptx' || format === 'odp') return pandocSlidesSnapshot(title, format);
      return deckSnapshot(title, format === 'slides-pdf');
    });
  }
  async function doExport(format, id) {
    if (exporting && !id) { toast('An export is already running…'); return; }
    exporting = true;
    toast('Exporting ' + LABELS[format] + '…', 300000);
    try {
      const html = await buildSnapshot(format);
      const r = await fetch('/api/export', { method: 'POST', body: JSON.stringify({ format, html, id }) });
      const j = await r.json();
      if (!r.ok) throw new Error(j.error || 'export failed');
      const done = toast('Saved ' + j.path.split(/[\\/]/).pop() + ' ', 8000);
      const a = document.createElement('a'); a.textContent = 'Show in folder';
      a.onclick = () => post('/api/reveal', { path: j.path });
      done.appendChild(a);
    } catch (e) {
      if (id) post('/api/export', { id, format, error: String(e.message || e) });
      toast('Export failed: ' + (e.message || e), 10000);
    } finally { exporting = false; }
  }
  ML.exportTo = doExport;
  ML.buildSnapshot = buildSnapshot;

  // ------------------------------------------------------------------ toolbar: export menu + slides
  const extra = $('extra-buttons');
  const slidesBtn = document.createElement('button');
  slidesBtn.className = 'icon'; slidesBtn.id = 'ml-btn-slides'; slidesBtn.hidden = true;
  slidesBtn.title = 'Slides view (P)';
  slidesBtn.innerHTML = '<svg class="i" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="12" rx="1"/><path d="M12 16v4M8 20h8"/></svg>';
  const exportBtn = document.createElement('button');
  exportBtn.className = 'icon'; exportBtn.id = 'ml-btn-export'; exportBtn.title = 'Export (E)';
  exportBtn.innerHTML = '<svg class="i" viewBox="0 0 24 24"><path d="M12 3v12M7 10l5 5 5-5"/><path d="M5 21h14"/></svg>';
  extra.prepend(slidesBtn, exportBtn);
  const menu = document.createElement('div');
  menu.className = 'menu'; menu.id = 'ml-export-menu';
  const item = (f, big, small) => '<button data-format="' + f + '"><b>' + big + '</b><small>' + small + '</small></button>';
  menu.innerHTML = '<h4>Document</h4><div class="grid">' + item('pdf', 'PDF', 'print') + item('docx', 'Word', '.docx') + item('odt', 'ODT', 'LibreOffice')
    + item('html', 'HTML', 'one file') + '</div><h4>Slides</h4><div class="grid">' + item('pptx', 'PowerPoint', '.pptx') + item('odp', 'Impress', '.odp')
    + item('slides-pdf', 'PDF', 'one per page') + item('slides-html', 'HTML', 'reveal.js') + '</div>'
    + '<p class="hint">Saved next to the Markdown file. Videos are left out. Slides split at <code>&lt;!-- slide --&gt;</code> (or at headings).</p>';
  document.body.appendChild(menu);
  exportBtn.onclick = (e) => { e.stopPropagation(); menu.classList.toggle('open'); };
  document.addEventListener('click', (e) => { if (!menu.contains(e.target)) menu.classList.remove('open'); });
  menu.addEventListener('click', (e) => {
    const b = e.target.closest('button[data-format]');
    if (!b) return;
    menu.classList.remove('open');
    doExport(b.dataset.format);
  });

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
    const req = s.exportRequest;
    if (req && !handled.has(req.id)) { handled.add(req.id); post('/api/export-ack', { id: req.id }); doExport(req.format, req.id); }
  };
  const prevKey = ML.onKey;
  ML.onKey = (e) => {
    if (e.key === 'p' || e.key === 'P') { if (!slidesBtn.hidden || ML.deckActive) showDeck(!ML.deckActive); }
    else if (e.key === 'e' || e.key === 'E') exportBtn.click();
    else if (prevKey) prevKey(e);
  };
})(window.ML);
