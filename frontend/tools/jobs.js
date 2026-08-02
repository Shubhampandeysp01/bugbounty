/**
 * Job Manager frontend — single source of truth for background tool runs.
 *
 * - `VaultJobs.api`   → REST wrappers over /api/jobs
 * - `VaultJobs.on`    → typed-event subscription over ONE shared EventSource
 * - `VaultJobs.submit`→ create a job + return its view
 * - `VaultJobs.state` → in-memory cache (id → JobView) + running count
 * - `JobCenter`       → topbar slide-over panel (running + recent, session-only)
 * - `VaultJobs.toast` → completion notifications
 *
 * Loaded after registry.js, before runners.js.
 */
window.VaultJobs = (() => {
  const TERMINAL = new Set(['succeeded', 'failed', 'cancelled']);

  // ─── REST API ──────────────────────────────────────────────────────────────
  const api = {
    async list() {
      const res = await fetch('/api/jobs');
      if (!res.ok) throw new Error(`GET /api/jobs → ${res.status}`);
      return res.json();
    },
    async get(id) {
      const res = await fetch(`/api/jobs/${encodeURIComponent(id)}`);
      if (!res.ok) throw new Error(`GET /api/jobs/${id} → ${res.status}`);
      return res.json();
    },
    async create(tool, params) {
      const res = await fetch('/api/jobs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tool, params }),
      });
      if (!res.ok) {
        const body = await res.text().catch(() => '');
        throw new Error(body || `POST /api/jobs → ${res.status}`);
      }
      return res.json();
    },
    async result(id) {
      const res = await fetch(`/api/jobs/${encodeURIComponent(id)}/result`);
      if (!res.ok) {
        const body = await res.text().catch(() => '');
        throw new Error(body || `GET /api/jobs/${id}/result → ${res.status}`);
      }
      return res.json();
    },
    async logs(id) {
      const res = await fetch(`/api/jobs/${encodeURIComponent(id)}/logs`);
      if (!res.ok) throw new Error(`GET /api/jobs/${id}/logs → ${res.status}`);
      return res.json();
    },
    async cancel(id) {
      const res = await fetch(`/api/jobs/${encodeURIComponent(id)}/cancel`, {
        method: 'POST',
      });
      if (!res.ok) throw new Error(`POST /api/jobs/${id}/cancel → ${res.status}`);
      return res.json();
    },
  };

  // ─── Typed event bus (one EventSource for the whole app) ──────────────────
  const listeners = new Map(); // type -> Set<fn(evt)>
  let es = null;
  let connected = false;

  function on(type, fn) {
    if (!listeners.has(type)) listeners.set(type, new Set());
    listeners.get(type).add(fn);
    return () => listeners.get(type).delete(fn);
  }

  function emit(type, evt) {
    const set = listeners.get(type);
    if (!set) return;
    for (const fn of [...set]) {
      try {
        fn(evt);
      } catch (err) {
        console.error('[VaultJobs] handler error', err);
      }
    }
  }

  function handle(type, data) {
    let evt;
    try {
      evt = JSON.parse(data);
    } catch {
      evt = {};
    }
    evt._type = type;
    if (evt.job) state.jobs.set(evt.job.id, evt.job);
    if (type === 'job.resync') {
      refresh();
      return;
    }
    emit(type, evt);
    emit('job.*', evt);
    updateBadge();
    JobCenter.touch(evt);
  }

  function connect() {
    if (es) return;
    es = new EventSource('/api/jobs/events');
    [
      'job.created',
      'job.started',
      'job.progress',
      'job.log',
      'job.completed',
      'job.failed',
      'job.cancelled',
      'job.resync',
    ].forEach((t) => es.addEventListener(t, (e) => handle(t, e.data)));
    es.onopen = () => {
      connected = true;
      refresh();
    };
    es.onerror = () => {
      connected = false;
      emit('job.offline', {});
    };
  }

  // ─── State / cache ─────────────────────────────────────────────────────────
  const state = {
    jobs: new Map(), // id -> JobView
    get running() {
      let n = 0;
      for (const j of state.jobs.values()) {
        if (!TERMINAL.has(j.status)) n += 1;
      }
      return n;
    },
    runningCount() {
      return state.running;
    },
    byTool(toolId) {
      return [...state.jobs.values()].filter((j) => j.tool === toolId);
    },
    hasRunningTool(toolId) {
      return state.byTool(toolId).some((j) => !TERMINAL.has(j.status));
    },
  };

  async function refresh() {
    try {
      const list = await api.list();
      state.jobs.clear();
      for (const j of list) state.jobs.set(j.id, j);
      updateBadge();
      emit('job.list', list);
      JobCenter.refresh();
    } catch {
      // server down / offline — keep whatever we have
    }
  }

  // ─── Public API ────────────────────────────────────────────────────────────
  async function submit(tool, params) {
    connect();
    return api.create(tool, params);
  }

  function updateBadge() {
    document.querySelectorAll('[data-jobs-badge]').forEach((el) => {
      const n = state.running;
      el.textContent = n > 99 ? '99+' : String(n);
      el.classList.toggle('hidden', n === 0);
      el.classList.toggle('is-active', n > 0);
    });
  }

  // ─── Toasts / notifications ────────────────────────────────────────────────
  const toasts = new Set();

  function toast(title, { type = 'info', body = '', timeout = 5000 } = {}) {
    if (!document.getElementById('toasts')) return;
    const el = document.createElement('div');
    el.className = `toast toast-${type}`;
    const icon =
      type === 'ok' ? '✓' : type === 'err' ? '✕' : type === 'warn' ? '!' : '•';
    el.innerHTML = `
      <span class="toast-icon">${icon}</span>
      <div class="toast-body">
        <div class="toast-title">${escapeHtml(title)}</div>
        ${body ? `<div class="toast-msg">${escapeHtml(body)}</div>` : ''}
      </div>
      <button type="button" class="toast-close" aria-label="Dismiss">✕</button>`;
    el.querySelector('.toast-close').addEventListener('click', () => dismiss(el));
    document.getElementById('toasts').appendChild(el);
    toasts.add(el);
    if (timeout) setTimeout(() => dismiss(el), timeout);
    return el;
  }

  function dismiss(el) {
    if (!el || !toasts.has(el)) return;
    toasts.delete(el);
    el.classList.add('toast-out');
    setTimeout(() => el.remove(), 240);
  }

  // ─── Job Center slide-over panel ───────────────────────────────────────────
  const JobCenter = {
    opened: false,
    expandedId: null,

    open() {
      this.opened = true;
      const panel = $job('#jobs-panel');
      const backdrop = $job('#jobs-backdrop');
      if (panel) panel.classList.remove('hidden');
      if (backdrop) backdrop.classList.remove('hidden');
      document.body.classList.add('jobs-open');
      this.refresh();
      const firstRow = panel && panel.querySelector('.job-row');
      if (firstRow) firstRow.scrollIntoView({ block: 'nearest' });
    },

    close() {
      this.opened = false;
      const panel = $job('#jobs-panel');
      const backdrop = $job('#jobs-backdrop');
      if (panel) panel.classList.add('hidden');
      if (backdrop) backdrop.classList.add('hidden');
      document.body.classList.remove('jobs-open');
    },

    toggle() {
      this.opened ? this.close() : this.open();
    },

    // Re-render when the panel is visible (cheap; fine to call on every event).
    touch(evt) {
      if (!this.opened) return;
      if (evt._type === 'job.log' || evt._type === 'job.progress') {
        this.updateRow(evt.job_id);
      } else {
        this.refresh();
      }
    },

    refresh() {
      if (!this.opened) return;
      const list = [...state.jobs.values()].sort(
        (a, b) =>
          String(b.started_at || b.id).localeCompare(String(a.started_at || a.id))
      );
      this.render(list);
    },

    render(list) {
      const host = $('#jobs-list');
      if (!host) return;

      const running = list.filter((j) => !TERMINAL.has(j.status));
      const recent = list.filter((j) => TERMINAL.has(j.status)).slice(0, 50);

      const runningHtml =
        running.length === 0
          ? '<div class="jobs-empty">No jobs running right now.</div>'
          : running.map((j) => this.row(j)).join('');

      const recentHtml =
        recent.length === 0
          ? ''
          : `<div class="jobs-section-title">Recent</div>` +
            recent.map((j) => this.row(j)).join('') +
            `<div class="jobs-actions">
              <button type="button" class="btn-ghost btn-xs" data-jobs-clear>Clear finished</button>
            </div>`;

      const runningTitle = `<div class="jobs-section-title">Running · ${running.length}</div>`;

      host.innerHTML =
        (running.length ? runningTitle : '') +
        runningHtml +
        recentHtml;

      const countEl = $('#jobs-running-count');
      if (countEl) {
        countEl.textContent = String(running.length);
        countEl.classList.toggle('hidden', running.length === 0);
      }

      host.querySelectorAll('[data-job-row]').forEach((row) => {
        row.addEventListener('click', (e) => {
          if (e.target.closest('[data-job-cancel]')) return;
          if (e.target.closest('[data-job-copy]')) return;
          this.toggleRow(row.dataset.jobRow);
        });
      });
      host.querySelectorAll('[data-job-cancel]').forEach((btn) => {
        btn.addEventListener('click', async (e) => {
          e.stopPropagation();
          btn.disabled = true;
          btn.textContent = 'Cancelling…';
          try {
            await api.cancel(btn.dataset.jobCancel);
          } catch {
            btn.disabled = false;
            btn.textContent = 'Cancel';
          }
        });
      });
      host.querySelectorAll('[data-job-copy]').forEach((btn) => {
        btn.addEventListener('click', (e) => {
          e.stopPropagation();
          const id = btn.dataset.jobCopy;
          navigator.clipboard?.writeText(id).catch(() => {});
          btn.textContent = 'copied';
          setTimeout(() => (btn.textContent = 'copy id'), 900);
        });
      });
      const clearBtn = host.querySelector('[data-jobs-clear]');
      if (clearBtn) {
        clearBtn.addEventListener('click', () => {
          for (const [id, j] of state.jobs) {
            if (TERMINAL.has(j.status)) state.jobs.delete(id);
          }
          this.refresh();
        });
      }
    },

    row(j) {
      const term = TERMINAL.has(j.status);
      const expanded = this.expandedId === j.id;
      const label = escapeHtml(j.label);
      const id = escapeHtml(j.id);
      const tool = escapeHtml(j.tool);
      const dur = j.duration_ms != null ? fmtMs(j.duration_ms) : '—';
      const params = summarizeParams(j.params);

      const statusIcon =
        j.status === 'succeeded'
          ? 'status-ok'
          : j.status === 'failed'
            ? 'status-err'
            : j.status === 'cancelled'
              ? 'status-warn'
              : 'status-run';
      const statusLabel = j.status;

      const cancelBtn = term
        ? ''
        : `<button type="button" class="job-cancel-btn" data-job-cancel="${id}"
             title="Cancel job">Cancel</button>`;
      const copyBtn = `<button type="button" class="job-copy-btn" data-job-copy="${id}"
            title="Copy job id">copy id</button>`;

      return `
        <div class="job-row ${expanded ? 'expanded' : ''}" data-job-row="${id}">
          <div class="job-row-head">
            <span class="job-status-dot ${statusIcon}" title="${escapeHtml(j.status)}"></span>
            <span class="job-row-label" title="${label}">${label}</span>
            <span class="job-row-id mono">${id}</span>
            <span class="job-row-dur mono">${dur}</span>
            ${cancelBtn}
          </div>
          <div class="job-row-meta">
            <span class="mono">${tool}</span>
            ${params ? `<span>${escapeHtml(params)}</span>` : ''}
            ${j.error ? `<span class="job-row-error">${escapeHtml(j.error)}</span>` : ''}
          </div>
          <div class="job-row-foot">
            <span class="mono job-row-status">${statusLabel}</span>
            <span>${copyBtn}</span>
          </div>
          <pre class="job-row-log ${expanded ? '' : 'hidden'}" data-job-log="${id}"></pre>
        </div>`;
    },

    async toggleRow(id) {
      this.expandedId = this.expandedId === id ? null : id;
      this.refresh();
      if (this.expandedId === id) {
        try {
          const lines = await api.logs(id);
          const log = document.querySelector(`[data-job-log="${id}"]`);
          if (log) {
            log.textContent = lines.length ? lines.join('\n') : '(no output)';
          }
        } catch {
          const log = document.querySelector(`[data-job-log="${id}"]`);
          if (log) log.textContent = '(logs unavailable)';
        }
      }
    },

    updateRow(id) {
      if (!this.opened || this.expandedId !== id) return;
      const log = document.querySelector(`[data-job-log="${id}"]`);
      if (!log) return;
      api
        .logs(id)
        .then((lines) => {
          if (document.body.contains(log)) {
            log.textContent = lines.join('\n');
            log.scrollTop = log.scrollHeight;
          }
        })
        .catch(() => {});
    },
  };

  return {
    api,
    on,
    state,
    submit,
    refresh,
    JobCenter,
    toast,
    dismiss,
    isTerminal: (s) => TERMINAL.has(s),
  };
})();

// ─── Small helpers (local to this file) ───────────────────────────────────────
function $job(sel) {
  return document.querySelector(sel);
}

function escapeHtml(s) {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function fmtMs(ms) {
  if (ms == null) return '—';
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${Math.round(s % 60)}s`;
}

function summarizeParams(params) {
  if (!params) return '';
  const keys = Object.keys(params);
  if (!keys.length) return '';
  const label = params.label || params.name || params.cve || params.q || params.url;
  if (label) return label;
  return keys.slice(0, 2).map((k) => `${k}=${params[k]}`).join(' ');
}
