/* MarkDown Live — browser side.
 *
 * The server sends rendered HTML (long polling /api/state). The page patches the
 * DOM with morphdom, then runs the "plugins" (Mermaid, KaTeX, highlight.js, …) on
 * blocks that are new or changed. Blocks that didn't change keep their rendered
 * output, so typing never makes diagrams or math flicker.
 */
(function () {
  'use strict';
  const $ = (id) => document.getElementById('ml-' + id);
  const content = $('content');
  const scroller = $('scroller');

  // ------------------------------------------------------------------ prefs
  const store = {
    get(k, d) { try { const v = localStorage.getItem('ml.' + k); return v === null ? d : JSON.parse(v); } catch (e) { return d; } },
    set(k, v) { try { localStorage.setItem('ml.' + k, JSON.stringify(v)); } catch (e) { /* private window */ } },
  };
  const state = {
    version: 0, cseq: -1, settings: {}, file: null, headings: [],
    themePref: store.get('theme', null), // null = follow the server setting
    zoom: store.get('zoom', 1),
    sync: store.get('sync', true),
    toc: store.get('toc', false),
    tab: Math.random().toString(36).slice(2), born: Date.now(),
    retired: false, cursorLine: null,
  };

  // ------------------------------------------------------------------ library loader
  const loaded = new Map();
  function setLibStatus() {
    const busy = [...loaded.entries()].filter(([, v]) => v.busy).map(([k]) => k.split('/')[2]);
    $('libs').textContent = busy.length ? 'loading ' + busy.join(', ') + '…' : '';
  }
  // Headings get ids, and ids become window properties ("## Mermaid" -> window.mermaid),
  // so a library global only counts when it isn't a DOM element.
  function hasGlobal(g) { return !!g && g in window && window[g] && !(window[g] instanceof Element) && !(window[g] instanceof HTMLCollection); }
  /** Loads a script once. Retries when the browser stalls (seen on busy machines). */
  function loadScript(src, globalName) {
    if (loaded.has(src)) return loaded.get(src).p;
    const entry = { busy: true };
    entry.p = new Promise((resolve, reject) => {
      let attempt = 0;
      const tryLoad = () => {
        if (hasGlobal(globalName)) { resolve(); return; }
        attempt++;
        const s = document.createElement('script');
        s.src = src + (attempt > 1 ? (src.includes('?') ? '&' : '?') + 'r=' + attempt : '');
        let done = false;
        const timer = setTimeout(() => {
          if (done) return;
          if (hasGlobal(globalName)) { done = true; resolve(); return; }
          if (attempt < 4) { s.remove(); tryLoad(); } else { done = true; reject(new Error('timeout loading ' + src)); }
        }, 9000 * attempt);
        s.onload = () => { if (!done) { done = true; clearTimeout(timer); resolve(); } };
        s.onerror = () => {
          if (done) return; clearTimeout(timer);
          if (attempt < 4) setTimeout(tryLoad, 500 * attempt); else { done = true; reject(new Error('cannot load ' + src)); }
        };
        document.head.appendChild(s);
      };
      tryLoad();
    }).finally(() => { entry.busy = false; setLibStatus(); });
    loaded.set(src, entry);
    setLibStatus();
    return entry.p;
  }
  function loadCss(href) {
    if (document.querySelector('link[href="' + href + '"]')) return;
    const l = document.createElement('link'); l.rel = 'stylesheet'; l.href = href; document.head.appendChild(l);
  }

  // ------------------------------------------------------------------ helpers
  function escapeHtml(s) { return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c])); }
  function errorBox(el, msg) {
    const d = document.createElement('div'); d.className = 'ml-error'; d.textContent = msg;
    el.querySelectorAll(':scope > .ml-error').forEach((e) => e.remove());
    el.appendChild(d);
  }
  let toastTimer = null;
  function toast(msg, ms) {
    const t = $('toast'); t.textContent = msg; t.style.display = 'block';
    clearTimeout(toastTimer); toastTimer = setTimeout(() => { t.style.display = 'none'; }, ms || 3500);
    return t;
  }
  function theme() {
    const pref = state.themePref || state.settings.theme || 'auto';
    if (pref === 'auto') return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    return pref;
  }
  function post(url, body) { return fetch(url, { method: 'POST', body: JSON.stringify(body || {}) }); }

  // ------------------------------------------------------------------ plugins
  /**
   * A plugin renders the elements matching `selector`.
   *  key(el)        identity of the source; unchanged key = keep the rendered output
   *  load()         loads the library (once)
   *  render(el)     renders one element (may be async)
   *  themed         re-render when the theme changes
   */
  const plugins = [];
  const ML = window.ML = { plugins, loadScript, hasGlobal, lib: (g) => (hasGlobal(g) ? window[g] : null), loadCss, escapeHtml, errorBox, toast, theme, state, post,
    register(p) { plugins.push(p); }, rerender: () => renderPlugins(true) };

  function keyOf(p, el) { return p.name + ':' + (p.themed ? theme() + ':' : '') + p.key(el); }

  ML.register({
    name: 'mermaid', selector: '.ml-diagram[data-kind="mermaid"]', themed: true,
    key: (el) => el.dataset.h,
    async load() {
      await loadScript('/vendor/mermaid/mermaid.min.js', 'mermaid');
    },
    n: 0,
    async render(el) {
      const src = el.querySelector('pre.ml-src')?.textContent ?? el.dataset.src ?? '';
      el.dataset.src = src;
      const mt = state.settings.mermaidTheme && state.settings.mermaidTheme !== 'auto' ? state.settings.mermaidTheme : (theme() === 'dark' ? 'dark' : 'default');
      ML.lib('mermaid').initialize({ startOnLoad: false, theme: mt, securityLevel: 'strict', fontFamily: 'inherit', htmlLabels: true, flowchart: { htmlLabels: true }, class: { htmlLabels: true }, state: { htmlLabels: true } });
      const id = 'ml-mermaid-' + (++this.n);
      try {
        const { svg } = await ML.lib('mermaid').render(id, src);
        el.innerHTML = svg;
      } catch (e) {
        document.getElementById('d' + id)?.remove();
        document.getElementById(id)?.remove();
        el.innerHTML = '<pre class="ml-src">' + escapeHtml(src) + '</pre>';
        errorBox(el, 'Mermaid: ' + (e && e.message ? e.message : e));
      }
    },
  });

  ML.math = {
    engine: () => 'katex',
    async loadKatex() {
      loadCss('/vendor/katex/katex.min.css');
      await loadScript('/vendor/katex/katex.min.js', 'katex');
    },
    async render(el, tex, display) {
      await this.loadKatex();
      try {
        ML.lib('katex').render(tex, el, { displayMode: display, throwOnError: false, strict: 'ignore', trust: false, output: 'htmlAndMathml' });
      } catch (e) {
        el.textContent = tex; el.title = String(e.message || e);
      }
    },
  };
  ML.register({
    name: 'math', selector: '[data-math-style]',
    key: (el) => (ML.math.engine()) + ':' + el.dataset.mathStyle + ':' + (el.dataset.tex ?? el.textContent),
    async load() {},
    async render(el) {
      const tex = el.dataset.tex ?? el.textContent;
      el.dataset.tex = tex;
      await ML.math.render(el, tex, el.dataset.mathStyle === 'display');
    },
  });

  ML.register({
    name: 'code', selector: 'pre.ml-code', themed: false,
    key: (el) => (el.querySelector('code')?.className.replace(/\s*hljs\b/g, '') || '') + '|' + (el.dataset.highlight || '') + '|' + (el.dataset.code ?? el.querySelector('code')?.textContent ?? ''),
    async load() { await loadScript('/vendor/hljs/highlight.min.js', 'hljs'); },
    async render(el) {
      const code = el.querySelector('code');
      if (!code) return;
      const text = el.dataset.code ?? code.textContent;
      el.dataset.code = text;
      const m = /language-([\w+#.-]+)/.exec(code.className);
      const lang = m && m[1];
      code.textContent = text;
      if (lang && ML.lib('hljs').getLanguage(lang)) {
        try { ML.lib('hljs').highlightElement(code); } catch (e) { /* ignore */ }
      }
      const hl = parseRanges(el.dataset.highlight);
      if (el.classList.contains('line-numbers') || hl.size) {
        const lines = code.innerHTML.replace(/\n$/, '').split('\n');
        code.innerHTML = lines.map((l, i) => '<span class="ln' + (hl.has(i + 1) ? ' hl-line' : '') + '">' + (l || ' ') + '</span>').join('\n');
      }
      if (!el.querySelector('.ml-copy')) {
        const b = document.createElement('button'); b.className = 'ml-copy'; b.textContent = 'Copy';
        b.onclick = async (ev) => { ev.stopPropagation(); try { await navigator.clipboard.writeText(el.dataset.code); b.textContent = 'Copied'; } catch (e) { b.textContent = 'Failed'; } setTimeout(() => { b.textContent = 'Copy'; }, 1200); };
        el.appendChild(b);
      }
    },
  });
  function parseRanges(s) {
    const out = new Set();
    if (!s) return out;
    String(s).replace(/[\[\]\s]/g, '').split(',').forEach((part) => {
      const [a, b] = part.split('-').map(Number);
      if (!isNaN(a)) for (let i = a; i <= (isNaN(b) ? a : b); i++) out.add(i);
    });
    return out;
  }

  let renderRun = 0;
  async function renderPlugins(force) {
    const run = ++renderRun;
    const jobs = plugins.map(async (p) => {
      const els = [...content.querySelectorAll(p.selector)].filter((el) => force || el.dataset.done !== keyOf(p, el));
      if (!els.length) return;
      try { await p.load(); } catch (e) { els.forEach((el) => errorBox(el, String(e.message || e))); return; }
      for (const el of els) {
        if (run !== renderRun && !el.isConnected) continue;
        const k = keyOf(p, el);
        try { await p.render(el); } catch (e) { errorBox(el, p.name + ': ' + (e.message || e)); }
        el.dataset.done = k;
      }
    });
    await Promise.all(jobs);
    if (run === renderRun) { buildSyncIndex(); }
  }

  // ------------------------------------------------------------------ DOM update
  function pluginFor(el) { return plugins.find((p) => el.matches && el.matches(p.selector)); }

  function applyHtml(html) {
    const next = document.createElement('article');
    next.innerHTML = html;
    if (!hasGlobal('morphdom')) { content.innerHTML = html; return; }
    window.morphdom(content, next, {
      childrenOnly: true,
      getNodeKey: () => null,
      onBeforeElUpdated(fromEl, toEl) {
        if (fromEl.dataset && fromEl.dataset.done) {
          const p = pluginFor(toEl);
          if (p && fromEl.dataset.done === keyOf(p, toEl)) {
            // unchanged: keep the rendered output, but take the new line numbers
            const sp = toEl.getAttribute('data-sourcepos');
            if (sp) fromEl.setAttribute('data-sourcepos', sp); else fromEl.removeAttribute('data-sourcepos');
            return false;
          }
        }
        if (fromEl.isEqualNode(toEl)) return false;
        return true;
      },
    });
  }

  // ------------------------------------------------------------------ scroll sync
  let syncIndex = [];
  const BLOCK = /^(P|H[1-6]|LI|PRE|TABLE|TR|BLOCKQUOTE|DIV|UL|OL|DL|DT|DD|HR|SECTION|NAV|FIGURE|DETAILS|VIDEO|IMG)$/;
  function buildSyncIndex() {
    syncIndex = [];
    content.querySelectorAll('[data-sourcepos]').forEach((el) => {
      if (!BLOCK.test(el.tagName)) return;
      const m = /^(\d+):\d+-(\d+):\d+$/.exec(el.getAttribute('data-sourcepos'));
      if (m) syncIndex.push({ el, start: +m[1], end: Math.max(+m[1], +m[2]) });
    });
  }
  let cursorEl = null;
  function scrollToLine(line0, instant) {
    if (line0 != null && ML.deckActive && ML.deckCursor) { ML.deckCursor(line0); return; }
    if (line0 == null || !syncIndex.length) return;
    const L = line0 + 1;
    let best = null;
    for (const e of syncIndex) {
      if (e.start <= L && L <= e.end && (!best || e.end - e.start <= best.end - best.start)) best = e;
    }
    let y;
    const top = (el) => el.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    if (best) {
      const h = best.el.getBoundingClientRect().height;
      const frac = best.end > best.start ? (L - best.start) / (best.end - best.start + 1) : 0;
      y = top(best.el) + (h > scroller.clientHeight * 0.6 ? h * frac : 0);
    } else {
      let prev = null, next = null;
      for (const e of syncIndex) { if (e.end < L && (!prev || e.end >= prev.end)) prev = e; if (e.start > L && (!next || e.start < next.start)) next = e; }
      if (!prev && !next) return;
      if (prev && next) {
        const a = top(prev.el) + prev.el.getBoundingClientRect().height, b = top(next.el);
        y = a + (b - a) * ((L - prev.end) / Math.max(1, next.start - prev.end));
      } else y = prev ? top(prev.el) + prev.el.getBoundingClientRect().height : top(next.el);
      best = prev || next;
    }
    if (cursorEl !== best.el) { cursorEl?.classList.remove('ml-cursor'); cursorEl = best.el; cursorEl.classList.add('ml-cursor'); }
    const rel = y - scroller.scrollTop;
    if (rel > scroller.clientHeight * 0.15 && rel < scroller.clientHeight * 0.7) return;
    const target = Math.max(0, y - scroller.clientHeight * 0.3);
    const far = Math.abs(target - scroller.scrollTop) > scroller.clientHeight * 2;
    scroller.scrollTo({ top: target, behavior: instant || far ? 'auto' : 'smooth' });
  }

  // ------------------------------------------------------------------ TOC sidebar
  function renderToc() {
    const nav = $('toc');
    nav.hidden = !state.toc;
    $('btn-toc').classList.toggle('on', state.toc);
    if (!state.toc) return;
    if (!state.headings.length) { nav.innerHTML = '<div class="empty">No headings</div>'; return; }
    nav.innerHTML = state.headings.map((h) => '<a class="l' + h.level + '" href="#' + escapeHtml(h.id) + '" data-line="' + h.line + '">' + escapeHtml(h.text) + '</a>').join('');
    markCurrentHeading();
  }
  function markCurrentHeading() {
    if (!state.toc) return;
    let cur = null;
    for (const h of state.headings) {
      const el = document.getElementById(h.id);
      if (el && el.getBoundingClientRect().top - scroller.getBoundingClientRect().top < 80) cur = h.id;
    }
    $('toc').querySelectorAll('a').forEach((a) => a.classList.toggle('current', a.getAttribute('href') === '#' + cur));
  }
  scroller.addEventListener('scroll', () => requestAnimationFrame(markCurrentHeading), { passive: true });

  // ------------------------------------------------------------------ links
  document.addEventListener('click', (ev) => {
    const a = ev.target.closest && ev.target.closest('a[href]');
    if (!a) return;
    const href = a.getAttribute('href');
    if (href.startsWith('#')) {
      ev.preventDefault();
      const id = decodeURIComponent(href.slice(1));
      const el = document.getElementById(id) || document.getElementsByName(id)[0];
      if (el) scroller.scrollTo({ top: el.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop - 10, behavior: 'smooth' });
      return;
    }
    if (href.startsWith('/fs/')) {
      const path = decodeURIComponent(href.slice(3).split('#')[0].split('?')[0]);
      if (/\.(md|markdown|mdown|mkd)$/i.test(path)) {
        ev.preventDefault();
        const frag = href.includes('#') ? href.slice(href.indexOf('#') + 1) : '';
        post('/api/open-link', { path }).then((r) => {
          if (!r.ok) toast('Cannot open ' + path);
          else if (frag) pendingAnchor = decodeURIComponent(frag);
        });
        return;
      }
      ev.preventDefault();
      window.open(href, '_blank');
      return;
    }
    if (/^https?:/i.test(href)) { ev.preventDefault(); window.open(href, '_blank', 'noopener'); }
  });
  let pendingAnchor = null;

  // ------------------------------------------------------------------ state updates
  function applyTheme() {
    const t = theme();
    document.documentElement.dataset.theme = t;
    $('hljs-light').disabled = t === 'dark';
    $('hljs-dark').disabled = t !== 'dark';
    const pref = state.themePref || 'auto';
    $('btn-theme').title = 'Theme: ' + (state.themePref ? pref : 'from settings (' + (state.settings.theme || 'auto') + ')') + ' — click to change (D)';
  }
  function applyZoom() {
    document.documentElement.style.setProperty('--zoom', state.zoom);
    $('btn-zoom').textContent = Math.round(state.zoom * 100) + '%';
  }

  async function onState(s) {
    if (s.newestTab && s.newestTab !== state.tab) { retire(); return; }
    const settingsChanged = JSON.stringify(s.settings || {}) !== JSON.stringify(state.settings);
    state.settings = s.settings || {};
    if (settingsChanged) applyTheme();
    if (s.html != null) {
      const fileChanged = s.file !== state.file;
      state.file = s.file;
      state.headings = s.headings || [];
      $('file').textContent = s.fileName || 'MarkDown Live';
      $('file').title = s.file || '';
      document.title = (s.title || s.fileName || 'Preview') + ' — MarkDown Live';
      const t0 = performance.now();
      applyHtml(s.html);
      if (fileChanged) { scroller.scrollTop = 0; cursorEl = null; }
      $('timing').textContent = 'rendered in ' + s.renderMs + ' ms (server) · ' + Math.round(performance.now() - t0) + ' ms (page)';
      renderToc();
      buildSyncIndex();
      ML.renderPromise = renderPlugins(false).then(() => {
        if (pendingAnchor) { const el = document.getElementById(pendingAnchor); pendingAnchor = null; el?.scrollIntoView(); }
        else if (state.sync && s.cursorLine != null && (fileChanged || s.cursorSeq !== state.cseq)) scrollToLine(s.cursorLine, fileChanged);
      });
      if (state.sync && s.cursorLine != null) scrollToLine(s.cursorLine, fileChanged);
      ML.onUpdate && ML.onUpdate(s);
    } else if (state.sync && s.cursorSeq !== state.cseq && s.cursorLine != null) {
      scrollToLine(s.cursorLine);
    }
    state.cseq = s.cursorSeq;
    state.cursorLine = s.cursorLine;
    state.version = s.version;
  }

  function retire() {
    state.retired = true;
    const d = document.createElement('div'); d.className = 'retired';
    d.innerHTML = '<div>This preview moved to a newer window.<br><small>You can close this tab.</small></div>';
    document.body.appendChild(d);
    $('dot').className = 'dot off'; $('conn').textContent = 'retired';
  }

  let failures = 0;
  async function poll() {
    while (!state.retired) {
      try {
        const vis = document.hidden ? 0 : 1;
        const url = '/api/state?since=' + state.version + '&cseq=' + state.cseq + '&tab=' + state.tab + '&born=' + state.born + '&visible=' + vis;
        const ctl = new AbortController();
        const timer = setTimeout(() => ctl.abort(), 30000);
        const r = await fetch(url, { cache: 'no-store', signal: ctl.signal });
        clearTimeout(timer);
        if (r.status === 403) { $('dot').className = 'dot off'; $('conn').textContent = 'session ended — open the preview again from the editor'; return; }
        const s = await r.json();
        failures = 0;
        $('dot').className = 'dot'; $('conn').textContent = s.mode === 'lsp' ? 'live (editor)' : 'live (file)';
        await onState(s);
      } catch (e) {
        failures++;
        $('dot').className = 'dot off';
        $('conn').textContent = failures > 5 ? 'disconnected — reopen the preview from the editor' : 'reconnecting…';
        await new Promise((res) => setTimeout(res, Math.min(5000, 500 * failures)));
      }
    }
  }
  document.addEventListener('visibilitychange', () => { /* the next poll reports visibility */ });

  // ------------------------------------------------------------------ toolbar
  $('btn-toc').onclick = () => { state.toc = !state.toc; store.set('toc', state.toc); renderToc(); };
  $('btn-sync').onclick = () => {
    state.sync = !state.sync; store.set('sync', state.sync);
    $('btn-sync').classList.toggle('on', state.sync);
    toast(state.sync ? 'Scroll sync on: the preview follows the editor cursor' : 'Scroll sync off');
    if (state.sync) scrollToLine(state.cursorLine);
  };
  $('btn-theme').onclick = () => {
    const order = [null, 'light', 'dark'];
    state.themePref = order[(order.indexOf(state.themePref) + 1) % order.length];
    store.set('theme', state.themePref);
    applyTheme();
    toast('Theme: ' + (state.themePref || 'from settings'));
    renderPlugins(false);
  };
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => { applyTheme(); renderPlugins(false); });
  $('btn-refresh').onclick = () => { post('/api/refresh', { hard: true }); document.querySelectorAll('[data-done]').forEach((e) => delete e.dataset.done); renderPlugins(true); };
  $('btn-print').onclick = () => window.print();
  const zoom = (z) => { state.zoom = Math.min(3, Math.max(0.5, Math.round(z * 10) / 10)); store.set('zoom', state.zoom); applyZoom(); };
  $('btn-zoom-in').onclick = () => zoom(state.zoom + 0.1);
  $('btn-zoom-out').onclick = () => zoom(state.zoom - 0.1);
  $('btn-zoom').onclick = () => zoom(1);
  document.addEventListener('keydown', (e) => {
    if (e.metaKey || e.ctrlKey || e.altKey || /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName || '')) return;
    if (ML.deckActive && e.key !== 'p' && e.key !== 'P') return; // reveal.js has the keyboard in slide view
    const k = e.key;
    if (k === 't' || k === 'T') $('btn-toc').click();
    else if (k === 's' || k === 'S') $('btn-sync').click();
    else if (k === 'd' || k === 'D') $('btn-theme').click();
    else if (k === 'r' || k === 'R') $('btn-refresh').click();
    else if (k === '+' || k === '=') zoom(state.zoom + 0.1);
    else if (k === '-') zoom(state.zoom - 0.1);
    else if (k === '0') zoom(1);
    else if (ML.onKey) ML.onKey(e);
  });

  /** Runs `fn` with the page temporarily in theme `t` (exports are always light). */
  ML.withTheme = async (t, fn) => {
    if (theme() === t) return fn();
    const prev = state.themePref;
    state.themePref = t; applyTheme(); await renderPlugins(false);
    try { return await fn(); } finally { state.themePref = prev; applyTheme(); renderPlugins(false); }
  };
  ML.renderPlugins = renderPlugins;

  $('btn-sync').classList.toggle('on', state.sync);
  applyTheme();
  applyZoom();
  renderToc();
  // Later versions add their own scripts (exports, slides…) before polling starts.
  (window.ML_EXTRA || []).forEach((f) => { try { f(ML); } catch (e) { console.error(e); } });
  // Start after every script of the page ran (plugins.js registers more renderers).
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', poll); else poll();
})();
