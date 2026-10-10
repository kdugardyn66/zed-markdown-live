/* MarkDown Live v6 — 3D molecules with Mol* (molstar.org).
 *
 * ```pdb / ```sdf / ```mol2 / ```xyz / ```mmcif … blocks, ```molstar (format detected),
 * @import "protein.pdb", and ```molstar {pdb=1CRN} (downloaded from the RCSB PDB:
 * the only case that goes online). Drag to rotate, scroll to zoom, right-drag to move. */
(function (ML) {
  'use strict';
  const { loadScript, loadCss, errorBox, theme } = ML;
  const viewers = new Map(); // element → { viewer, src }

  function opts(el) { try { return JSON.parse(el.dataset.opts || '{}'); } catch (e) { return {}; } }
  let gl = null;
  function webgl() {
    if (gl === null) {
      try { const c = document.createElement('canvas'); gl = !!(c.getContext('webgl2') || c.getContext('webgl')); } catch (e) { gl = false; }
    }
    return gl;
  }
  const background = () => (theme() === 'dark' ? 0x0d1117 : 0xffffff);

  function setBackground(viewer) {
    try {
      viewer.plugin.canvas3d?.setProps({ renderer: { backgroundColor: background() } });
    } catch (e) { /* older Mol* */ }
  }

  function dispose(el) {
    const v = viewers.get(el);
    if (!v) return;
    viewers.delete(el);
    try { v.viewer.plugin.dispose(); } catch (e) { /* gone */ }
  }
  // browsers allow only a few WebGL contexts: free the ones whose block left the page
  setInterval(() => { for (const el of [...viewers.keys()]) if (!el.isConnected) dispose(el); }, 3000);

  ML.register({
    name: 'molstar', selector: '.ml-diagram[data-kind="molstar"]', themed: true,
    key: (el) => el.dataset.h,
    async load() {
      loadCss('/vendor/molstar/viewer/theme/light.css');
      await loadScript('/vendor/molstar/viewer/molstar.js', 'molstar');
    },
    async render(el) {
      const o = opts(el);
      const src = el.dataset.src ?? (el.querySelector('pre.ml-src')?.textContent || '');
      el.dataset.src = src;
      const ident = el.dataset.h;
      // same structure, only the theme changed: keep the viewer, change the background
      const prev = viewers.get(el);
      if (prev && prev.ident === ident && el.querySelector('.ml-molstar-box')) { setBackground(prev.viewer); return; }
      for (const [other, v] of viewers) if (!other.isConnected) dispose(other);
      dispose(el);

      el.replaceChildren();
      const box = document.createElement('div');
      box.className = 'ml-molstar-box';
      const h = parseInt(o.height, 10);
      box.style.height = (h > 80 ? h : 420) + 'px';
      el.appendChild(box);
      if (o.title) { const c = document.createElement('div'); c.className = 'ml-molstar-title'; c.textContent = o.title; el.appendChild(c); }

      if (!webgl()) { box.remove(); errorBox(el, 'Mol*: this browser has WebGL turned off, so 3D molecules can\'t be shown'); return; }
      const viewer = await ML.lib('molstar').Viewer.create(box, {
        layoutIsExpanded: false, layoutShowControls: false, layoutShowRemoteState: false,
        layoutShowSequence: false, layoutShowLog: false, layoutShowLeftPanel: false,
        viewportShowExpand: true, viewportShowSelectionMode: false, viewportShowAnimation: false,
        viewportShowControls: false, viewportShowSettings: false, viewportShowTrajectoryControls: false,
        pdbProvider: 'rcsb', emdbProvider: 'rcsb', collapseLeftPanel: true,
      });
      viewers.set(el, { viewer, ident });
      setBackground(viewer);
      try {
        if (o.pdb) await viewer.loadPdb(String(o.pdb).trim().toLowerCase());
        else if (o.url) await viewer.loadStructureFromUrl(o.url, o.format || 'pdb', !!o.binary);
        else if (src.trim()) await viewer.loadStructureFromData(src, o.format || 'pdb', false);
        else throw new Error('no structure: paste PDB/SDF/MOL2/XYZ/mmCIF text, @import a file, or use {pdb=1CRN}');
        if (!viewer.plugin.managers.structure.hierarchy.current.structures.length) throw new Error('could not read the structure as ' + (o.format || 'pdb'));
      } catch (e) {
        errorBox(el, 'Mol*: ' + (e && e.message ? e.message : e));
      }
      if (o.spin === 'true' || o.spin === true) {
        try { viewer.plugin.canvas3d?.setProps({ trackball: { animate: { name: 'spin', params: { speed: 0.1, axis: [0, -1, 0] } } } }); } catch (e) { /* ignore */ }
      }
      el.dataset.ready = '1';
    },
  });

  /** A PNG of the current view, for printing. */
  ML.molstarImage = async (el) => {
    const v = viewers.get(el);
    if (!v) return null;
    try {
      const helper = v.viewer.plugin.helpers.viewportScreenshot;
      if (helper) return await helper.getImageDataUri();
    } catch (e) { /* fall back */ }
    try { const c = el.querySelector('canvas'); v.viewer.plugin.canvas3d?.commit(true); return c ? c.toDataURL('image/png') : null; } catch (e) { return null; }
  };
})(window.ML);
