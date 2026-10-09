/* MarkDown Live v2 — more renderers: Graphviz, Vega/Vega-Lite, WaveDrom, flowchart.js,
 * Chart.js, Plotly, SMILES (chemistry), MathJax (+ mhchem for KaTeX), and the
 * code-chunk banner. All libraries are bundled and loaded on first use. */
(function (ML) {
  'use strict';
  const { loadScript, errorBox, escapeHtml, theme } = ML;

  // ------------------------------------------------------------------ helpers
  function srcOf(el) {
    if (el.dataset.src == null) {
      let t = el.querySelector('pre.ml-src')?.textContent ?? '';
      if (el.dataset.b64) {
        try { t = new TextDecoder().decode(Uint8Array.from(atob(t.trim()), (c) => c.charCodeAt(0))); } catch (e) { /* keep */ }
      }
      el.dataset.src = t;
    }
    return el.dataset.src;
  }
  function opts(el) { try { return JSON.parse(el.dataset.opts || '{}'); } catch (e) { return {}; } }
  function colors() {
    const dark = theme() === 'dark';
    return { dark, fg: dark ? '#d1d7e0' : '#1f2328', muted: dark ? '#9198a1' : '#59636e', grid: dark ? '#3d444d' : '#d1d9e0', bg: dark ? '#0d1117' : '#ffffff' };
  }
  function fail(el, what, e) {
    el.innerHTML = '<pre class="ml-src">' + escapeHtml(el.dataset.src || '') + '</pre>';
    errorBox(el, what + ': ' + (e && e.message ? e.message : e));
  }

  /** JSON with the usual hand-written relaxations (JSON5-ish): comments, unquoted
   *  keys, single quotes, trailing commas, hex, Infinity/NaN. No code is evaluated. */
  function relaxedJson(text) {
    let i = 0; const s = text;
    const err = (m) => { const line = s.slice(0, i).split('\n').length; throw new Error(m + ' (line ' + line + ')'); };
    const ws = () => {
      for (;;) {
        while (i < s.length && /\s/.test(s[i])) i++;
        if (s[i] === '/' && s[i + 1] === '/') { while (i < s.length && s[i] !== '\n') i++; continue; }
        if (s[i] === '/' && s[i + 1] === '*') { const e = s.indexOf('*/', i + 2); i = e < 0 ? s.length : e + 2; continue; }
        if (s[i] === '#') { while (i < s.length && s[i] !== '\n') i++; continue; }
        break;
      }
    };
    const str = () => {
      const q = s[i++]; let out = '';
      while (i < s.length && s[i] !== q) {
        if (s[i] === '\\') {
          const c = s[++i];
          if (c === 'n') out += '\n'; else if (c === 't') out += '\t'; else if (c === 'r') out += '\r';
          else if (c === 'u') { out += String.fromCharCode(parseInt(s.substr(i + 1, 4), 16)); i += 4; }
          else if (c === '\n') { /* line continuation */ } else out += c;
          i++;
        } else out += s[i++];
      }
      if (s[i] !== q) err('unterminated string');
      i++; return out;
    };
    const ident = () => { const m = /^[A-Za-z_$][\w$]*/.exec(s.slice(i)); if (!m) err('unexpected ' + JSON.stringify(s[i] || 'end')); i += m[0].length; return m[0]; };
    const value = () => {
      ws();
      const c = s[i];
      if (c === '{') {
        i++; const o = {};
        for (;;) {
          ws(); if (s[i] === '}') { i++; return o; }
          const k = (s[i] === '"' || s[i] === "'") ? str() : (/[\d]/.test(s[i]) ? String(value()) : ident());
          ws(); if (s[i] !== ':') err('expected ":"'); i++;
          o[k] = value(); ws();
          if (s[i] === ',') { i++; continue; }
          if (s[i] === '}') { i++; return o; }
          err('expected "," or "}"');
        }
      }
      if (c === '[') {
        i++; const a = [];
        for (;;) {
          ws(); if (s[i] === ']') { i++; return a; }
          a.push(value()); ws();
          if (s[i] === ',') { i++; continue; }
          if (s[i] === ']') { i++; return a; }
          err('expected "," or "]"');
        }
      }
      if (c === '"' || c === "'") return str();
      const num = /^[+-]?(0x[0-9a-f]+|(\d+\.?\d*|\.\d+)(e[+-]?\d+)?|Infinity|NaN)/i.exec(s.slice(i));
      if (num) { i += num[0].length; const n = num[0]; return /^[+-]?0x/i.test(n) ? parseInt(n, 16) : Number(n); }
      const id = ident();
      if (id === 'true') return true; if (id === 'false') return false; if (id === 'null') return null; if (id === 'undefined') return undefined;
      err('unexpected "' + id + '"');
    };
    const v = value(); ws();
    if (i < s.length) err('unexpected text after the value');
    return v;
  }
  ML.relaxedJson = relaxedJson;

  // ------------------------------------------------------------------ Graphviz (Viz.js)
  ML.register({
    name: 'viz', selector: '.ml-diagram[data-kind="viz"]', themed: true, key: (el) => el.dataset.h,
    async load() {
      await loadScript('/vendor/viz/viz-standalone.js', 'Viz');
      this.viz = this.viz || await ML.lib('Viz').instance();
    },
    async render(el) {
      const src = srcOf(el); const c = colors();
      try {
        const svg = this.viz.renderSVGElement(src, {
          engine: opts(el).engine || 'dot',
          graphAttributes: { bgcolor: 'transparent', color: c.fg, fontcolor: c.fg },
          nodeAttributes: { color: c.fg, fontcolor: c.fg },
          edgeAttributes: { color: c.fg, fontcolor: c.fg },
        });
        el.replaceChildren(svg);
      } catch (e) { fail(el, 'Graphviz', e); }
    },
  });

  // ------------------------------------------------------------------ Vega / Vega-Lite
  for (const kind of ['vega', 'vega-lite']) {
    ML.register({
      name: kind, selector: '.ml-diagram[data-kind="' + kind + '"]', themed: true, key: (el) => el.dataset.h,
      async load() {
        await loadScript('/vendor/vega/vega.min.js', 'vega');
        await loadScript('/vendor/vega-lite/vega-lite.min.js', 'vegaLite');
        await loadScript('/vendor/vega-embed/vega-embed.min.js', 'vegaEmbed');
      },
      async render(el) {
        const src = srcOf(el); const c = colors();
        try {
          const spec = relaxedJson(src);
          el.__view?.finalize?.();
          const box = document.createElement('div'); el.replaceChildren(box);
          const res = await ML.lib('vegaEmbed')(box, spec, {
            mode: kind, actions: false, renderer: 'svg', theme: c.dark ? 'dark' : undefined,
            loader: ML.lib('vega').loader({ baseURL: el.dataset.base || '' }),
            config: { background: 'transparent' },
          });
          el.__view = res.view;
        } catch (e) { fail(el, kind === 'vega' ? 'Vega' : 'Vega-Lite', e); }
      },
    });
  }

  // ------------------------------------------------------------------ WaveDrom
  ML.register({
    name: 'wavedrom', selector: '.ml-diagram[data-kind="wavedrom"]', key: (el) => el.dataset.h, n: 0,
    async load() {
      await loadScript('/vendor/wavedrom/skins/default.js', 'WaveSkin');
      await loadScript('/vendor/wavedrom/wavedrom.min.js', 'WaveDrom');
    },
    async render(el) {
      const src = srcOf(el);
      try {
        const obj = relaxedJson(src);
        const n = 'ml' + (++this.n);
        const box = document.createElement('div'); box.id = 'WaveDrom_Display_' + n;
        el.replaceChildren(box);
        ML.lib('WaveDrom').RenderWaveForm(n, obj, 'WaveDrom_Display_');
      } catch (e) { fail(el, 'WaveDrom', e); }
    },
  });

  // ------------------------------------------------------------------ flowchart.js
  ML.register({
    name: 'flow', selector: '.ml-diagram[data-kind="flow"]', themed: true, key: (el) => el.dataset.h,
    async load() { await loadScript('/vendor/flowchart/flowchart.min.js', 'flowchart'); },
    async render(el) {
      const src = srcOf(el); const c = colors();
      try {
        const chart = ML.lib('flowchart').parse(src);
        const box = document.createElement('div'); el.replaceChildren(box);
        chart.drawSVG(box, { 'line-color': c.fg, 'element-color': c.fg, 'font-color': c.fg, fill: c.bg, 'yes-text': 'yes', 'no-text': 'no', 'font-family': 'inherit' });
      } catch (e) { fail(el, 'flowchart.js', e); }
    },
  });

  // ------------------------------------------------------------------ Chart.js
  ML.register({
    name: 'chart', selector: '.ml-diagram[data-kind="chart"]', themed: true, key: (el) => el.dataset.h,
    async load() { await loadScript('/vendor/chartjs/chart.umd.min.js', 'Chart'); },
    async render(el) {
      const src = srcOf(el); const c = colors(); const o = opts(el);
      try {
        const cfg = relaxedJson(src);
        const Chart = ML.lib('Chart');
        Chart.defaults.color = c.fg; // (don't touch Chart.defaults.borderColor: it breaks bar outlines)
        Chart.defaults.font.family = getComputedStyle(document.body).fontFamily;
        cfg.options = cfg.options || {};
        if (cfg.options.animation === undefined) cfg.options.animation = false;
        for (const ax of Object.values(cfg.options.scales || {})) { ax.grid = Object.assign({ color: c.grid }, ax.grid || {}); }
        el.__chart?.destroy();
        const box = document.createElement('div'); box.className = 'ml-chart-box';
        if (o.width) box.style.maxWidth = /^\d+$/.test(o.width) ? o.width + 'px' : o.width;
        const canvas = document.createElement('canvas'); box.appendChild(canvas);
        el.replaceChildren(box);
        if (!['radar', 'polarArea', 'pie', 'doughnut'].includes(cfg.type)) {
          cfg.options.scales = cfg.options.scales || {};
          for (const a of ['x', 'y']) { cfg.options.scales[a] = cfg.options.scales[a] || {}; cfg.options.scales[a].grid = Object.assign({ color: c.grid }, cfg.options.scales[a].grid || {}); }
        }
        el.__chart = new Chart(canvas, cfg);
      } catch (e) { fail(el, 'Chart.js', e); }
    },
  });

  // ------------------------------------------------------------------ Plotly
  ML.register({
    name: 'plotly', selector: '.ml-diagram[data-kind="plotly"]', themed: true, key: (el) => el.dataset.h,
    async load() { await loadScript('/vendor/plotly/plotly.min.js', 'Plotly'); },
    async render(el) {
      const src = srcOf(el); const c = colors();
      try {
        const fig = relaxedJson(src);
        const data = Array.isArray(fig) ? fig : (fig.data || []);
        const layout = Object.assign({}, fig.layout || {});
        layout.paper_bgcolor = 'rgba(0,0,0,0)'; layout.plot_bgcolor = 'rgba(0,0,0,0)';
        layout.font = Object.assign({ color: c.fg }, layout.font || {});
        if (layout.template && c.dark) delete layout.template; // python's default template is light
        const Plotly = ML.lib('Plotly');
        const box = el.querySelector(':scope > .ml-plotly-box') || document.createElement('div');
        box.className = 'ml-plotly-box';
        if (el.__plotted) Plotly.purge(box);
        el.replaceChildren(box);
        await Plotly.newPlot(box, data, layout, { responsive: true, displaylogo: false });
        el.__plotted = true;
      } catch (e) { fail(el, 'Plotly', e); }
    },
  });

  // ------------------------------------------------------------------ SMILES (chemical structures)
  ML.register({
    name: 'smiles', selector: '.ml-diagram[data-kind="smiles"]', themed: true, key: (el) => el.dataset.h,
    async load() { await loadScript('/vendor/smiles/smiles-drawer.min.js', 'SmilesDrawer'); },
    async render(el) {
      const SD = ML.lib('SmilesDrawer'); const c = colors(); const o = opts(el);
      const lines = srcOf(el).split('\n').map((l) => l.trim()).filter((l) => l && !l.startsWith('#'));
      const row = document.createElement('div'); row.className = 'ml-smiles-row';
      el.replaceChildren(row);
      const errors = [];
      for (const line of lines) {
        const [smiles, ...rest] = line.split(/\s+/);
        const fig = document.createElement('figure');
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        const w = Number(o.width) || 260, h = Number(o.height) || 200;
        svg.setAttribute('width', w); svg.setAttribute('height', h);
        fig.appendChild(svg);
        if (rest.length) { const cap = document.createElement('figcaption'); cap.textContent = rest.join(' '); fig.appendChild(cap); }
        row.appendChild(fig);
        await new Promise((resolve) => SD.parse(smiles, (tree) => {
          try { new SD.SvgDrawer({ width: w, height: h, padding: 10 }).draw(tree, svg, c.dark ? 'dark' : 'light', null); } catch (e) { errors.push(smiles + ': ' + e.message); }
          resolve();
        }, (e) => { errors.push(smiles + ': ' + (e.message || e)); fig.remove(); resolve(); }));
      }
      if (errors.length) errorBox(el, 'SMILES: ' + errors.join('\n'));
    },
  });

  // ------------------------------------------------------------------ math: MathJax option + mhchem
  const katexRender = ML.math.render.bind(ML.math);
  const pref = () => { try { return localStorage.getItem('ml.math'); } catch (e) { return null; } };
  ML.math.engine = () => pref() || ML.state.settings.math || 'katex';
  ML.math.loadKatex = async function () {
    ML.loadCss('/vendor/katex/katex.min.css');
    await loadScript('/vendor/katex/katex.min.js', 'katex');
    await loadScript('/vendor/katex/contrib/mhchem.min.js'); // \ce{…} and \pu{…}
  };
  ML.math.loadMathJax = async function () {
    if (!ML.lib('MathJax') || !window.MathJax.tex2svg) {
      if (!window.MathJax || window.MathJax instanceof Element) {
        window.MathJax = { tex: { packages: { '[+]': ['mhchem'] } }, svg: { fontCache: 'local' }, startup: { typeset: false } };
      }
      await loadScript('/vendor/mathjax/tex-svg-full.js');
      await window.MathJax.startup.promise;
    }
  };
  ML.math.render = async function (el, tex, display) {
    if (ML.math.engine() !== 'mathjax') return katexRender(el, tex, display);
    await ML.math.loadMathJax();
    try {
      const node = window.MathJax.tex2svg(tex, { display });
      el.replaceChildren(node);
    } catch (e) { el.textContent = tex; el.title = String(e.message || e); }
  };

  // ------------------------------------------------------------------ toolbar: math engine
  const extra = document.getElementById('ml-extra-buttons');
  const mathBtn = document.createElement('button');
  mathBtn.id = 'ml-btn-math'; mathBtn.className = 'hide-sm';
  const updateMathBtn = () => { mathBtn.textContent = ML.math.engine() === 'mathjax' ? 'MathJax' : 'KaTeX'; mathBtn.title = 'Math engine: ' + mathBtn.textContent + ' — click to switch (M)'; };
  mathBtn.onclick = () => {
    const next = ML.math.engine() === 'mathjax' ? 'katex' : 'mathjax';
    try { localStorage.setItem('ml.math', next); } catch (e) { /* ignore */ }
    updateMathBtn(); ML.toast('Math: ' + mathBtn.textContent);
    ML.rerender();
  };
  extra.appendChild(mathBtn);
  updateMathBtn();
  const prevKey = ML.onKey;
  ML.onKey = (e) => { if (e.key === 'm' || e.key === 'M') mathBtn.click(); else if (prevKey) prevKey(e); };

  // ------------------------------------------------------------------ code chunks banner + pending
  const prevUpdate = ML.onUpdate;
  ML.onUpdate = (s) => {
    prevUpdate && prevUpdate(s);
    updateMathBtn();
    const banner = document.getElementById('ml-banner');
    if (s.blockedChunks > 0) {
      banner.className = 'info';
      banner.innerHTML = '';
      const t = document.createElement('span');
      t.textContent = s.blockedChunks + ' code chunk' + (s.blockedChunks > 1 ? 's' : '') + ' in this document didn’t run (code execution is off). ';
      const b = document.createElement('button'); b.textContent = 'Run code chunks for this session';
      b.onclick = () => { b.disabled = true; ML.post('/api/allow-scripts'); };
      banner.append(t, b); banner.hidden = false;
    } else if (banner.className === 'info') {
      banner.hidden = true; banner.className = '';
    }
    const libs = document.getElementById('ml-libs');
    if (s.pending > 0) { libs.dataset.pending = '1'; libs.textContent = '⏳ rendering ' + s.pending + ' block' + (s.pending > 1 ? 's' : '') + '…'; }
    else if (libs.dataset.pending) { delete libs.dataset.pending; libs.textContent = ''; }
  };
})(window.ML);
