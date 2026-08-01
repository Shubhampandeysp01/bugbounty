// ─── State ─────────────────────────────────────────────────────────────────
const state = {
  tree: [],
  currentFile: null,
  sidebarOpen: true,
  rawViewerOpen: false,
  mode: 'learn', // 'learn' | 'tools' | 'home'
  toolStatus: {},
};

// ─── DOM References ─────────────────────────────────────────────────────────
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const dom = {
  sidebar: $('#sidebar'),
  sidebarToggle: $('#sidebar-toggle'),
  sidebarBackdrop: $('#sidebar-backdrop'),
  treeContainer: $('#tree-container'),
  collapseAll: $('#collapse-all'),
  searchInput: $('#search-input'),
  searchResults: $('#search-results'),
  welcome: $('#welcome'),
  reader: $('#reader'),
  readerTitle: $('#reader-title'),
  readerCategory: $('#reader-category'),
  readerContent: $('#reader-content'),
  readerRaw: $('#reader-raw'),
  rawViewer: $('#raw-viewer'),
  rawContent: $('#raw-content'),
  rawClose: $('#raw-close'),
  statsBadge: $('#stats-badge'),
  countGuides: $('#count-guides'),
  countReferences: $('#count-references'),
  countCaseStudies: $('#count-case-studies'),
  toolsPanel: $('#tools-panel'),
  toolsNav: $('#tools-nav'),
  learnEmpty: $('#learn-empty'),
  countTools: $('#count-tools'),
  modeSwitcher: $('.mode-switcher'),
  modeBtns: $$('.mode-btn'),
  sidebarLearn: $('#sidebar-learn'),
  sidebarTools: $('#sidebar-tools'),
  pathCards: $$('.path-card'),
  logoHome: $('#logo-home'),
  vulnDetail: $('#vuln-detail'),
  vulnDetailBack: $('#vuln-detail-back'),
  vulnDetailTitle: $('#vuln-detail-title'),
  vulnDetailSub: $('#vuln-detail-sub'),
  vulnDetailBadge: $('#vuln-detail-badge'),
  vulnDetailBody: $('#vuln-detail-body'),
  vulnDetailCveExt: $('#vuln-detail-cve-ext'),
};

// Live NodeList helpers (tools are built dynamically)
const toolItems = () => $$('.tool-item');

// ─── API ───────────────────────────────────────────────────────────────────
const api = {
  async getTree() {
    const res = await fetch('/api/tree');
    return res.json();
  },
  async getFile(path) {
    const res = await fetch(`/api/file?path=${encodeURIComponent(path)}`);
    if (!res.ok) throw new Error('File not found');
    return res.json();
  },
  async search(query, limit = 20) {
    const res = await fetch(`/api/search?q=${encodeURIComponent(query)}&limit=${limit}`);
    return res.json();
  },
  async getStats() {
    const res = await fetch('/api/stats');
    return res.json();
  },
};

// ─── Mode Switching ────────────────────────────────────────────────────────
function updateModeIndicator() {
  const indicator = $('#mode-indicator');
  if (!indicator || !dom.modeSwitcher) return;

  if (state.mode === 'home') {
    indicator.classList.add('is-idle');
    return;
  }

  indicator.classList.remove('is-idle');
  const activeBtn = document.querySelector(`.mode-btn[data-mode="${state.mode}"]`);
  if (!activeBtn) return;

  const switcherRect = dom.modeSwitcher.getBoundingClientRect();
  const btnRect = activeBtn.getBoundingClientRect();
  const left = btnRect.left - switcherRect.left;
  indicator.style.width = `${btnRect.width}px`;
  indicator.style.transform = `translateX(${left}px)`;
}

// ─── History navigation ─────────────────────────────────────────────────────
// The very first navigation triggered by a deep-link (?path=…, ?tool=…, …)
// replaces the current entry instead of pushing, so the Back button doesn't
// just bounce back to the same URL.
let initialNav = true;

function nav(url, state) {
  if (initialNav) {
    history.replaceState(state, '', url);
    initialNav = false;
  } else {
    history.pushState(state, '', url);
  }
}

function setMode(mode, { push = true } = {}) {
  state.mode = mode;

  // Mode switcher UI — home = neither path locked in
  dom.modeBtns.forEach((btn) => {
    const isActive = mode !== 'home' && btn.dataset.mode === mode;
    btn.classList.toggle('active', isActive);
    btn.setAttribute('aria-selected', isActive ? 'true' : 'false');
  });

  dom.modeSwitcher.dataset.active = mode === 'home' ? '' : mode;
  requestAnimationFrame(updateModeIndicator);

  // Sidebar panels: tools path vs learn/home library
  if (mode === 'tools') {
    dom.sidebarLearn.classList.remove('active');
    dom.sidebarTools.classList.add('active');
  } else {
    dom.sidebarLearn.classList.add('active');
    dom.sidebarTools.classList.remove('active');
  }

  // Open sidebar when entering a path (desktop); mobile stays user-controlled
  if (mode !== 'home' && !state.sidebarOpen && !isMobileLayout()) {
    openSidebar();
  }

  if (push && mode === 'home') {
    nav(window.location.pathname, { mode: 'home' });
  } else if (push && mode === 'learn' && !state.currentFile) {
    nav('?mode=learn', { mode: 'learn' });
  } else if (push && mode === 'tools') {
    nav('?mode=tools', { mode: 'tools' });
  }
}

function hideAllViews() {
  dom.welcome.classList.add('hidden');
  dom.reader.classList.add('hidden');
  dom.rawViewer.classList.add('hidden');
  dom.toolsPanel.classList.add('hidden');
  if (dom.learnEmpty) dom.learnEmpty.classList.add('hidden');
  if (dom.vulnDetail) dom.vulnDetail.classList.add('hidden');
}

function showHome() {
  state.currentFile = null;
  state.rawViewerOpen = false;
  setMode('home', { push: true });

  hideAllViews();
  dom.welcome.classList.remove('hidden');
  toolItems().forEach((el) => el.classList.remove('active'));
  $$('.tree-node-header.active').forEach((el) => el.classList.remove('active'));

  const contentEl = document.getElementById('content');
  contentEl.scrollTo({ top: 0, behavior: 'smooth' });
}

function enterLearnPath({ push = true } = {}) {
  setMode('learn', { push });
  hideAllViews();
  toolItems().forEach((el) => el.classList.remove('active'));

  if (state.currentFile) {
    dom.reader.classList.remove('hidden');
  } else if (dom.learnEmpty) {
    dom.learnEmpty.classList.remove('hidden');
  }
}

function enterToolsPath({ push = true, toolId = null } = {}) {
  setMode('tools', { push: false });
  if (toolId) {
    switchToTool(toolId, { push });
    return;
  }
  const first =
    document.querySelector('.tool-item') ||
    (window.VAULT_TOOLS && window.VAULT_TOOLS.allTools()[0]);
  if (first && first.dataset) {
    switchToTool(first.dataset.tool, { push });
  } else if (first && first.id) {
    switchToTool(first.id, { push });
  } else {
    hideAllViews();
    dom.toolsPanel.classList.remove('hidden');
    if (push) nav('?mode=tools', { mode: 'tools' });
  }
}

// Mode switcher clicks
dom.modeBtns.forEach((btn) => {
  btn.addEventListener('click', () => {
    const mode = btn.dataset.mode;
    if (mode === 'learn') {
      enterLearnPath({ push: true });
    } else if (mode === 'tools') {
      enterToolsPath({ push: true });
    }
  });
});

// Path cards on landing
dom.pathCards.forEach((card) => {
  card.addEventListener('click', () => {
    const path = card.dataset.path;
    if (path === 'learn') {
      enterLearnPath({ push: true });
    } else if (path === 'tools') {
      enterToolsPath({ push: true });
    }
  });
});

// Logo → home
if (dom.logoHome) {
  dom.logoHome.addEventListener('click', (e) => {
    e.preventDefault();
    showHome();
  });
}

// ─── Tree Rendering ────────────────────────────────────────────────────────
function renderTree(nodes, container, depth = 0) {
  container.innerHTML = '';
  for (const node of nodes) {
    const nodeEl = createTreeNode(node, depth);
    container.appendChild(nodeEl);
  }
}

function createTreeNode(node, depth) {
  const wrapper = document.createElement('div');
  wrapper.className = 'tree-node';

  const header = document.createElement('div');
  header.className = 'tree-node-header';
  header.dataset.path = node.path;
  header.dataset.isDir = node.is_dir;

  const toggle = document.createElement('span');
  toggle.className = 'tree-toggle';
  if (node.is_dir) {
    toggle.textContent = '▶';
    toggle.classList.add('expanded');
  } else {
    toggle.classList.add('hidden');
  }
  header.appendChild(toggle);

  const icon = document.createElement('span');
  icon.className = 'tree-icon';
  if (node.is_dir) {
    icon.textContent =
      node.name === 'guides' ? '📘' :
      node.name === 'references' ? '📚' :
      node.name === 'case-studies' ? '🔬' : '📁';
  } else {
    icon.textContent = '📄';
  }
  header.appendChild(icon);

  const label = document.createElement('span');
  label.className = `tree-label${node.is_dir ? '' : ' file'}`;
  label.textContent = node.name;
  header.appendChild(label);

  wrapper.appendChild(header);

  if (node.is_dir && node.children.length > 0) {
    const childrenContainer = document.createElement('div');
    childrenContainer.className = 'tree-children';
    childrenContainer.style.maxHeight = 'none';
    renderTree(node.children, childrenContainer, depth + 1);
    wrapper.appendChild(childrenContainer);

    header.addEventListener('click', (e) => {
      e.stopPropagation();
      toggle.classList.toggle('expanded');
      childrenContainer.classList.toggle('collapsed');
      if (childrenContainer.classList.contains('collapsed')) {
        childrenContainer.style.maxHeight = '0';
      } else {
        childrenContainer.style.maxHeight = childrenContainer.scrollHeight + 'px';
      }
    });
  } else if (node.is_dir) {
    const childrenContainer = document.createElement('div');
    childrenContainer.className = 'tree-children collapsed';
    childrenContainer.style.maxHeight = '0';
    wrapper.appendChild(childrenContainer);
  }

  if (!node.is_dir) {
    header.addEventListener('click', (e) => {
      e.stopPropagation();
      loadFile(node.path);
      // close mobile menu after picking a file
      if (isMobileLayout()) closeSidebar();
    });
  }

  return wrapper;
}

// ─── File Loading ──────────────────────────────────────────────────────────
async function loadFile(path) {
  try {
    const file = await api.getFile(path);
    state.currentFile = path;
    setMode('learn', { push: false });

    $$('.tree-node-header.active').forEach((el) => el.classList.remove('active'));
    const activeHeader = document.querySelector(`.tree-node-header[data-path="${path}"]`);
    if (activeHeader) activeHeader.classList.add('active');

    hideAllViews();
    dom.reader.classList.remove('hidden');
    state.rawViewerOpen = false;
    toolItems().forEach((el) => el.classList.remove('active'));

    dom.readerTitle.textContent = file.title;
    dom.readerCategory.textContent = file.category;
    dom.readerCategory.className = `reader-category-badge ${file.category}`;
    dom.readerContent.innerHTML = file.html;

    if (typeof hljs !== 'undefined') {
      document.querySelectorAll('.markdown-body pre code').forEach((block) => {
        hljs.highlightElement(block);
      });
    }

    dom.rawContent.textContent = file.raw;

    const contentEl = document.getElementById('content');
    contentEl.scrollTo({ top: 0, behavior: 'smooth' });

    nav(`?path=${encodeURIComponent(path)}`, { path, mode: 'learn' });
    closeSearch();
  } catch (err) {
    console.error('Failed to load file:', err);
  }
}

// ─── Search ────────────────────────────────────────────────────────────────
let searchTimeout = null;
let searchSeq = 0;

dom.searchInput.addEventListener('input', () => {
  clearTimeout(searchTimeout);
  const query = dom.searchInput.value.trim();
  if (query.length < 2) {
    dom.searchResults.classList.add('hidden');
    return;
  }
  searchTimeout = setTimeout(() => performSearch(query), 200);
});

dom.searchInput.addEventListener('focus', () => {
  if (dom.searchInput.value.trim().length >= 2) {
    dom.searchResults.classList.remove('hidden');
  }
});

dom.searchInput.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeSearch();
  if (e.key === 'Enter') {
    const first = dom.searchResults.querySelector('.search-result-item');
    if (first) first.click();
  }
});

function closeSearch() {
  dom.searchResults.classList.add('hidden');
}

async function performSearch(query) {
  const seq = ++searchSeq;
  try {
    const results = await api.search(query);
    // Ignore stale responses: a newer query may have superseded this one.
    if (seq !== searchSeq) return;
    dom.searchResults.innerHTML = '';
    dom.searchResults.classList.remove('hidden');

    if (results.length === 0) {
      dom.searchResults.innerHTML = '<div class="search-empty">No results found</div>';
      return;
    }

    for (const result of results) {
      const item = document.createElement('div');
      item.className = 'search-result-item';

      const title = document.createElement('div');
      title.className = 'search-result-title';
      title.textContent = result.title;
      item.appendChild(title);

      const path = document.createElement('div');
      path.className = 'search-result-path';
      path.textContent = result.path;
      item.appendChild(path);

      const snippet = document.createElement('div');
      snippet.className = 'search-result-snippet';
      snippet.textContent = result.snippet;
      item.appendChild(snippet);

      item.addEventListener('click', () => {
        loadFile(result.path);
        dom.searchInput.value = '';
        closeSearch();
      });

      dom.searchResults.appendChild(item);
    }
  } catch (err) {
    console.error('Search failed:', err);
  }
}

// ─── Raw Viewer ─────────────────────────────────────────────────────────────
dom.readerRaw.addEventListener('click', () => {
  state.rawViewerOpen = !state.rawViewerOpen;
  dom.rawViewer.classList.toggle('hidden');
});

dom.rawClose.addEventListener('click', () => {
  state.rawViewerOpen = false;
  dom.rawViewer.classList.add('hidden');
});

// ─── Sidebar Toggle / close on outside click ────────────────────────────────
function isMobileLayout() {
  return window.matchMedia('(max-width: 768px)').matches;
}

function syncSidebarUi() {
  if (!dom.sidebar) return;
  dom.sidebar.classList.toggle('collapsed', !state.sidebarOpen);
  if (dom.sidebarToggle) {
    dom.sidebarToggle.setAttribute('aria-expanded', state.sidebarOpen ? 'true' : 'false');
  }
  // Backdrop only when menu is open on small screens
  if (dom.sidebarBackdrop) {
    const show = state.sidebarOpen && isMobileLayout();
    dom.sidebarBackdrop.classList.toggle('hidden', !show);
    dom.sidebarBackdrop.setAttribute('aria-hidden', show ? 'false' : 'true');
  }
  document.body.classList.toggle('sidebar-drawer-open', state.sidebarOpen && isMobileLayout());
}

function openSidebar() {
  state.sidebarOpen = true;
  syncSidebarUi();
}

function closeSidebar() {
  state.sidebarOpen = false;
  syncSidebarUi();
}

function toggleSidebar() {
  state.sidebarOpen = !state.sidebarOpen;
  syncSidebarUi();
}

dom.sidebarToggle.addEventListener('click', (e) => {
  e.stopPropagation();
  toggleSidebar();
});

if (dom.sidebarBackdrop) {
  dom.sidebarBackdrop.addEventListener('click', () => {
    closeSidebar();
  });
}

// Click outside sidebar / toggle / search closes menu + search dropdown
document.addEventListener('click', (e) => {
  // search dropdown
  if (!e.target.closest('.search-container')) {
    closeSearch();
  }

  // sidebar: close when open and click is outside it (and not the hamburger)
  if (
    state.sidebarOpen &&
    !e.target.closest('#sidebar') &&
    !e.target.closest('#sidebar-toggle')
  ) {
    // On desktop keep sidebar open unless user collapsed it intentionally —
    // outside-click close is for the drawer/menu pattern (mobile or when overlay is shown).
    if (isMobileLayout()) {
      closeSidebar();
    }
  }
});

window.addEventListener('resize', () => {
  // keep backdrop state correct when rotating / resizing
  syncSidebarUi();
});

// ─── Collapse All ───────────────────────────────────────────────────────────
dom.collapseAll.addEventListener('click', () => {
  const allChildren = $$('.tree-children');
  const allToggles = $$('.tree-toggle');
  allChildren.forEach((el) => {
    el.classList.add('collapsed');
    el.style.maxHeight = '0';
  });
  allToggles.forEach((el) => el.classList.remove('expanded'));
});

// ─── Tools UI (from registry) ───────────────────────────────────────────────
function buildToolsUI() {
  if (!window.VAULT_TOOLS || !dom.toolsNav || !dom.toolsPanel) return;

  const statusMap = state.toolStatus || {};
  dom.toolsNav.innerHTML = '';
  dom.toolsPanel.innerHTML = '';

  for (const cat of window.VAULT_TOOLS.categories) {
    const section = document.createElement('div');
    section.className = 'tools-cat';
    section.innerHTML = `
      <div class="tools-cat-header">
        <span class="tools-cat-eyebrow">${escapeUi(cat.eyebrow || cat.label)}</span>
        <span class="tools-cat-label">${escapeUi(cat.label)}</span>
      </div>
      <div class="tools-list" data-category="${escapeUi(cat.id)}"></div>`;
    const list = section.querySelector('.tools-list');

    for (const tool of cat.tools) {
      const installed =
        statusMap[tool.id] !== undefined ? statusMap[tool.id] : true;
      const item = document.createElement('div');
      item.className = 'tool-item' + (installed ? '' : ' tool-missing');
      item.dataset.tool = tool.id;
      item.innerHTML = `
        <span class="tool-icon-wrap">${toolIcon(cat.id)}</span>
        <div class="tool-meta">
          <span class="tool-label">${escapeUi(tool.label)}</span>
          <span class="tool-desc">${escapeUi(tool.desc)}${installed ? '' : ' · not installed'}</span>
        </div>
        <svg class="tool-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m9 18 6-6-6-6"/></svg>`;
      item.addEventListener('click', () => {
        switchToTool(tool.id);
        if (isMobileLayout()) closeSidebar();
      });
      list.appendChild(item);

      // Pane
      const pane = document.createElement('div');
      pane.id = tool.id;
      pane.className = 'tool-pane hidden';
      pane.dataset.tool = tool.id;

      const extrasHtml = (tool.extras || [])
        .map(
          (ex) => `
          <label class="tool-field">
            <span class="tool-field-label">${escapeUi(ex.label || ex.name)}</span>
            <input type="${escapeUi(ex.type || 'text')}" data-extra="${escapeUi(ex.name)}"
              placeholder="${escapeUi(ex.placeholder || '')}" autocomplete="off">
          </label>`
        )
        .join('');

      const defaultVal = tool.input.defaultValue
        ? ` value="${escapeUi(tool.input.defaultValue)}"`
        : '';

      const dbBar = tool.dbRefresh
        ? `<div class="tool-db-bar" data-db-bar>
            <div class="tool-db-meta" data-db-meta>Loading DB status…</div>
            <button type="button" class="btn-ghost" data-db-refresh title="Download latest Wordfence vulnerability feed">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-2.6-6.3"/><path d="M21 3v6h-6"/></svg>
              <span>Refresh vulnerability DB</span>
            </button>
          </div>`
        : '';

      pane.innerHTML = `
        <div class="tool-pane-header">
          <div class="tool-pane-badge">${escapeUi(tool.badge || cat.label)}</div>
          <h2>${escapeUi(tool.title)}</h2>
          <p class="tool-pane-desc">${escapeUi(tool.blurb)}</p>
          ${dbBar}
        </div>
        <div class="tool-pane-body">
          <form class="tool-form" data-tool-form="${escapeUi(tool.id)}">
            <label class="tool-field tool-field-primary">
              <span class="tool-field-label">${escapeUi(tool.input.label || tool.input.name)}</span>
              <div class="tool-input-group">
                <div class="tool-input-wrap">
                  <input data-primary="1" type="${escapeUi(tool.input.type || 'text')}"
                    name="${escapeUi(tool.input.name)}"
                    placeholder="${escapeUi(tool.input.placeholder || '')}"
                    autocomplete="off"${defaultVal}>
                </div>
                <button type="submit" class="btn-primary">
                  <span>Run</span>
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M13 6l6 6-6 6"/></svg>
                </button>
              </div>
            </label>
            ${extrasHtml ? `<div class="tool-extras">${extrasHtml}</div>` : ''}
          </form>
          <div class="tool-results-host hidden" data-results></div>
          <div class="tool-spinner hidden" data-spinner>
            <div class="spinner"></div>
            <span>Running ${escapeUi(tool.binary || tool.label)}…</span>
          </div>
        </div>`;

      const form = pane.querySelector('form');
      form.addEventListener('submit', (e) => {
        e.preventDefault();
        const resultsEl = pane.querySelector('[data-results]');
        const spinnerEl = pane.querySelector('[data-spinner]');
        window.VAULT_TOOL_RUNNERS.run(tool, form, resultsEl, spinnerEl);
      });

      if (tool.dbRefresh) {
        wireDbRefresh(pane);
      }

      dom.toolsPanel.appendChild(pane);
    }

    dom.toolsNav.appendChild(section);
  }

  if (dom.countTools) {
    dom.countTools.textContent = String(window.VAULT_TOOLS.count());
  }
}

// ─── Vuln detail page (Wordfence) ───────────────────────────────────────────
function escapeDetail(s) {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

async function openVulnDetail({ id, software_type, slug, detected_version }, { push = true } = {}) {
  if (!id || !dom.vulnDetail) return;

  setMode('tools', { push: false });
  hideAllViews();
  dom.vulnDetail.classList.remove('hidden');
  if (dom.vulnDetailBody) {
    dom.vulnDetailBody.innerHTML =
      '<div class="tool-spinner"><div class="spinner"></div><span>Loading vulnerability…</span></div>';
  }

  const qs = new URLSearchParams({ id });
  if (software_type) qs.set('software_type', software_type);
  if (slug) qs.set('slug', slug);
  if (detected_version) qs.set('detected_version', detected_version);

  try {
    const res = await fetch(`/api/tools/wordpress-vuln-detail?${qs.toString()}`);
    if (!res.ok) throw new Error(await res.text());
    const d = await res.json();
    renderVulnDetail(d);

    if (push) {
      const url = `?tool=wordpress-vuln-scan&vuln=${encodeURIComponent(id)}${
        software_type ? `&st=${encodeURIComponent(software_type)}` : ''
      }${slug ? `&slug=${encodeURIComponent(slug)}` : ''}${
        detected_version ? `&dv=${encodeURIComponent(detected_version)}` : ''
      }`;
      nav(
        url,
        {
          tool: 'wordpress-vuln-scan',
          mode: 'tools',
          vuln: id,
          software_type,
          slug,
          detected_version,
        }
      );
    }

    const contentEl = document.getElementById('content');
    if (contentEl) contentEl.scrollTo({ top: 0, behavior: 'smooth' });
  } catch (err) {
    if (dom.vulnDetailBody) {
      dom.vulnDetailBody.innerHTML = `<div class="tool-error">Failed to load: ${escapeDetail(err.message)}</div>`;
    }
  }
}

function renderVulnDetail(d) {
  const score = d.cvss_score != null ? Number(d.cvss_score).toFixed(1) : '—';
  const rating = d.cvss_rating || 'n/a';
  if (dom.vulnDetailBadge) {
    dom.vulnDetailBadge.textContent = `${rating} · ${score}`;
    dom.vulnDetailBadge.className = `reader-category-badge ${
      String(rating).toLowerCase().includes('critical') || Number(d.cvss_score) >= 9
        ? 'case-studies'
        : String(rating).toLowerCase().includes('high') || Number(d.cvss_score) >= 7
          ? 'case-studies'
          : 'guides'
    }`;
  }
  if (dom.vulnDetailTitle) dom.vulnDetailTitle.textContent = d.title || d.cve || d.id;
  if (dom.vulnDetailSub) {
    const bits = [
      d.cve || null,
      d.software_type && d.slug ? `${d.software_type}:${d.slug}` : null,
      d.detected_version ? `detected v${d.detected_version}` : null,
    ].filter(Boolean);
    dom.vulnDetailSub.textContent = bits.join(' · ');
  }

  if (dom.vulnDetailCveExt) {
    if (d.cve_link) {
      dom.vulnDetailCveExt.href = d.cve_link;
      dom.vulnDetailCveExt.classList.remove('hidden');
      dom.vulnDetailCveExt.textContent = d.cve ? `Open ${d.cve} ↗` : 'Open CVE ↗';
    } else {
      dom.vulnDetailCveExt.classList.add('hidden');
    }
  }

  const row = (k, v) =>
    v == null || v === '' || (Array.isArray(v) && !v.length)
      ? ''
      : `<div class="vd-row"><span class="vd-k">${escapeDetail(k)}</span><span class="vd-v">${v}</span></div>`;

  const softwareBlocks = (d.record && d.record.software) || [];
  const softwareHtml = softwareBlocks.length
    ? `<div class="vd-section"><h3>Affected software (full record)</h3>
        ${softwareBlocks
          .map((s) => {
            const av = s.affected_versions
              ? Object.keys(s.affected_versions).join(', ')
              : '—';
            const pv = (s.patched_versions || []).join(', ') || 'none';
            return `<div class="vd-soft-card">
              <div class="finding-name">${escapeDetail(s.type)} · ${escapeDetail(s.name || s.slug)} <code>${escapeDetail(s.slug)}</code></div>
              <div class="finding-meta">Affected: <code>${escapeDetail(av)}</code></div>
              <div class="finding-meta">Patched: ${s.patched ? 'Yes' : 'No'} · ${escapeDetail(pv)}</div>
              ${s.remediation ? `<div class="vuln-fix"><strong>Remediation:</strong> ${escapeDetail(s.remediation)}</div>` : ''}
            </div>`;
          })
          .join('')}</div>`
    : '';

  const refs = d.references || [];
  const refsHtml = refs.length
    ? `<div class="vd-section"><h3>References</h3><ul class="vd-links">${refs
        .map(
          (u) =>
            `<li><a href="${escapeDetail(u)}" target="_blank" rel="noopener">${escapeDetail(u)}</a></li>`
        )
        .join('')}</ul></div>`
    : '';

  const cveExt = d.cve_link
    ? `<div class="vd-section"><h3>External CVE</h3>
        <p><a class="vd-ext-link" href="${escapeDetail(d.cve_link)}" target="_blank" rel="noopener">${escapeDetail(d.cve_link)}</a></p>
       </div>`
    : '';

  const rawJson = d.record
    ? `<details class="tool-raw-details vd-raw"><summary>Full Wordfence JSON record</summary>
        <pre class="tool-raw-pre">${escapeDetail(JSON.stringify(d.record, null, 2))}</pre></details>`
    : '';

  const cweLabel =
    d.cwe_id != null
      ? `CWE-${d.cwe_id}${d.cwe ? ': ' + d.cwe : ''}`
      : d.cwe || '—';

  if (dom.vulnDetailBody) {
    dom.vulnDetailBody.innerHTML = `
      <div class="vd-section">
        <h3>Overview</h3>
        <div class="vd-grid">
          ${row('CVE', escapeDetail(d.cve || '—'))}
          ${row('CVSS', `${escapeDetail(rating)} · <strong>${escapeDetail(score)}</strong>`)}
          ${row('Vector', d.cvss_vector ? `<code>${escapeDetail(d.cvss_vector)}</code>` : '—')}
          ${row('CWE', escapeDetail(cweLabel))}
          ${row('Published', escapeDetail(d.published || '—'))}
          ${row('Updated', escapeDetail(d.updated || '—'))}
          ${row('Researchers', escapeDetail((d.researchers || []).join(', ') || '—'))}
          ${row('Record ID', `<code>${escapeDetail(d.id)}</code>`)}
        </div>
      </div>
      <div class="vd-section">
        <h3>Scan match</h3>
        <div class="vd-grid">
          ${row('Type', escapeDetail(d.software_type || '—'))}
          ${row('Slug', escapeDetail(d.slug || '—'))}
          ${row('Name', escapeDetail(d.name || '—'))}
          ${row('Detected version', d.detected_version ? `<code>v${escapeDetail(d.detected_version)}</code>` : '—')}
          ${row('Affected ranges', escapeDetail((d.affected_versions || []).join(', ') || '—'))}
          ${row('Patched', d.patched == null ? '—' : d.patched ? 'Yes' : 'No')}
          ${row('Patched versions', escapeDetail((d.patched_versions || []).join(', ') || 'none'))}
        </div>
        ${d.remediation ? `<div class="vuln-fix"><strong>Remediation:</strong> ${escapeDetail(d.remediation)}</div>` : ''}
      </div>
      <div class="vd-section">
        <h3>Description</h3>
        <p class="vuln-desc">${escapeDetail(d.description || '—')}</p>
        ${d.cwe_description ? `<p class="vuln-cwe-desc"><strong>CWE detail:</strong> ${escapeDetail(d.cwe_description)}</p>` : ''}
      </div>
      ${softwareHtml}
      ${refsHtml}
      ${cveExt}
      ${d.copyright ? `<p class="finding-meta vuln-copy">${escapeDetail(d.copyright)}</p>` : ''}
      ${rawJson}
    `;
  }
}

function closeVulnDetailToScan({ push = true } = {}) {
  hideAllViews();
  setMode('tools', { push: false });
  if (dom.toolsPanel) dom.toolsPanel.classList.remove('hidden');
  // ensure vuln scan tool pane is visible
  switchToTool('wordpress-vuln-scan', { push: false });
  if (push) {
    nav('?tool=wordpress-vuln-scan', { tool: 'wordpress-vuln-scan', mode: 'tools' });
  }
}

// Click short CVE cards inside tools panel
if (dom.toolsPanel) {
  dom.toolsPanel.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-vuln-open]');
    if (!btn) return;
    e.preventDefault();
    openVulnDetail({
      id: btn.dataset.vulnId,
      software_type: btn.dataset.softwareType,
      slug: btn.dataset.slug,
      detected_version: btn.dataset.detectedVersion,
    });
  });
}

if (dom.vulnDetailBack) {
  dom.vulnDetailBack.addEventListener('click', () => closeVulnDetailToScan({ push: true }));
}

async function wireDbRefresh(pane) {
  const metaEl = pane.querySelector('[data-db-meta]');
  const btn = pane.querySelector('[data-db-refresh]');
  if (!metaEl || !btn) return;

  async function loadStatus() {
    try {
      const res = await fetch('/api/tools/wordpress-vuln-db/status');
      const d = await res.json();
      if (!d.present) {
        metaEl.innerHTML =
          '<span class="db-badge db-miss">DB missing</span> Click refresh to download Wordfence feed';
      } else {
        const when = d.updated_at ? new Date(d.updated_at).toLocaleString() : 'unknown';
        const count = d.count != null ? d.count.toLocaleString() : '?';
        const mb = d.bytes != null ? (d.bytes / (1024 * 1024)).toFixed(1) + ' MB' : '';
        metaEl.innerHTML = `<span class="db-badge db-ok">DB ready</span> ${count} vulns · ${mb} · updated ${when}`;
      }
    } catch {
      metaEl.textContent = 'Could not load DB status';
    }
  }

  btn.addEventListener('click', async () => {
    btn.disabled = true;
    metaEl.innerHTML = '<span class="db-badge">Refreshing…</span> Downloading full Wordfence feed (may take 1–2 min)';
    try {
      const res = await fetch('/api/tools/wordpress-vuln-db/refresh');
      const d = await res.json();
      if (!d.ok) {
        metaEl.innerHTML = `<span class="db-badge db-miss">Refresh failed</span> ${escapeUi(d.error || 'unknown error')}`;
      } else {
        metaEl.innerHTML = `<span class="db-badge db-ok">Updated</span> ${Number(d.count).toLocaleString()} vulns in ${(d.duration_ms / 1000).toFixed(1)}s`;
        setTimeout(loadStatus, 800);
      }
    } catch (e) {
      metaEl.innerHTML = `<span class="db-badge db-miss">Error</span> ${escapeUi(e.message)}`;
    } finally {
      btn.disabled = false;
    }
  });

  loadStatus();
}

function toolIcon(categoryId) {
  if (categoryId === 'wordpress') {
    return `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 3 2.5 15 0 18M12 3c-2.5 3-2.5 15 0 18"/></svg>`;
  }
  if (categoryId === 'local') {
    return `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7h6l2 2h10v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></svg>`;
  }
  return `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="11" cy="11" r="7"/><path d="m20 20-3-3"/></svg>`;
}

function escapeUi(s) {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function switchToTool(toolId, { push = true } = {}) {
  setMode('tools', { push: false });

  toolItems().forEach((el) => el.classList.remove('active'));
  const activeTool = document.querySelector(`.tool-item[data-tool="${toolId}"]`);
  if (activeTool) activeTool.classList.add('active');

  hideAllViews();
  state.rawViewerOpen = false;
  state.currentFile = null;
  dom.toolsPanel.classList.remove('hidden');

  $$('.tool-pane').forEach((el) => el.classList.add('hidden'));
  const pane = document.getElementById(toolId);
  if (pane) pane.classList.remove('hidden');

  if (push) {
    nav(`?tool=${toolId}`, { tool: toolId, mode: 'tools' });
  }

  closeSearch();
}

async function loadToolStatus() {
  try {
    const res = await fetch('/api/tools/status');
    const data = await res.json();
    state.toolStatus = {};
    for (const t of data.tools || []) {
      state.toolStatus[t.id] = t.installed;
    }
  } catch {
    state.toolStatus = {};
  }
}

// ─── Keyboard Shortcuts ────────────────────────────────────────────────────
document.addEventListener('keydown', (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault();
    dom.searchInput.focus();
  }
  if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
    e.preventDefault();
    dom.sidebarToggle.click();
  }
  if (e.key === 'Escape') {
    if (state.rawViewerOpen) dom.rawClose.click();
    closeSearch();
    dom.searchInput.blur();
    if (state.sidebarOpen && isMobileLayout()) closeSidebar();
  }
});

// ─── History Navigation ────────────────────────────────────────────────────
window.addEventListener('popstate', (e) => {
  if (e.state && e.state.vuln) {
    openVulnDetail(
      {
        id: e.state.vuln,
        software_type: e.state.software_type,
        slug: e.state.slug,
        detected_version: e.state.detected_version,
      },
      { push: false }
    );
  } else if (e.state && e.state.path) {
    loadFile(e.state.path);
  } else if (e.state && e.state.tool) {
    switchToTool(e.state.tool, { push: false });
  } else if (e.state && e.state.mode === 'learn') {
    enterLearnPath({ push: false });
  } else if (e.state && e.state.mode === 'tools') {
    enterToolsPath({ push: false });
  } else {
    // Treat as home without double push
    state.currentFile = null;
    state.rawViewerOpen = false;
    setMode('home', { push: false });
    hideAllViews();
    dom.welcome.classList.remove('hidden');
    toolItems().forEach((el) => el.classList.remove('active'));
  }
});

// ─── Init ───────────────────────────────────────────────────────────────────
async function init() {
  setMode('home', { push: false });
  window.addEventListener('resize', updateModeIndicator);
  // Start with menu closed on mobile so content is usable
  if (isMobileLayout()) {
    state.sidebarOpen = false;
  }
  syncSidebarUi();

  // Build tools UI from registry (works even if status API fails)
  await loadToolStatus();
  buildToolsUI();

  try {
    const stats = await api.getStats();
    dom.statsBadge.textContent = `${stats.total_files} files`;
    dom.countGuides.textContent = stats.guides;
    dom.countReferences.textContent = stats.references;
    dom.countCaseStudies.textContent = stats.case_studies;

    state.tree = await api.getTree();
    renderTree(state.tree, dom.treeContainer);

    const params = new URLSearchParams(window.location.search);
    const filePath = params.get('path');
    const toolId = params.get('tool');
    const mode = params.get('mode');
    const vulnId = params.get('vuln');

    if (filePath) {
      await loadFile(filePath);
    } else if (vulnId) {
      await openVulnDetail(
        {
          id: vulnId,
          software_type: params.get('st') || undefined,
          slug: params.get('slug') || undefined,
          detected_version: params.get('dv') || undefined,
        },
        { push: false }
      );
    } else if (toolId) {
      switchToTool(toolId, { push: false });
    } else if (mode === 'tools') {
      enterToolsPath({ push: false });
    } else if (mode === 'learn') {
      enterLearnPath({ push: false });
    }

    // Deep-link handling is done; subsequent navigation must push new entries.
    initialNav = false;
  } catch (err) {
    console.error('Init failed:', err);
    dom.treeContainer.innerHTML = `
      <div class="tree-loading">
        <span style="color: var(--red);">⚠️ Failed to load library</span>
        <span style="font-size: 12px; color: var(--text-muted);">Make sure the server is running</span>
      </div>`;
  }
}

init();
