// ─── State ─────────────────────────────────────────────────────────────────
const state = {
  tree: [],
  currentFile: null,
  sidebarOpen: true,
  rawViewerOpen: false,
};

// ─── DOM References ─────────────────────────────────────────────────────────
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const dom = {
  sidebar: $('#sidebar'),
  sidebarToggle: $('#sidebar-toggle'),
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
  welcomeCards: $$('.welcome-card'),
  toolsPanel: $('#tools-panel'),
  toolItems: $$('.tool-item'),
  wpCheckUrl: $('#wp-check-url'),
  wpCheckBtn: $('#wp-check-btn'),
  wpCheckResults: $('#wp-check-results'),
  wpCheckSpinner: $('#wp-check-spinner'),
  wpVersion: $('#wp-version'),
  wpSource: $('#wp-source'),
  wpRestApi: $('#wp-rest-api'),
  wpXmlrpc: $('#wp-xmlrpc'),
  wpReadme: $('#wp-readme'),
  wpServer: $('#wp-server'),
  wpError: $('#wp-error'),
};

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

  // Toggle arrow
  const toggle = document.createElement('span');
  toggle.className = 'tree-toggle';
  if (node.is_dir) {
    toggle.textContent = '▶';
    toggle.classList.add('expanded');
  } else {
    toggle.classList.add('hidden');
  }
  header.appendChild(toggle);

  // Icon
  const icon = document.createElement('span');
  icon.className = 'tree-icon';
  if (node.is_dir) {
    icon.textContent = node.name === 'guides' ? '📘' :
                       node.name === 'references' ? '📚' :
                       node.name === 'case-studies' ? '🔬' : '📁';
  } else {
    icon.textContent = '📄';
  }
  header.appendChild(icon);

  // Label
  const label = document.createElement('span');
  label.className = `tree-label${node.is_dir ? '' : ' file'}`;
  label.textContent = node.name;
  header.appendChild(label);

  wrapper.appendChild(header);

  // Children
  if (node.is_dir && node.children.length > 0) {
    const childrenContainer = document.createElement('div');
    childrenContainer.className = 'tree-children';
    childrenContainer.style.maxHeight = 'none';
    renderTree(node.children, childrenContainer, depth + 1);
    wrapper.appendChild(childrenContainer);

    // Toggle click
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

  // File click
  if (!node.is_dir) {
    header.addEventListener('click', (e) => {
      e.stopPropagation();
      loadFile(node.path);
    });
  }

  return wrapper;
}

// ─── File Loading ──────────────────────────────────────────────────────────
async function loadFile(path) {
  try {
    const file = await api.getFile(path);
    state.currentFile = path;

    // Update active state in tree
    $$('.tree-node-header.active').forEach(el => el.classList.remove('active'));
    const activeHeader = document.querySelector(`.tree-node-header[data-path="${path}"]`);
    if (activeHeader) activeHeader.classList.add('active');

    // Show reader, hide welcome
    dom.welcome.classList.add('hidden');
    dom.reader.classList.remove('hidden');
    dom.rawViewer.classList.add('hidden');
    state.rawViewerOpen = false;

    // Set content
    dom.readerTitle.textContent = file.title;
    dom.readerCategory.textContent = file.category;
    dom.readerCategory.className = `reader-category-badge ${file.category}`;
    dom.readerContent.innerHTML = file.html;

    // Highlight code blocks
    if (typeof hljs !== 'undefined') {
      document.querySelectorAll('.markdown-body pre code').forEach((block) => {
        hljs.highlightElement(block);
      });
    }

    // Store raw for raw viewer
    dom.rawContent.textContent = file.raw;

    // Scroll to top of content area smoothly
    const contentEl = document.getElementById('content');
    contentEl.scrollTo({ top: 0, behavior: 'smooth' });

    // Update URL
    history.pushState({ path }, '', `?path=${encodeURIComponent(path)}`);

    // Close search
    closeSearch();
  } catch (err) {
    console.error('Failed to load file:', err);
  }
}

// ─── Search ────────────────────────────────────────────────────────────────
let searchTimeout = null;

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

document.addEventListener('click', (e) => {
  if (!e.target.closest('.search-container')) {
    closeSearch();
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
  try {
    const results = await api.search(query);
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

// ─── Sidebar Toggle ─────────────────────────────────────────────────────────
dom.sidebarToggle.addEventListener('click', () => {
  state.sidebarOpen = !state.sidebarOpen;
  dom.sidebar.classList.toggle('collapsed');
});

// ─── Collapse All ───────────────────────────────────────────────────────────
dom.collapseAll.addEventListener('click', () => {
  const allChildren = $$('.tree-children');
  const allToggles = $$('.tree-toggle');
  allChildren.forEach(el => {
    el.classList.add('collapsed');
    el.style.maxHeight = '0';
  });
  allToggles.forEach(el => el.classList.remove('expanded'));
});

// ─── Welcome Cards ─────────────────────────────────────────────────────────
dom.welcomeCards.forEach(card => {
  card.addEventListener('click', () => {
    const category = card.dataset.category;
    // Find the first file in that category
    const firstFile = findFirstFileInCategory(state.tree, category);
    if (firstFile) loadFile(firstFile);
  });
});

function findFirstFileInCategory(tree, category) {
  for (const node of tree) {
    if (node.name === category && node.is_dir && node.children.length > 0) {
      return findFirstFile(node.children);
    }
  }
  return null;
}

function findFirstFile(nodes) {
  for (const node of nodes) {
    if (!node.is_dir) return node.path;
    if (node.children.length > 0) {
      const found = findFirstFile(node.children);
      if (found) return found;
    }
  }
  return null;
}

// ─── Tools ──────────────────────────────────────────────────────────────────

function switchToTool(toolId) {
  // Update active state in tools list
  dom.toolItems.forEach(el => el.classList.remove('active'));
  const activeTool = document.querySelector(`.tool-item[data-tool="${toolId}"]`);
  if (activeTool) activeTool.classList.add('active');

  // Hide learning views, show tools panel
  dom.welcome.classList.add('hidden');
  dom.reader.classList.add('hidden');
  dom.rawViewer.classList.add('hidden');
  state.rawViewerOpen = false;
  dom.toolsPanel.classList.remove('hidden');

  // Show the specific tool pane
  $$('.tool-pane').forEach(el => el.classList.add('hidden'));
  const pane = document.getElementById(toolId);
  if (pane) pane.classList.remove('hidden');

  // Update URL
  history.pushState({ tool: toolId }, '', `?tool=${toolId}`);

  // Close search
  closeSearch();
}

// Tool item click handlers
dom.toolItems.forEach(item => {
  item.addEventListener('click', () => {
    switchToTool(item.dataset.tool);
  });
});

// WordPress Version Check
async function checkWordPressVersion() {
  const url = dom.wpCheckUrl.value.trim();
  if (!url) {
    dom.wpCheckUrl.focus();
    return;
  }

  // Add https:// if missing
  let targetUrl = url;
  if (!targetUrl.startsWith('http://') && !targetUrl.startsWith('https://')) {
    targetUrl = 'https://' + targetUrl;
  }

  dom.wpCheckBtn.disabled = true;
  dom.wpCheckResults.classList.add('hidden');
  dom.wpError.classList.add('hidden');
  dom.wpCheckSpinner.classList.remove('hidden');

  try {
    const res = await fetch(`/api/tools/wordpress-check?url=${encodeURIComponent(targetUrl)}`);
    const data = await res.json();

    dom.wpCheckSpinner.classList.add('hidden');
    dom.wpCheckResults.classList.remove('hidden');

    if (data.error) {
      dom.wpError.textContent = 'Error: ' + data.error;
      dom.wpError.classList.remove('hidden');
    }

    // Version
    dom.wpVersion.textContent = data.version || 'Not detected';
    dom.wpVersion.style.color = data.version ? 'var(--accent)' : 'var(--text-muted)';

    // Source
    const sourceLabels = {
      'generator_meta_tag': 'Meta Generator Tag',
      'wp_json': 'REST API (/wp-json/)',
      'readme_html': 'readme.html',
    };
    dom.wpSource.textContent = data.version_source ? (sourceLabels[data.version_source] || data.version_source) : '—';

    // REST API
    dom.wpRestApi.textContent = data.rest_api_available ? '✅ Available' : '❌ Not found';
    dom.wpRestApi.style.color = data.rest_api_available ? 'var(--accent)' : 'var(--text-muted)';

    // XML-RPC
    dom.wpXmlrpc.textContent = data.xmlrpc_available ? '⚠️ Enabled' : '✅ Disabled / Blocked';
    dom.wpXmlrpc.style.color = data.xmlrpc_available ? 'var(--yellow)' : 'var(--accent)';

    // readme.html
    dom.wpReadme.textContent = data.readme_accessible ? '⚠️ Accessible' : '✅ Blocked / Hidden';
    dom.wpReadme.style.color = data.readme_accessible ? 'var(--yellow)' : 'var(--accent)';

    // Server
    dom.wpServer.textContent = data.headers?.server || '—';
  } catch (err) {
    dom.wpCheckSpinner.classList.add('hidden');
    dom.wpError.textContent = 'Failed to check: ' + err.message;
    dom.wpError.classList.remove('hidden');
  } finally {
    dom.wpCheckBtn.disabled = false;
  }
}

dom.wpCheckBtn.addEventListener('click', checkWordPressVersion);
dom.wpCheckUrl.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') checkWordPressVersion();
});

// ─── Keyboard Shortcuts ────────────────────────────────────────────────────
document.addEventListener('keydown', (e) => {
  // Ctrl/Cmd + K → focus search
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault();
    dom.searchInput.focus();
  }
  // Ctrl/Cmd + B → toggle sidebar
  if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
    e.preventDefault();
    dom.sidebarToggle.click();
  }
  // Escape → close raw viewer / search
  if (e.key === 'Escape') {
    if (state.rawViewerOpen) dom.rawClose.click();
    closeSearch();
    dom.searchInput.blur();
  }
});

// ─── History Navigation ────────────────────────────────────────────────────
window.addEventListener('popstate', (e) => {
  if (e.state && e.state.path) {
    loadFile(e.state.path);
  } else if (e.state && e.state.tool) {
    switchToTool(e.state.tool);
  } else {
    dom.welcome.classList.remove('hidden');
    dom.reader.classList.add('hidden');
    dom.rawViewer.classList.add('hidden');
    dom.toolsPanel.classList.add('hidden');
    dom.toolItems.forEach(el => el.classList.remove('active'));
  }
});

// ─── Init ───────────────────────────────────────────────────────────────────
async function init() {
  try {
    // Load stats
    const stats = await api.getStats();
    dom.statsBadge.textContent = `${stats.total_files} files`;
    dom.countGuides.textContent = stats.guides;
    dom.countReferences.textContent = stats.references;
    dom.countCaseStudies.textContent = stats.case_studies;

    // Load tree
    state.tree = await api.getTree();
    renderTree(state.tree, dom.treeContainer);

    // Check URL for direct file load or tool
    const params = new URLSearchParams(window.location.search);
    const filePath = params.get('path');
    const toolId = params.get('tool');
    if (filePath) {
      await loadFile(filePath);
    } else if (toolId) {
      switchToTool(toolId);
    }
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
