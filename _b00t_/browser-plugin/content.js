// b00t DOM Enrichment Engine
// Adds data-b00t attributes to every interactive element on the page,
// making them directly addressable by b00t RPA scripts without fragile CSS selectors.
//
// Enriched elements are queryable via: document.querySelectorAll('[data-b00t]')
// Each element gets: data-b00t-type, data-b00t-role, data-b00t-label, data-b00t-id

(function() {
  'use strict';

  const ENRICHED = new WeakSet();

  function enrichElement(el) {
    if (ENRICHED.has(el)) return;
    ENRICHED.add(el);

    const tag = el.tagName.toLowerCase();
    const type = el.getAttribute('type') || '';
    const role = el.getAttribute('role') || '';
    const id = el.id || '';
    const label = el.getAttribute('aria-label')
      || el.getAttribute('placeholder')
      || el.textContent?.trim()?.slice(0, 60)
      || el.getAttribute('name')
      || '';
    const href = el.getAttribute('href') || '';
    const name = el.getAttribute('name') || '';

    // Determine b00t type based on element semantics
    let b00tType = 'element';
    let b00tRole = 'generic';

    if (tag === 'a' && href) { b00tType = 'link'; b00tRole = 'navigation'; }
    else if (tag === 'button' || role === 'button') { b00tType = 'button'; b00tRole = 'action'; }
    else if (tag === 'input') {
      if (type === 'text' || type === 'search' || type === 'email' || type === 'url') { b00tType = 'input'; b00tRole = 'text'; }
      else if (type === 'checkbox') { b00tType = 'input'; b00tRole = 'checkbox'; }
      else if (type === 'radio') { b00tType = 'input'; b00tRole = 'radio'; }
      else if (type === 'submit' || type === 'button') { b00tType = 'button'; b00tRole = 'submit'; }
      else { b00tType = 'input'; b00tRole = type || 'text'; }
    }
    else if (tag === 'textarea') { b00tType = 'input'; b00tRole = 'textarea'; }
    else if (tag === 'select') { b00tType = 'input'; b00tRole = 'select'; }
    else if (tag === 'form') { b00tType = 'form'; b00tRole = 'container'; }
    else if (role === 'dialog' || role === 'alertdialog') { b00tType = 'dialog'; b00tRole = 'modal'; }
    else if (tag === 'nav' || role === 'navigation') { b00tType = 'nav'; b00tRole = 'navigation'; }
    else if (tag === 'img') { b00tType = 'image'; b00tRole = 'media'; }
    else if (['h1','h2','h3','h4','h5','h6'].includes(tag)) { b00tType = 'heading'; b00tRole = `h${tag[1]}`; }
    else if (tag === 'iframe') { b00tType = 'frame'; b00tRole = 'embedded'; }

    el.dataset.b00tType = b00tType;
    el.dataset.b00tRole = b00tRole;
    if (label) el.dataset.b00tLabel = label.slice(0, 120);
    if (id) el.dataset.b00tId = id;
    if (name) el.dataset.b00tName = name;
  }

  // Enrich all interactive elements on the page
  function enrichPage() {
    const selector = 'a, button, input, textarea, select, form, [role="button"], [role="dialog"], [role="navigation"], nav, iframe, img, h1, h2, h3, h4, h5, h6';
    document.querySelectorAll(selector).forEach(enrichElement);
  }

  // Watch for DOM changes (SPA: new elements loaded dynamically)
  const observer = new MutationObserver((mutations) => {
    let needsEnrich = false;
    for (const m of mutations) {
      if (m.addedNodes.length > 0) { needsEnrich = true; break; }
    }
    if (needsEnrich) {
      document.querySelectorAll('[data-b00t-type]').forEach(el => ENRICHED.add(el)); // mark existing
      document.querySelectorAll('a, button, input, textarea, select, form, [role="button"], [role="dialog"], nav, iframe, img, h1, h2, h3, h4, h5, h6')
        .forEach(enrichElement);
    }
  });
  observer.observe(document.body || document.documentElement, { childList: true, subtree: true });

  // Run on load
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', enrichPage);
  } else {
    enrichPage();
  }

  console.log('🐝 b00t DOM enrichment active —', document.querySelectorAll('[data-b00t-type]').length, 'elements enriched');

  // Inject enrichment API into the MAIN page context (not just isolated world)
  // so window.__b00t is accessible from CDP Runtime.evaluate and the console.
  const script = document.createElement('script');
  script.textContent = `
    window.__b00t = {
      enrichedCount: () => document.querySelectorAll('[data-b00t-type]').length,
      findB00t: (type, role) => {
        let sel = '[data-b00t-type]';
        if (type) sel += '[data-b00t-type="' + type + '"]';
        if (role) sel += '[data-b00t-role="' + role + '"]';
        return Array.from(document.querySelectorAll(sel)).map(function(el) { return {
          type: el.dataset.b00tType,
          role: el.dataset.b00tRole,
          label: el.dataset.b00tLabel || '',
          id: el.dataset.b00tId || el.id || '',
          selector: '[data-b00t-type="' + el.dataset.b00tType + '"]' + (el.dataset.b00tLabel ? '[data-b00t-label="' + el.dataset.b00tLabel + '"]' : ''),
          tag: el.tagName.toLowerCase(),
        };});
      },
      enrichCountByType: function() {
        var counts = {};
        document.querySelectorAll('[data-b00t-type]').forEach(function(el) {
          var t = el.dataset.b00tType;
          counts[t] = (counts[t] || 0) + 1;
        });
        return counts;
      },
    };
  `;
  document.documentElement.appendChild(script);
  script.remove();
})();
