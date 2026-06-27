// b00t Browser Plugin — sidebar panel UI
(function() {
  const searchBar = document.getElementById('searchBar');
  const cmdList = document.getElementById('cmdList');
  const scriptBar = document.getElementById('scriptBar');
  const statusDot = document.getElementById('statusDot');
  const statusText = document.getElementById('statusText');
  const btnExecute = document.getElementById('btnExecute');
  const btnClear = document.getElementById('btnClear');
  const btnScreenshot = document.getElementById('btnScreenshot');
  const actionButtons = document.getElementById('actionButtons');

  let commands = B00T_CLIENT.commands;
  let filtered = [...commands];
  let selectedIdx = 0;
  let connected = false;

  // Try to attach to the active tab
  async function connect() {
    const tabs = await new Promise(r => chrome.tabs.query({ active: true, currentWindow: true }, r));
    if (tabs.length === 0) {
      setStatus(false, 'no active tab');
      return;
    }
    const ok = await B00T_CLIENT.attach(tabs[0].id);
    if (ok) {
      connected = true;
      setStatus(true, `connected · ${tabs[0].title?.slice(0, 30)}`);
      actionButtons.style.display = 'flex';
    } else {
      setStatus(false, 'attach failed — refresh page?');
    }
  }

  function setStatus(ok, msg) {
    statusDot.className = 'status-dot' + (ok ? ' connected' : '');
    statusText.textContent = msg || (ok ? 'connected' : 'disconnected');
  }

  function renderList() {
    cmdList.innerHTML = '';
    filtered.forEach((cmd, i) => {
      const li = document.createElement('li');
      li.className = 'cmd-item' + (i === selectedIdx ? ' selected' : '');
      li.innerHTML = `
        <span>
          <span class="cmd-action">${cmd.action}</span>
          <span style="color:#888"> ${cmd.args}</span>
        </span>
        <span class="cmd-desc">${cmd.desc}</span>
      `;
      li.onclick = () => { selectedIdx = i; addCommand(i); };
      li.ondblclick = () => { selectedIdx = i; executeSingle(i); };
      cmdList.appendChild(li);
    });
    if (filtered.length > 0) {
      const sel = cmdList.children[selectedIdx];
      if (sel) sel.scrollIntoView({ block: 'nearest' });
    }
  }

  function updateScriptBar() {
    const steps = B00T_CLIENT.curated;
    if (steps.length === 0) {
      scriptBar.innerHTML = '📋 Press Enter on a command to add it';
    } else {
      scriptBar.innerHTML = steps.map((s, i) =>
        `<span class="step">${i + 1}. ${s.action}</span>`
      ).join('  →  ');
    }
  }

  function addCommand(idx) {
    const cmd = filtered[idx];
    if (!cmd) return;
    B00T_CLIENT.curated.push({ action: cmd.action, args: cmd.args });
    updateScriptBar();
  }

  async function executeSingle(idx) {
    const cmd = filtered[idx];
    if (!cmd || !connected) return;
    const result = await B00T_CLIENT.runCommand(cmd);
    scriptBar.textContent = `▶ ${cmd.action}: ${result?.slice(0, 100) || 'done'}`;
  }

  async function executeAll() {
    const steps = B00T_CLIENT.curated;
    if (steps.length === 0) return;
    scriptBar.textContent = '▶ executing...';
    for (let i = 0; i < steps.length; i++) {
      const s = steps[i];
      const cmd = commands.find(c => c.action === s.action);
      if (cmd) {
        const result = await B00T_CLIENT.runCommand(cmd, s.args);
        scriptBar.innerHTML += `<br><span class="step">${i + 1}. ${s.action}</span> ${(result || '').slice(0, 60)}`;
      }
    }
    B00T_CLIENT.curated = [];
    updateScriptBar();
    scriptBar.innerHTML += '<br>✅ Done';
  }

  function clearScript() {
    B00T_CLIENT.curated = [];
    updateScriptBar();
  }

  async function takeScreenshot() {
    if (!connected) return;
    const dataUrl = await B00T_CLIENT.screenshot();
    const w = window.open('');
    if (w) {
      w.document.write(`<img src="${dataUrl}" style="max-width:100%">`);
      w.document.title = 'b00t screenshot';
    }
  }

  // Search / filter
  searchBar.addEventListener('input', () => {
    const q = searchBar.value.toLowerCase();
    filtered = commands.filter(c =>
      c.action.includes(q) || c.desc.includes(q) || c.args.includes(q)
    );
    selectedIdx = 0;
    renderList();
  });

  searchBar.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && filtered.length > 0) {
      addCommand(selectedIdx);
    }
    if (e.key === 'ArrowDown') {
      selectedIdx = Math.min(selectedIdx + 1, filtered.length - 1);
      renderList();
      e.preventDefault();
    }
    if (e.key === 'ArrowUp') {
      selectedIdx = Math.max(selectedIdx - 1, 0);
      renderList();
      e.preventDefault();
    }
    if (e.key === 'Escape') {
      searchBar.value = '';
      filtered = [...commands];
      selectedIdx = 0;
      renderList();
    }
  });

  // Buttons
  btnExecute.onclick = executeAll;
  btnClear.onclick = clearScript;
  btnScreenshot.onclick = takeScreenshot;

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'Enter') executeAll();
  });

  // Init
  connect();
  renderList();
})();
