// b00t Browser Plugin — CDP bridge + DOM enrichment + RPA command palette
//
// Uses chrome.debugger API for CDP commands and the enrichment engine
// (content.js) for DOM-relative selectors via [data-b00t-*] attributes.
//
// Key capability: selectors like [data-b00t-type="button"][data-b00t-label="Submit"]
// are STABLE across page redesigns — unlike fragile CSS class selectors.

const B00T_CLIENT = {
  commands: [
    // ─── Navigation ────────────────────────────────────────────────────
    { action: 'navigate', args: '<url>', desc: 'Navigate to a URL',
      eval: (url) => `window.location.href = '${url}'` },

    // ─── DOM-targeted actions (leveraging data-b00t enrichment) ─────────
    { action: 'click', args: '<type> <label>', desc: 'Click enriched element by type+label',
      eval: (type, label) =>
        `document.querySelector('[data-b00t-type="${type}"][data-b00t-label="${label}"]')?.click()` },

    { action: 'type', args: '<label> <text>', desc: 'Type into enriched input by label',
      eval: (label, text) => {
        const el = document.querySelector(`[data-b00t-type="input"][data-b00t-label="${label}"]`)
                || document.querySelector(`[data-b00t-type="input"][data-b00t-name="${label}"]`)
                || document.querySelector(`input[name="${label}"]`);
        if (!el) return 'NOT_FOUND';
        el.value = text;
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
        return `typed "${text}"`;
      } },

    { action: 'select_text', args: '<label>', desc: 'Select text in enriched input',
      eval: (label) => {
        const el = document.querySelector(`[data-b00t-type="input"][data-b00t-label="${label}"]`);
        if (!el) return 'NOT_FOUND';
        el.select(); return 'selected';
      } },

    { action: 'clear', args: '<label>', desc: 'Clear enriched input',
      eval: (label) => {
        const el = document.querySelector(`[data-b00t-type="input"][data-b00t-label="${label}"]`);
        if (!el) return 'NOT_FOUND';
        el.value = ''; return 'cleared';
      } },

    // ─── Form interaction ───────────────────────────────────────────────
    { action: 'submit', args: '', desc: 'Submit nearest form',
      eval: () => `document.querySelector('form')?.requestSubmit(); 'submitted'` },

    { action: 'select_option', args: '<label> <value>', desc: 'Select option in enriched select',
      eval: (label, value) => {
        const el = document.querySelector(`[data-b00t-type="input"][data-b00t-role="select"][data-b00t-label="${label}"]`);
        if (!el) return 'NOT_FOUND';
        el.value = value;
        el.dispatchEvent(new Event('change', { bubbles: true }));
        return `selected "${value}"`;
      } },

    // ─── DOM introspection ──────────────────────────────────────────────
    { action: 'list_enriched', args: '<type_filter>', desc: 'List enriched elements (filter by type)',
      eval: (type) => `JSON.stringify(window.__b00t?.findB00t(${type ? `'${type}'` : ''}) || [])` },

    { action: 'resolve_selector', args: '<type> <label>',
      desc: 'selector-forge propose→verify→settle: rank candidates, return the first that re-verifies' },

    { action: 'enrich_counts', args: '', desc: 'Count enriched elements by type',
      eval: () => {
        const counts = {};
        document.querySelectorAll('[data-b00t-type]').forEach(el => {
          const t = el.dataset.b00tType;
          counts[t] = (counts[t] || 0) + 1;
        });
        return JSON.stringify(counts);
      } },

    // ─── JavaScript execution ──────────────────────────────────────────
    { action: 'evaluate', args: '<js>', desc: 'Execute arbitrary JavaScript',
      eval: (js) => js },
    { action: 'highlight', args: '<selector>', desc: 'Highlight elements matching CSS selector',
      eval: (sel) => `document.querySelectorAll('${sel}').forEach(e => e.style.outline='3px solid #00ff88')` },

    // ─── Content extraction ─────────────────────────────────────────────
    { action: 'get_text', args: '', desc: 'Get page visible text',
      eval: () => 'document.body.innerText' },
    { action: 'get_links', args: '', desc: 'List all links (href + label)',
      eval: () => 'Array.from(document.querySelectorAll("a[href]")).slice(0,100).map(a => ({href:a.href,text:a.textContent.trim().slice(0,60)})).map(x=>JSON.stringify(x)).join("\\n")' },
    { action: 'get_forms', args: '', desc: 'List all forms and their enriched fields',
      eval: () => 'Array.from(document.forms).map(f => ({id:f.id,action:f.action,fields:Array.from(f.querySelectorAll("[data-b00t-type]")).map(e => e.dataset.b00tLabel || e.name)})).map(x=>JSON.stringify(x)).join("\\n")' },
    { action: 'get_tables', args: '', desc: 'Extract table data as JSON',
      eval: () => 'Array.from(document.querySelectorAll("table")).map(t => ({caption:t.caption?.textContent||"",headers:Array.from(t.querySelectorAll("th")).map(h=>h.textContent.trim()),rows:Array.from(t.querySelectorAll("tr")).slice(0,10).map(r=>Array.from(r.querySelectorAll("td")).map(d=>d.textContent.trim()))})).map(x=>JSON.stringify(x)).join("\\n")' },

    // ─── Visual ─────────────────────────────────────────────────────────
    { action: 'screenshot', args: '', desc: 'Capture visible page screenshot (returns data URL)',
      eval: () => new Promise((resolve) => {
        chrome.tabs.captureVisibleTab(null, { format: 'png' }, resolve)
      }) },
    { action: 'screenshot_full', args: '', desc: 'Capture full page screenshot via CDP',
      eval: () => 'FULL_PAGE_SCREENSHOT' },
  ],

  tabId: null,
  debuggerAttached: false,
  curated: [],

  async attach(tabId) {
    this.tabId = tabId;
    return new Promise((resolve) => {
      chrome.debugger.attach({ tabId }, '1.3', () => {
        if (chrome.runtime.lastError) {
          console.warn('b00t: debugger attach error:', chrome.runtime.lastError);
          resolve(false); return;
        }
        this.debuggerAttached = true;
        chrome.debugger.sendCommand({ tabId }, 'Page.enable', {}, () => {
          chrome.debugger.sendCommand({ tabId }, 'Runtime.enable', {}, resolve(true));
        });
      });
    });
  },

  async detach() {
    if (this.tabId && this.debuggerAttached) {
      return new Promise((resolve) => {
        chrome.debugger.detach({ tabId: this.tabId }, () => resolve());
      });
    }
  },

  async evaluate(expression) {
    // Check if expression is a Promise-returning function (for chrome API calls)
    if (expression === 'FULL_PAGE_SCREENSHOT') {
      return await this.fullPageScreenshot();
    }
    return new Promise((resolve, reject) => {
      chrome.debugger.sendCommand({ tabId: this.tabId }, 'Runtime.evaluate', {
        expression,
        returnByValue: true,
        awaitPromise: true,
      }, (result) => {
        if (chrome.runtime.lastError) { reject(chrome.runtime.lastError); return; }
        const value = result?.result?.value;
        resolve(value !== undefined ? String(value) : 'undefined');
      });
    });
  },

  async runCommand(cmd, ...args) {
    if (cmd.action === 'navigate' && args[0]) {
      return new Promise((resolve) => {
        chrome.tabs.update(this.tabId, { url: args[0] }, resolve);
      }).then(() => 'navigating...');
    }
    if (cmd.action === 'resolve_selector') {
      // Routed via chrome.tabs.sendMessage (not CDP Runtime.evaluate, which
      // only reaches the MAIN world) to content.js's isolated-world bridge,
      // which calls the tested selector-forge.js module directly.
      const [type, label] = args;
      return new Promise((resolve) => {
        chrome.tabs.sendMessage(
          this.tabId,
          { action: 'b00t_resolve_selector', type, label },
          (result) => resolve(JSON.stringify(result ?? null))
        );
      });
    }
    const js = typeof cmd.eval === 'function' ? cmd.eval(...args) : cmd.eval;
    return await this.evaluate(js);
  },

  // Visible tab screenshot
  async screenshot() {
    return new Promise((resolve) => {
      chrome.tabs.captureVisibleTab(null, { format: 'png' }, (dataUrl) => resolve(dataUrl));
    });
  },

  // Full page screenshot via CDP Page.captureScreenshot
  async fullPageScreenshot() {
    const { contentWidth, contentHeight } = await new Promise((resolve) => {
      chrome.debugger.sendCommand({ tabId: this.tabId }, 'Page.getLayoutMetrics', {}, (r) => {
        resolve({
          contentWidth: r?.contentSize?.width || 1920,
          contentHeight: r?.contentSize?.height || 1080,
        });
      });
    });
    // Temporarily set viewport to full content size, screenshot, restore
    await new Promise((r) => {
      chrome.debugger.sendCommand({ tabId: this.tabId }, 'Emulation.setDeviceMetricsOverride', {
        width: Math.ceil(contentWidth),
        height: Math.ceil(contentHeight),
        deviceScaleFactor: 1,
        mobile: false,
      }, r);
    });
    const dataUrl = await new Promise((resolve) => {
      chrome.debugger.sendCommand({ tabId: this.tabId }, 'Page.captureScreenshot', {
        format: 'png',
        fromSurface: true,
      }, (r) => resolve(r?.data ? `data:image/png;base64,${r.data}` : ''));
    });
    // Restore viewport
    await new Promise((r) => {
      chrome.debugger.sendCommand({ tabId: this.tabId }, 'Emulation.clearDeviceMetricsOverride', {}, r);
    });
    return dataUrl || 'screenshot_failed';
  },
};
