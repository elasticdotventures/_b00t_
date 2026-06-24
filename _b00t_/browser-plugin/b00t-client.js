// b00t Browser Plugin — CDP bridge + RPA command palette
// Uses chrome.debugger API to attach to tabs and execute CDP commands.

const B00T_CLIENT = {
  commands: [
    { action: 'navigate', args: '<url>', desc: 'Navigate to a URL', eval: (url) => `window.location.href = '${url}'` },
    { action: 'click', args: '<selector>', desc: 'Click an element', eval: (sel) => `document.querySelector('${sel}').click()` },
    { action: 'type', args: '<sel> <text>', desc: 'Type into an input', eval: (sel, text) => `document.querySelector('${sel}').value = '${text}'` },
    { action: 'evaluate', args: '<js>', desc: 'Execute JavaScript', eval: (js) => js },
    { action: 'get_text', args: '', desc: 'Get page text content', eval: () => 'document.body.innerText' },
    { action: 'highlight', args: '<selector>', desc: 'Highlight elements', eval: (sel) => `document.querySelectorAll('${sel}').forEach(e => e.style.outline='2px solid #0ff')` },
    { action: 'get_links', args: '', desc: 'List all links', eval: () => 'Array.from(document.querySelectorAll("a")).map(a => a.href).join("\\n")' },
    { action: 'get_forms', args: '', desc: 'List all forms', eval: () => 'Array.from(document.forms).map(f => ({id:f.id,action:f.action,fields:Array.from(f.elements).map(e=>e.name)})).map(x=>JSON.stringify(x)).join("\\n")' },
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
          resolve(false);
          return;
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
    return new Promise((resolve, reject) => {
      chrome.debugger.sendCommand({ tabId: this.tabId }, 'Runtime.evaluate', {
        expression,
        returnByValue: true,
        awaitPromise: true,
      }, (result) => {
        if (chrome.runtime.lastError) {
          reject(chrome.runtime.lastError);
          return;
        }
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
    const js = cmd.eval(...args);
    return await this.evaluate(js);
  },

  async screenshot() {
    return new Promise((resolve) => {
      chrome.tabs.captureVisibleTab(null, { format: 'png' }, (dataUrl) => {
        resolve(dataUrl);
      });
    });
  },
};
