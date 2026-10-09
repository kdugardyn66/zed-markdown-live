/* MarkDown Live v3 — pictures and videos: tells the server what this browser can
 * play (open-source Chromium has no H.264/AAC; Chrome does), and a lightbox. */
(function (ML) {
  'use strict';
  const v = document.createElement('video');
  const h264 = v.canPlayType('video/mp4; codecs="avc1.42E01E, mp4a.40.2"') !== '';
  ML.post('/api/client', { h264 });

  // Lightbox: click a picture to see it full size (Esc or click to close).
  const box = document.createElement('div');
  box.id = 'ml-lightbox'; box.hidden = true;
  document.body.appendChild(box);
  const close = () => { box.hidden = true; box.replaceChildren(); };
  box.addEventListener('click', close);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !box.hidden) close(); });
  document.getElementById('ml-content').addEventListener('click', (e) => {
    const img = e.target.closest && e.target.closest('img');
    if (!img || img.closest('a') || img.closest('.ml-chunk-note')) return;
    const big = document.createElement('img');
    big.src = img.currentSrc || img.src; big.alt = img.alt;
    const cap = document.createElement('div'); cap.className = 'cap';
    cap.textContent = img.alt || img.title || '';
    box.replaceChildren(big, cap);
    box.hidden = false;
  });
})(window.ML);
