/**
 * Tool runners — one function family per tool.
 * Keep logic here so deleting a tool = drop its render case + registry entry.
 */
window.VAULT_TOOL_RUNNERS = {
  async run(tool, formEl, resultsEl, spinnerEl) {
    const params = new URLSearchParams();
    const primary = formEl.querySelector('[data-primary="1"]');
    if (primary) {
      const v = primary.value.trim();
      if (!v && !tool.allowEmptyInput) {
        primary.focus();
        return;
      }
      if (v) params.set(tool.input.name, v);
    }
    formEl.querySelectorAll('[data-extra]').forEach((el) => {
      const v = el.value.trim();
      if (v) params.set(el.dataset.extra, v);
    });

    resultsEl.classList.add('hidden');
    resultsEl.innerHTML = '';
    spinnerEl.classList.remove('hidden');

    if (tool.async) {
      try {
        const job = await window.VaultJobs.submit(
          tool.id,
          Object.fromEntries(params)
        );
        spinnerEl.classList.add('hidden');
        resultsEl.classList.remove('hidden');
        this.renderJobRun(tool, resultsEl, job);
      } catch (err) {
        spinnerEl.classList.add('hidden');
        resultsEl.classList.remove('hidden');
        resultsEl.innerHTML = `<div class="tool-error">Failed to start job: ${escapeHtml(err.message)}</div>`;
      }
      return;
    }

    try {
      const res = await fetch(`${tool.endpoint}?${params.toString()}`);
      const data = await res.json();
      spinnerEl.classList.add('hidden');
      resultsEl.classList.remove('hidden');

      const renderer = this.renderers[tool.render] || this.renderers.generic;
      resultsEl.innerHTML = renderer(data, tool);
    } catch (err) {
      spinnerEl.classList.add('hidden');
      resultsEl.classList.remove('hidden');
      resultsEl.innerHTML = `<div class="tool-error">Failed: ${escapeHtml(err.message)}</div>`;
    }
  },

  // Render the live running view for an async (job-managed) tool run, then
  // swap in the normal renderer once the job reaches a terminal state.
  async renderJobRun(tool, resultsEl, job) {
    const wrap = document.createElement('div');
    wrap.className = 'job-run';
    wrap.innerHTML = `
      <div class="job-run-head">
        <span class="job-run-spinner"></span>
        <span class="job-run-label">${escapeHtml(tool.title)}</span>
        <span class="job-run-id mono">${escapeHtml(job.id)}</span>
      </div>
      <pre class="job-run-log" data-job-run-log></pre>
      <div class="job-run-actions">
        <button type="button" class="btn-ghost btn-xs" data-job-run-cancel>Cancel</button>
        <span class="job-run-note">Running in background — open the Job Center to watch all runs.</span>
      </div>`;
    resultsEl.appendChild(wrap);

    const logEl = wrap.querySelector('[data-job-run-log]');
    const cancelBtn = wrap.querySelector('[data-job-run-cancel]');

    window.VaultJobs.api
      .logs(job.id)
      .then((lines) => {
        if (document.body.contains(logEl) && lines.length) {
          logEl.textContent = lines.join('\n');
          logEl.scrollTop = logEl.scrollHeight;
        }
      })
      .catch(() => {});

    const subs = [
      window.VaultJobs.on('job.log', (evt) => {
        if (evt.job_id !== job.id || !document.body.contains(logEl)) return;
        logEl.textContent += evt.line + '\n';
        logEl.scrollTop = logEl.scrollHeight;
      }),
      window.VaultJobs.on('job.completed', (evt) => {
        if (evt.job && evt.job.id === job.id) {
          subs.forEach((u) => u());
          this.finishJob(tool, resultsEl, job.id);
        }
      }),
      window.VaultJobs.on('job.failed', (evt) => {
        if (evt.job && evt.job.id === job.id) {
          subs.forEach((u) => u());
          resultsEl.innerHTML = `<div class="tool-error">${escapeHtml(
            evt.job.error || 'Job failed'
          )}</div>`;
        }
      }),
      window.VaultJobs.on('job.cancelled', (evt) => {
        if (evt.job && evt.job.id === job.id) {
          subs.forEach((u) => u());
          resultsEl.innerHTML = `<div class="tool-empty">Job cancelled.</div>`;
        }
      }),
    ];

    cancelBtn.addEventListener('click', async () => {
      cancelBtn.disabled = true;
      cancelBtn.textContent = 'Cancelling…';
      try {
        await window.VaultJobs.api.cancel(job.id);
      } catch {
        cancelBtn.disabled = false;
        cancelBtn.textContent = 'Cancel';
      }
    });

    // Race guard: a very fast job can finish before we subscribed to the bus.
    let view;
    try {
      view = await window.VaultJobs.api.get(job.id);
    } catch {
      return;
    }
    if (!document.body.contains(wrap)) return;
    if (window.VaultJobs.isTerminal(view.status)) {
      subs.forEach((u) => u());
      if (view.status === 'succeeded') {
        this.finishJob(tool, resultsEl, job.id);
      } else if (view.status === 'failed') {
        resultsEl.innerHTML = `<div class="tool-error">${escapeHtml(
          view.error || 'Job failed'
        )}</div>`;
      } else {
        resultsEl.innerHTML = `<div class="tool-empty">Job cancelled.</div>`;
      }
    }
  },

  async finishJob(tool, resultsEl, jobId) {
    try {
      const data = await window.VaultJobs.api.result(jobId);
      const renderer = this.renderers[tool.render] || this.renderers.generic;
      resultsEl.innerHTML = renderer(data, tool);
    } catch (err) {
      resultsEl.innerHTML = `<div class="tool-error">Failed to load result: ${escapeHtml(err.message)}</div>`;
    }
  },

  renderers: {
    wordpress(data) {
      const sourceLabels = {
        generator_meta_tag: 'Meta Generator Tag',
        wp_json: 'REST API (/wp-json/)',
        readme_html: 'readme.html',
      };
      const ver = data.version || 'Not detected';
      const verColor = data.version ? 'var(--accent)' : 'var(--text-muted)';
      const src = data.version_source
        ? sourceLabels[data.version_source] || data.version_source
        : '—';
      const rest = data.rest_api_available ? '✅ Available' : '❌ Not found';
      const xml = data.xmlrpc_available ? '⚠️ Enabled' : '✅ Disabled / Blocked';
      const readme = data.readme_accessible ? '⚠️ Accessible' : '✅ Blocked / Hidden';
      const server = (data.headers && data.headers.server) || '—';
      const err = data.error
        ? `<div class="tool-error">${escapeHtml(data.error)}</div>`
        : '';

      return `
        <div class="tool-results-grid">
          <div class="result-card version-card">
            <div class="result-label">WordPress Version</div>
            <div class="result-value" style="color:${verColor}">${escapeHtml(ver)}</div>
          </div>
          ${card('Source', src)}
          ${card('REST API', rest)}
          ${card('XML-RPC', xml)}
          ${card('readme.html', readme)}
          ${card('Server', server)}
          ${err}
        </div>`;
    },

    'wp-users'(data) {
      const users = data.users || [];
      const notes = (data.notes || []).map((n) => `<li>${escapeHtml(n)}</li>`).join('');
      let list = '';
      if (!users.length) {
        list = emptyMsg('No users found (or enum is locked down).');
      } else {
        list = `<div class="findings-list">${users
          .map((u) => {
            const title = [u.name, u.slug, u.id != null ? `id=${u.id}` : null]
              .filter(Boolean)
              .join(' · ');
            return `<div class="finding-row sev-medium">
              <span class="finding-sev">${escapeHtml(u.source || 'user')}</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(title || 'user')}</div>
                <div class="finding-meta mono">${escapeHtml(u.link || '')}</div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      return (
        `<div class="tool-results-grid">
          ${card('Users found', String(users.length))}
          ${card('REST users', data.rest_users_enabled ? 'Open' : 'Blocked / empty')}
          ${card('Author enum', data.author_enum_works ? 'Works' : 'No hits')}
        </div>` +
        list +
        (notes ? `<ul class="tool-notes-list">${notes}</ul>` : '') +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'wp-list'(data) {
      // plugins
      const items = data.plugins || [];
      let list = '';
      if (!items.length) {
        list = emptyMsg('No plugins detected from probe list / HTML.');
      } else {
        list = `<div class="comp-list">${items
          .map((p) =>
            compCard({
              type: 'plugin',
              slug: p.slug,
              name: p.slug,
              version: p.version,
              evidence: p.evidence,
              confidence: p.confidence,
              meta: `${p.path || ''} · HTTP ${String(p.status || '')}`,
            })
          )
          .join('')}</div>`;
      }
      return (
        `<div class="tool-results-grid">
          ${card('Plugins found', String(items.length))}
          ${card('Probed', String(data.probed || '—'))}
        </div>` +
        list +
        notesList(data.notes) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'wp-themes'(data) {
      const items = data.themes || [];
      let list = '';
      if (!items.length) {
        list = emptyMsg('No themes detected.');
      } else {
        list = `<div class="comp-list">${items
          .map((t) =>
            compCard({
              type: 'theme',
              slug: t.slug,
              name: t.theme_name ? `${t.theme_name} (${t.slug})` : t.slug,
              version: t.version,
              evidence: t.evidence,
              confidence: t.confidence,
              meta: t.path || '',
            })
          )
          .join('')}</div>`;
      }
      return (
        `<div class="tool-results-grid">
          ${card('Themes found', String(items.length))}
          ${card('Active guess', data.active_guess || '—')}
        </div>` +
        list +
        notesList(data.notes) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'wp-xmlrpc'(data) {
      const methods = data.methods || [];
      const interesting = data.interesting || [];
      return (
        `<div class="tool-results-grid">
          ${card('Available', data.available ? 'Yes' : 'No / blocked')}
          ${card('Methods', String(data.method_count || methods.length))}
          ${card('Multicall', data.multicall ? '⚠️ Yes' : 'No')}
          ${card('Pingback', data.pingback ? '⚠️ Yes' : 'No')}
        </div>` +
        (interesting.length
          ? `<p class="tool-note">Interesting methods</p>
             <div class="findings-list">${interesting
               .map(
                 (m) => `<div class="finding-row sev-medium">
                   <span class="finding-sev">rpc</span>
                   <div class="finding-body"><div class="finding-name mono">${escapeHtml(m)}</div></div>
                 </div>`
               )
               .join('')}</div>`
          : emptyMsg(data.available ? 'No high-interest methods flagged.' : 'XML-RPC not available.')) +
        (methods.length
          ? `<details class="tool-raw-details"><summary>All methods (${methods.length})</summary>
             <pre class="tool-raw-pre">${escapeHtml(methods.join('\n'))}</pre></details>`
          : '') +
        notesList(data.notes) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'wp-paths'(data) {
      const findings = data.findings || [];
      let list = '';
      if (!findings.length) {
        list = emptyMsg('No interesting sensitive paths found (good or filtered).');
      } else {
        list = `<div class="findings-list">${findings
          .map((f) => {
            const sev = (f.risk || 'info').toLowerCase();
            return `<div class="finding-row sev-${escapeHtml(sev)}">
              <span class="finding-sev">${escapeHtml(sev)}</span>
              <div class="finding-body">
                <div class="finding-name mono">${escapeHtml(f.path)}</div>
                <div class="finding-meta">HTTP ${escapeHtml(String(f.status))} · ${escapeHtml(f.note || '')} · ${escapeHtml(String(f.length || 0))}b</div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      return (
        `<div class="tool-results-grid">
          ${card('Findings', String(findings.length))}
          ${card('Probed', String(data.probed || '—'))}
        </div>` +
        list +
        notesList(data.notes) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'wp-vuln-scan'(data) {
      // stash for detail navigation
      try {
        sessionStorage.setItem('vault_last_wp_scan', JSON.stringify({
          url: data.url,
          at: Date.now(),
          findings: data.findings || [],
        }));
      } catch (_) {}

      if (data.error && !(data.findings && data.findings.length)) {
        return (
          `<div class="tool-error">${escapeHtml(data.error)}</div>` +
          notesList(data.notes) +
          (data.db
            ? `<p class="tool-note">DB: present=${data.db.present} · key=${data.db.api_key_configured}</p>`
            : '')
        );
      }

      const s = data.summary || {};
      const comps = data.components || [];
      const findings = data.findings || [];

      const summary = `<div class="tool-results-grid">
        ${card('Findings', String(s.total_findings ?? findings.length))}
        ${card('Critical', String(s.critical ?? 0))}
        ${card('High', String(s.high ?? 0))}
        ${card('Medium', String(s.medium ?? 0))}
        ${card('Low', String(s.low ?? 0))}
        ${card('Components', `${s.components_with_version ?? 0}/${s.components_scanned ?? comps.length} versioned`)}
      </div>`;

      const compList =
        comps.length === 0
          ? ''
          : `<p class="tool-note">Detected components</p>
             <div class="findings-list">${comps
               .map((c) => {
                 const ver = c.version || 'unknown';
                 const sev = c.version ? 'info' : 'medium';
                 return `<div class="finding-row sev-${sev}">
                   <span class="finding-sev">${escapeHtml(c.software_type || '?')}</span>
                   <div class="finding-body">
                     <div class="finding-name mono">${escapeHtml(c.slug || c.name || '')} <span style="color:var(--text-muted)">v${escapeHtml(ver)}</span></div>
                     <div class="finding-meta">${escapeHtml(c.name || '')} · ${escapeHtml(c.evidence || '')}</div>
                   </div>
                 </div>`;
               })
               .join('')}</div>`;

      let vulnList = '';
      if (!findings.length) {
        vulnList = emptyMsg(
          'No matching vulnerabilities for detected versioned components (or versions unknown).'
        );
      } else {
        vulnList = `<p class="tool-note">Vulnerabilities — click a row for full details</p>` +
          vulnShortCards(findings);
      }

      const dbLine = data.db
        ? `<p class="tool-note">DB ${data.db.count != null ? Number(data.db.count).toLocaleString() + ' records' : ''} · updated ${escapeHtml(data.db.updated_at || '—')} · ${data.duration_ms != null ? data.duration_ms + ' ms' : ''}</p>`
        : '';

      return (
        summary +
        dbLine +
        compList +
        vulnList +
        notesList(data.notes) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'wp-rest'(data) {
      const ns = data.namespaces || [];
      const routes = data.interesting_routes || [];
      return (
        `<div class="tool-results-grid">
          ${card('REST', data.available ? 'Available' : 'Blocked')}
          ${card('Site', data.name || '—')}
          ${card('Namespaces', String(ns.length))}
          ${card('Routes', String(data.route_count || 0))}
        </div>` +
        (ns.length
          ? `<p class="tool-note">Namespaces</p>
             <div class="tag-cloud">${ns
               .map((n) => `<span class="tag-chip">${escapeHtml(n)}</span>`)
               .join('')}</div>`
          : '') +
        (routes.length
          ? `<p class="tool-note">Interesting routes</p>
             <div class="findings-list">${routes
               .map(
                 (r) => `<div class="finding-row sev-info">
                   <span class="finding-sev">route</span>
                   <div class="finding-body"><div class="finding-name mono">${escapeHtml(r)}</div></div>
                 </div>`
               )
               .join('')}</div>`
          : emptyMsg(data.available ? 'No flagged routes.' : 'REST root not reachable.')) +
        notesList(data.notes) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'subdomains'(data) {
      if (!data.installed) {
        return missing(data, 'subfinder');
      }
      const subs = data.subdomains || [];
      let list = '';
      if (subs.length === 0) {
        list = emptyMsg('No subdomains found. Add provider API keys to subfinder config for more sources.');
      } else {
        list = `<div class="findings-list">${subs
          .map((s) => `<div class="finding-row sev-info">
            <span class="finding-sev">sub</span>
            <div class="finding-body">
              <div class="finding-name mono">${escapeHtml(s)}</div>
              <div class="finding-meta">${escapeHtml(data.domain || '')}</div>
            </div>
          </div>`)
          .join('')}</div>`;
      }
      return (
        `<div class="tool-results-grid">
          ${card('Subdomains', String(subs.length))}
          ${card('Domain', data.domain || '—')}
        </div>` +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    'url-list'(data) {
      if (!data.installed) {
        return missing(data, data.command && data.command.includes('katana') ? 'katana' : 'waybackurls');
      }
      const urls = data.urls || [];
      let list = '';
      if (urls.length === 0) {
        list = emptyMsg('No URLs found.');
      } else {
        list = `<div class="findings-list">${urls
          .map((u) => `<div class="finding-row sev-info">
            <span class="finding-sev">url</span>
            <div class="finding-body">
              <div class="finding-name mono">${escapeHtml(u)}</div>
            </div>
          </div>`)
          .join('')}</div>`;
      }
      return (
        metaBar(data) +
        `<div class="tool-results-grid">
          ${card('URLs', String(data.count ?? urls.length))}
        </div>` +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    'js-analysis'(data) {
      if (!data.installed) {
        return missing(data, 'js-analysis');
      }
      const counts = data.counts || {};
      const scripts = data.scripts || [];
      const endpoints = data.endpoints || [];
      const secrets = data.secrets || [];

      let scriptLine = '';
      if (scripts.length) {
        scriptLine = `<p class="tool-note">Scripts (${scripts.length})</p>
          <div class="tag-cloud">${scripts
            .map((s) => `<span class="tag-chip mono" title="${escapeHtml(s.url)}">${escapeHtml(s.url)}</span>`)
            .join('')}</div>`;
      }

      let apiBlock = '';
      const apiUrls = endpoints.filter(
        (u) => u.includes('/api/') || u.includes('/graphql') || u.includes('/wp-json')
      );
      if (apiUrls.length) {
        apiBlock = `<p class="tool-note">API endpoints</p>
          <div class="findings-list">${apiUrls
            .map((u) => `<div class="finding-row sev-high">
              <span class="finding-sev">api</span>
              <div class="finding-body"><div class="finding-name mono">${escapeHtml(u)}</div></div>
            </div>`)
            .join('')}</div>`;
      }

      let endpointBlock = '';
      if (endpoints.length) {
        endpointBlock = `<details class="tool-raw-details" ${apiUrls.length ? '' : 'open'}>
          <summary>All endpoints (${endpoints.length})</summary>
          <div class="findings-list">${endpoints
            .map((u) => `<div class="finding-row sev-info">
              <span class="finding-sev">ep</span>
              <div class="finding-body"><div class="finding-name mono">${escapeHtml(u)}</div></div>
            </div>`)
            .join('')}</div>
        </details>`;
      }

      let secretBlock = '';
      if (secrets.length) {
        secretBlock = `<p class="tool-note">Possible secrets — verify before trusting!</p>
          <div class="findings-list">${secrets
            .map((s) => `<div class="finding-row sev-critical">
              <span class="finding-sev">${escapeHtml(s.key)}</span>
              <div class="finding-body">
                <div class="finding-meta mono">${escapeHtml(s.value)}</div>
              </div>
            </div>`)
            .join('')}</div>`;
      }

      return (
        metaBar(data) +
        `<div class="tool-results-grid">
          ${card('Scripts', String(counts.scripts ?? scripts.length))}
          ${card('Endpoints', String(counts.endpoints ?? endpoints.length))}
          ${card('API routes', String(counts.api_endpoints ?? apiUrls.length))}
          ${card('Secrets', String(counts.secrets ?? secrets.length))}
        </div>` +
        scriptLine +
        apiBlock +
        endpointBlock +
        secretBlock +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    'cors-check'(data) {
      if (!data.installed) {
        return missing(data, 'cors-check');
      }
      const tests = data.tests || [];
      let list = '';
      if (!tests.length) {
        list = emptyMsg('No tests run.');
      } else {
        list = `<div class="findings-list">${tests
          .map((t) => {
            const sev = t.verdict || 'ok';
            const ao = t.allow_origin
              ? `· ACAO: ${escapeHtml(t.allow_origin)}`
              : '';
            const cred = t.allow_credentials
              ? ' · ⚠️ Allow-Credentials: true'
              : '';
            return `<div class="finding-row sev-${escapeHtml(sev)}">
              <span class="finding-sev">${escapeHtml(sev)}</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(t.name)} <code class="mono">${escapeHtml(t.origin)}</code></div>
                <div class="finding-meta">${escapeHtml(t.note)}${ao}${cred}</div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      const risk =
        data.high_risk
          ? `<p class="tool-note">High risk — reflected origin with credentials (or wildcard+credentials).</p>`
          : data.medium_risk
            ? `<p class="tool-note">Medium risk — reflected origin without credentials.</p>`
            : `<p class="tool-note">No risky CORS configuration detected.</p>`;
      return (
        metaBar(data) +
        `<div class="tool-results-grid">
          ${card('Tests', String(tests.length))}
          ${card('High risk', data.high_risk ? '⚠️ Yes' : 'No')}
        </div>` +
        risk +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'open-redirect'(data) {
      if (!data.installed) {
        return missing(data, 'open-redirect');
      }
      const tests = data.tests || [];
      let list = '';
      if (!tests.length) {
        list = emptyMsg('No tests run.');
      } else {
        list = `<div class="findings-list">${tests
          .map((t) => {
            const sev = t.vulnerable ? 'high' : 'info';
            const loc = t.location
              ? `<div class="finding-meta mono">Location: ${escapeHtml(t.location)}</div>`
              : '';
            return `<div class="finding-row sev-${escapeHtml(sev)}">
              <span class="finding-sev">${escapeHtml(t.vulnerable ? 'open' : t.status != null ? String(t.status) : 'err')}</span>
              <div class="finding-body">
                <div class="finding-name mono">?${escapeHtml(t.param)}=</div>
                <div class="finding-meta">${escapeHtml(t.note)}</div>
                ${loc}
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      const verdict = data.vulnerable
        ? `<p class="tool-note">Potential open redirect — verify manually!</p>`
        : `<p class="tool-note">No off-site redirects on the probed params.</p>`;
      return (
        metaBar(data) +
        `<div class="tool-results-grid">
          ${card('Params probed', String(tests.length))}
          ${card('Vulnerable', data.vulnerable ? '⚠️ Yes' : 'No')}
        </div>` +
        verdict +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    httpx(data) {
      if (!data.installed) {
        return missing(data, 'httpx');
      }
      const f = (data.findings && data.findings[0]) || null;
      if (!f) {
        return metaBar(data) + emptyMsg('No response parsed. Is the host up?') + rawBlock(data.raw);
      }
      return (
        metaBar(data) +
        `<div class="tool-results-grid">
          ${card('URL', f.url || f.input || data.url || '—')}
          ${card('Status', String(f.status_code ?? f['status-code'] ?? '—'))}
          ${card('Title', f.title || '—')}
          ${card('Web Server', f.webserver || f['web-server'] || '—')}
          ${card('IP', Array.isArray(f.a) ? f.a.join(', ') : f.ip || f.host || '—')}
          ${card('Tech', formatTech(f.tech || f.technologies))}
          ${card('Length', String(f.content_length ?? f['content-length'] ?? '—'))}
        </div>` +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    nuclei(data) {
      if (!data.installed) {
        return missing(data, 'nuclei');
      }
      const findings = data.findings || [];
      let list = '';
      if (findings.length === 0) {
        list = emptyMsg('No findings at selected severity (or templates not installed).');
      } else {
        list = `<div class="findings-list">${findings
          .map((f) => {
            const info = f.info || {};
            const name = info.name || f['template-id'] || f.template_id || 'Finding';
            const sev = (info.severity || 'unknown').toLowerCase();
            const matched = f['matched-at'] || f.matched_at || f.host || '';
            return `<div class="finding-row sev-${escapeHtml(sev)}">
              <span class="finding-sev">${escapeHtml(sev)}</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(name)}</div>
                <div class="finding-meta">${escapeHtml(matched)}</div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      return (
        metaBar(data) +
        (data.note ? `<p class="tool-note">${escapeHtml(data.note)}</p>` : '') +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    ffuf(data) {
      if (!data.installed) {
        return missing(data, 'ffuf');
      }
      const findings = data.findings || [];
      let list = '';
      if (findings.length === 0) {
        list = emptyMsg('No interesting paths (or wordlist empty).');
      } else {
        list = `<div class="findings-list">${findings
          .map((f) => {
            const url = f.url || f.input || '—';
            const status = f.status ?? f['status'] ?? '—';
            const length = f.length ?? f['length'] ?? '';
            const words = f.words ?? '';
            return `<div class="finding-row">
              <span class="finding-sev sev-info">${escapeHtml(String(status))}</span>
              <div class="finding-body">
                <div class="finding-name mono">${escapeHtml(String(url))}</div>
                <div class="finding-meta">len ${escapeHtml(String(length))} · words ${escapeHtml(String(words))}</div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      return (
        metaBar(data) +
        (data.wordlist
          ? `<p class="tool-note">Wordlist: <code>${escapeHtml(data.wordlist)}</code></p>`
          : '') +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    gitleaks(data) {
      if (!data.installed) {
        return missing(data, 'gitleaks');
      }
      const findings = data.findings || [];
      let list = '';
      if (findings.length === 0) {
        list = emptyMsg('No secrets found (clean or path empty).');
      } else {
        list = `<div class="findings-list">${findings
          .map((f) => {
            const rule = f.RuleID || f.Description || f.rule || 'secret';
            const file = f.File || f.file || '';
            const line = f.StartLine || f.line || '';
            return `<div class="finding-row sev-high">
              <span class="finding-sev">leak</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(String(rule))}</div>
                <div class="finding-meta mono">${escapeHtml(String(file))}${line ? ':' + escapeHtml(String(line)) : ''}</div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }
      return (
        metaBar(data) +
        (data.path ? `<p class="tool-note">Path: <code>${escapeHtml(data.path)}</code></p>` : '') +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    trivy(data) {      if (!data.installed) {
        return missing(data, 'trivy');
      }
      const report = data.report;
      const results = (report && report.Results) || [];
      let rows = [];
      for (const r of results) {
        const target = r.Target || r.Type || 'target';
        for (const v of r.Vulnerabilities || []) {
          rows.push({
            sev: (v.Severity || 'UNKNOWN').toLowerCase(),
            name: v.VulnerabilityID || v.PkgName || 'vuln',
            meta: `${target} · ${v.PkgName || ''} ${v.InstalledVersion || ''}`,
          });
        }
        for (const s of r.Secrets || []) {
          rows.push({
            sev: 'high',
            name: s.Title || s.RuleID || 'secret',
            meta: `${target} · ${s.Category || ''}`,
          });
        }
        for (const m of r.Misconfigurations || []) {
          rows.push({
            sev: (m.Severity || 'medium').toLowerCase(),
            name: m.Title || m.ID || 'misconfig',
            meta: `${target} · ${m.Type || ''}`,
          });
        }
      }

      let list = '';
      if (rows.length === 0) {
        list = emptyMsg('No HIGH/CRITICAL issues in report (or first-run DB still downloading).');
      } else {
        list = `<div class="findings-list">${rows
          .slice(0, 80)
          .map(
            (r) => `<div class="finding-row sev-${escapeHtml(r.sev)}">
              <span class="finding-sev">${escapeHtml(r.sev)}</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(r.name)}</div>
                <div class="finding-meta">${escapeHtml(r.meta)}</div>
              </div>
            </div>`
          )
          .join('')}</div>`;
        if (rows.length > 80) {
          list += `<p class="tool-note">Showing 80 of ${rows.length} issues — see raw JSON for full report.</p>`;
        }
      }

      return (
        metaBar(data) +
        (data.path ? `<p class="tool-note">Path: <code>${escapeHtml(data.path)}</code></p>` : '') +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(data.raw)
      );
    },

    'cve-lookup'(data) {
      if (data.error && !(data.records && data.records.length)) {
        return (
          metaBar(data) +
          `<div class="tool-error">${escapeHtml(data.error)}</div>` +
          notesList(data.notes)
        );
      }
      const records = data.records || [];
      if (!records.length) {
        return emptyMsg('No CVEs found. Try a CVE ID like CVE-2024-1234 or a keyword.');
      }
      return (
        metaBar(data) +
        `<p class="tool-note">${records.length} record(s)${data.cached ? ' · served from on-disk cache' : ' · live NVD'}</p>` +
        `<div class="findings-list">${records
          .map((r) => {
            const sev = severityClass(r);
            const score = pickScore(r);
            const scoreBadge = score
              ? `<span class="finding-sev sev-${sev}">CVSS ${escapeHtml(score.base_score.toFixed(1))} ${escapeHtml(score.severity)}</span>`
              : `<span class="finding-sev sev-info">n/a</span>`;
            const cwes = (r.cwes || []).map((c) => escapeHtml(c)).join(', ');
            const refs = (r.references || [])
              .slice(0, 5)
              .map((x) => `<a href="${escapeHtml(x.url)}" target="_blank" rel="noopener">${escapeHtml(x.source || x.url)}</a>`)
              .join(' · ');
            const published = r.published ? `<span>${escapeHtml(r.published.slice(0, 10))}</span>` : '';
            return `<div class="finding-row sev-${sev}">
              ${scoreBadge}
              <div class="finding-body">
                <div class="finding-name mono">${escapeHtml(r.cve_id)}</div>
                <div class="finding-meta">${escapeHtml(r.description || '')}</div>
                ${cwes ? `<div class="finding-meta">CWE: ${cwes}</div>` : ''}
                ${published ? `<div class="finding-meta">Published: ${published}</div>` : ''}
                ${refs ? `<div class="finding-meta">${refs}</div>` : ''}
                <div class="finding-actions">
                  <button type="button" class="btn-ghost btn-xs" data-finding-save
                    data-title="${escapeHtml(r.cve_id + ': ' + truncate(r.description, 140))}"
                    data-cve="${escapeHtml(r.cve_id)}"
                    data-cvss="${score ? score.base_score : ''}"
                    data-severity="${score ? score.severity.toLowerCase() : ''}"
                    data-description="${escapeHtml(r.description || '')}"
                    data-references="${escapeHtml((r.references || []).map((x) => x.url).join('\n'))}"
                    data-endpoint="${escapeHtml(r.description.match(/in ([^\s.]+)/)?.[1] || '')}">
                    Save to findings
                  </button>
                </div>
              </div>
            </div>`;
          })
          .join('')}</div>` +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'findings-db'(data) {
      if (data.error) {
        return `<div class="tool-error">${escapeHtml(data.error)}</div>`;
      }
      const findings = data.findings || [];
      const sevCounts = {};
      const statusCounts = {};
      for (const f of findings) {
        sevCounts[f.severity || 'medium'] = (sevCounts[f.severity || 'medium'] || 0) + 1;
        statusCounts[f.status || 'open'] = (statusCounts[f.status || 'open'] || 0) + 1;
      }
      const sevCards = ['critical', 'high', 'medium', 'low', 'info']
        .filter((s) => sevCounts[s])
        .map((s) => card(s, String(sevCounts[s])))
        .join('');
      const statusLine = Object.keys(statusCounts)
        .map((s) => `${escapeHtml(s)}: ${statusCounts[s]}`)
        .join(' · ');

      const form = `
        <div class="finding-form hidden" data-finding-form-wrap>
          <div class="tool-results-grid">
            ${input('title', 'Title *', 'f_title', 'Title of the finding', true)}
            ${input('target', 'Target / host', 'f_target', 'example.com')}
          </div>
          <div class="tool-results-grid">
            ${input('vuln_type', 'Vuln type', 'f_vuln_type', 'XSS, SQLi, IDOR…')}
            ${select('Severity', 'f_severity', ['info', 'low', 'medium', 'high', 'critical'])}
            ${select('Status', 'f_status', ['open', 'confirmed', 'fixed', 'accepted', 'info'])}
          </div>
          <div class="tool-results-grid">
            ${input('cve_id', 'CVE ID', 'f_cve_id', 'CVE-2024-1234 (optional)')}
            ${input('cvss_score', 'CVSS score', 'f_cvss_score', '9.8 (optional)')}
            ${input('endpoint', 'Affected endpoint', 'f_endpoint', '/api/v1/users (optional)')}
          </div>
          ${textarea('description', 'Description', 'f_description', 'What, where, impact…')}
          ${textarea('remediation', 'Remediation', 'f_remediation', 'How to fix…')}
          ${input('text', 'References (one per line)', 'f_references', 'https://…')}
          ${input('text', 'Tags (comma separated)', 'f_tags', 'wordpress, authenticated')}
          <div class="tool-results-grid form-actions">
            <button type="button" class="btn-primary" data-finding-save-new>Save finding</button>
            <button type="button" class="btn-ghost" data-finding-cancel>Cancel</button>
          </div>
        </div>`;

      let list = '';
      if (!findings.length) {
        list = emptyMsg('No findings yet. Run a lookup, then Save to findings — or add one manually.');
      } else {
        list = `<div class="findings-list">${findings
          .map((f) => {
            const sev = severityFrom(f.severity);
            return `<div class="finding-row sev-${sev}">
              <span class="finding-sev">${escapeHtml(f.severity)} ${f.cvss_score ? escapeHtml(Number(f.cvss_score).toFixed(1)) : ''}</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(f.title)}</div>
                <div class="finding-meta">
                  ${f.cve_id ? `<span class="vuln-cve-tag mono">${escapeHtml(f.cve_id)}</span>` : ''}
                  <span>${escapeHtml(f.target || '')}</span>
                  <span>${escapeHtml(f.vuln_type || '')}</span>
                  ${f.endpoint ? `<code>${escapeHtml(f.endpoint)}</code>` : ''}
                  <span class="pill ${f.status === 'fixed' ? 'pill-ok' : 'pill-bad'}">${escapeHtml(f.status)}</span>
                </div>
                ${f.description ? `<div class="finding-meta">${escapeHtml(truncate(f.description, 160))}</div>` : ''}
                <div class="finding-meta mono">updated ${escapeHtml((f.updated_at || '').slice(0, 10))}</div>
                <div class="finding-actions">
                  <button type="button" class="btn-ghost btn-xs" data-finding-edit data-id="${escapeHtml(f.id)}">Edit</button>
                  <button type="button" class="btn-ghost btn-xs" data-finding-delete data-id="${escapeHtml(f.id)}">Delete</button>
                </div>
              </div>
            </div>`;
          })
          .join('')}</div>`;
      }

      return (
        metaBar(data) +
        `<div class="tool-results-grid">
          ${card('Findings', String(findings.length))}
          ${sevCards || card('Severity', '—')}
        </div>` +
        (statusLine ? `<p class="tool-note">${statusLine}</p>` : '') +
        `<div class="tool-results-grid form-actions">
          <button type="button" class="btn-primary" data-finding-new>New finding</button>
        </div>` +
        form +
        list +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '')
      );
    },

    'attack-surface'(data) {
      const html = buildAttackSurfaceHtml(data);
      queueMicrotask(() => {
        const pane = [...document.querySelectorAll('.tool-pane')].find(
          (p) => !p.classList.contains('hidden')
        );
        const root = pane && pane.querySelector('[data-ase-root]');
        if (root) window.AttackSurfaceExplorer.bind(root, data);
      });
      return html;
    },

    generic(data) {
      return (
        metaBar(data) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(JSON.stringify(data, null, 2))
      );
    },
  },
};function escapeHtml(s) {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function card(label, value) {
  return `<div class="result-card">
    <div class="result-label">${escapeHtml(label)}</div>
    <div class="result-value">${escapeHtml(String(value))}</div>
  </div>`;
}

function metaBar(data) {
  const ms = data.duration_ms != null ? `${data.duration_ms} ms` : '';
  const cmd = data.command ? escapeHtml(data.command) : '';
  return `<div class="tool-meta-bar">
    ${ms ? `<span>${escapeHtml(ms)}</span>` : ''}
    ${cmd ? `<code title="${cmd}">${cmd}</code>` : ''}
  </div>`;
}

function rawBlock(raw) {
  if (!raw) return '';
  return `<details class="tool-raw-details">
    <summary>Raw output</summary>
    <pre class="tool-raw-pre">${escapeHtml(raw)}</pre>
  </details>`;
}

function emptyMsg(msg) {
  return `<div class="tool-empty">${escapeHtml(msg)}</div>`;
}

function missing(data, binary) {
  return `<div class="tool-error">
    <strong>${escapeHtml(binary)}</strong> is not installed on this machine.<br>
    Run: <code>brew install ${escapeHtml(binary)}</code>
    ${data.error ? `<br>${escapeHtml(data.error)}` : ''}
  </div>`;
}

function formatTech(tech) {
  if (!tech) return '—';
  if (Array.isArray(tech)) return tech.join(', ') || '—';
  if (typeof tech === 'object') return Object.keys(tech).join(', ') || '—';
  return String(tech);
}

function notesList(notes) {
  if (!notes || !notes.length) return '';
  return `<ul class="tool-notes-list">${notes
    .map((n) => `<li>${escapeHtml(n)}</li>`)
    .join('')}</ul>`;
}

// Expandable component card (Plugin Enum / Theme Enum). Enrichment body is
// lazily filled by the delegated handler in app.js via /api/tools/component-intel.
function compCard(o) {
  const ver = o.version
    ? `<span class="comp-version">v${escapeHtml(o.version)}</span>`
    : `<span class="comp-version comp-version-unknown">version unknown</span>`;
  const conf =
    o.confidence != null
      ? `<span class="comp-conf" title="Detection confidence">${escapeHtml(String(o.confidence))}%</span>`
      : '';
  return `<div class="comp-card" data-comp-card
      data-comp-type="${escapeHtml(o.type)}"
      data-comp-slug="${escapeHtml(o.slug)}"
      ${o.version ? `data-comp-version="${escapeHtml(o.version)}"` : ''}>
    <button type="button" class="comp-head" data-comp-toggle>
      <span class="comp-sev">${escapeHtml(o.evidence || o.type)}</span>
      <span class="comp-main">
        <span class="comp-name">${escapeHtml(o.name)}${ver}</span>
        <span class="comp-meta mono">${escapeHtml(o.meta || '')}</span>
      </span>
      ${conf}
      <span class="comp-chev">▸</span>
    </button>
    <div class="comp-body" data-comp-body hidden></div>
  </div>`;
}

function metaCell(label, html) {
  return `<div class="result-card comp-cell">
    <div class="result-label">${escapeHtml(label)}</div>
    <div class="result-value">${html || '—'}</div>
  </div>`;
}

// Renders the lazy-loaded /api/tools/component-intel payload inside a card body.
function renderComponentIntel(d) {
  if (d.error && !d.name) {
    return `<div class="comp-error">${escapeHtml(d.error)}</div>` +
      notesList(d.notes);
  }
  const inst = d.detected_version
    ? `v${escapeHtml(d.detected_version)}`
    : 'unknown';
  const latest = d.latest_version
    ? `<code>${escapeHtml(d.latest_version)}</code>`
    : '—';
  let pill = '<span class="pill">—</span>';
  if (d.outdated === true) pill = '<span class="pill pill-bad">⚠️ Outdated</span>';
  else if (d.outdated === false) pill = '<span class="pill pill-ok">Up to date</span>';

  const links = [
    d.repo_url
      ? `<a href="${escapeHtml(d.repo_url)}" target="_blank" rel="noopener">${escapeHtml(d.on_repo ? 'WP.org' : 'repo')} →</a>`
      : '',
    d.homepage
      ? `<a href="${escapeHtml(d.homepage)}" target="_blank" rel="noopener">homepage →</a>`
      : '',
  ]
    .filter(Boolean)
    .join(' ');

  const grid = `<div class="tool-results-grid comp-meta-grid">
    ${metaCell('Installed', inst)}
    ${metaCell('Latest', latest)}
    ${metaCell('Status', pill)}
    ${metaCell('Maintainer', escapeHtml(d.author || '—'))}
    ${metaCell('Downloads', d.downloads != null ? Number(d.downloads).toLocaleString() : '—')}
    ${metaCell('Active installs', d.active_installs != null ? Number(d.active_installs).toLocaleString() : '—')}
    ${metaCell('Last updated', escapeHtml((d.last_updated || '').slice(0, 10) || '—'))}
    ${metaCell('Requires / Tested', escapeHtml([d.requires, d.tested].filter(Boolean).join(' / ') || '—'))}
    ${metaCell('Links', links)}
  </div>`;

  const tags = (d.tags || []).length
    ? `<p class="tool-note">Tags</p><div class="tag-cloud">${d.tags
        .slice(0, 12)
        .map((t) => `<span class="tag-chip">${escapeHtml(t)}</span>`)
        .join('')}</div>`
    : '';

  let vulnBlock = '';
  if (d.db_note) {
    vulnBlock = `<p class="tool-note">⚠️ ${escapeHtml(d.db_note)}</p>`;
  } else if (d.detected_version == null) {
    vulnBlock = `<p class="tool-note">Installed version unknown — vulnerability matching skipped. Version detection is strongest via readme.txt / style.css.</p>`;
  } else if (!(d.vulnerabilities || []).length) {
    vulnBlock = `<p class="tool-note">No matching vulnerabilities in the Wordfence DB for <code>v${escapeHtml(d.detected_version)}</code>.</p>`;
  } else {
    vulnBlock = `<p class="tool-note">Vulnerabilities (${d.vulnerabilities.length}) — click a row for full details</p>` +
      vulnShortCards(d.vulnerabilities);
  }

  return grid + tags + vulnBlock + (d.notes && d.notes.length ? notesList(d.notes) : '');
}

// Shared short vulnerability cards — used by the WF scan renderer and the
// lazily-enriched component cards (both rely on app.js [data-vuln-open]).
function vulnShortCards(findings) {
  return `<div class="findings-list vuln-short-list">${findings
    .map((f, i) => {
      const rating = (f.cvss_rating || 'none').toLowerCase();
      const sevClass =
        rating.includes('critical')
          ? 'critical'
          : rating.includes('high')
            ? 'high'
            : rating.includes('medium')
              ? 'medium'
              : rating.includes('low')
                ? 'low'
                : 'info';
      const score = f.cvss_score != null ? Number(f.cvss_score).toFixed(1) : '—';
      const cve = f.cve || 'No CVE ID';
      const patch = f.patched
        ? `Patched${(f.patched_versions || []).length ? ' → ' + f.patched_versions.join(', ') : ''}`
        : 'Unpatched';
      const affected = (f.affected_versions || []).join(', ') || '—';
      const remShort = (f.remediation || '').slice(0, 90);
      return `<button type="button" class="finding-row sev-${sevClass} vuln-short-card"
          data-vuln-open
          data-vuln-id="${escapeHtml(f.id)}"
          data-software-type="${escapeHtml(f.software_type || '')}"
          data-slug="${escapeHtml(f.slug || '')}"
          data-detected-version="${escapeHtml(f.detected_version || '')}"
          data-idx="${i}">
        <span class="finding-sev">${escapeHtml(f.cvss_rating || 'n/a')} ${escapeHtml(score)}</span>
        <div class="finding-body">
          <div class="finding-name">
            <span class="vuln-cve-tag mono">${escapeHtml(cve)}</span>
            ${escapeHtml(f.title || '')}
          </div>
          <div class="finding-meta vuln-short-meta">
            <span><strong>${escapeHtml(f.software_type)}</strong> ${escapeHtml(f.slug)} <code>v${escapeHtml(f.detected_version || '?')}</code></span>
            <span class="pill ${f.patched ? 'pill-ok' : 'pill-bad'}">${escapeHtml(patch)}</span>
            <span>Affected: <code>${escapeHtml(affected)}</code></span>
            ${f.cwe_id != null || f.cwe ? `<span>CWE-${escapeHtml(String(f.cwe_id ?? ''))}${f.cwe ? ' ' + escapeHtml(f.cwe) : ''}</span>` : ''}
            ${f.published ? `<span>${escapeHtml(String(f.published).slice(0, 10))}</span>` : ''}
          </div>
          ${remShort ? `<div class="vuln-short-rem">${escapeHtml(remShort)}${(f.remediation || '').length > 90 ? '…' : ''}</div>` : ''}
          <div class="vuln-short-cta">View full details →</div>
        </div>
      </button>`;
    })
    .join('')}</div>`;
}

function truncate(s, n) {
  if (!s) return '';
  const t = String(s);
  return t.length > n ? t.slice(0, n) + '…' : t;
}

function pickScore(r) {
  return r.cvss_v3 || r.cvss_v2 || null;
}

function severityClass(r) {
  const s = pickScore(r);
  if (!s) return 'info';
  return severityFrom(s.severity);
}

function severityFrom(s) {
  const t = String(s || '').toLowerCase();
  if (t.includes('critical')) return 'critical';
  if (t.includes('high')) return 'high';
  if (t.includes('medium')) return 'medium';
  if (t.includes('low')) return 'low';
  return 'info';
}

function input(type, label, name, placeholder, required) {
  return `<div class="result-card form-field">
    <label class="result-label" for="${name}">${escapeHtml(label)}</label>
    <input type="${escapeHtml(type || 'text')}" id="${name}" name="${name}" placeholder="${escapeHtml(placeholder || '')}" ${required ? 'required' : ''} data-findings-input>
  </div>`;
}

function select(label, name, options) {
  const opts = (options || [])
    .map((o) => `<option value="${escapeHtml(o)}">${escapeHtml(o)}</option>`)
    .join('');
  return `<div class="result-card form-field">
    <label class="result-label" for="${name}">${escapeHtml(label)}</label>
    <select id="${name}" name="${name}" data-findings-input>${opts}</select>
  </div>`;
}

function textarea(label, name, placeholder) {
  return `<div class="result-card form-field form-field-wide">
    <label class="result-label" for="${name}">${escapeHtml(label)}</label>
    <textarea id="${name}" name="${name}" rows="2" placeholder="${escapeHtml(placeholder || '')}" data-findings-input></textarea>
  </div>`;
}

// ─── Attack Surface Explorer ────────────────────────────────────────────────
// Aggregation view over all WordPress tool results (see /api/tools/attack-surface).
// Filter/search/tree state lives here so SSE-driven refreshes preserve it.

const ASE_TRACKED_TOOLS = [
  'wordpress-check',
  'wordpress-plugins',
  'wordpress-themes',
  'wordpress-rest',
  'wordpress-xmlrpc',
  'wordpress-users',
  'wordpress-paths',
  'wordpress-vuln-scan',
  'wordpress-nuclei',
];

const ASE_SEVERITIES = [
  ['all', 'All'],
  ['critical', 'Critical'],
  ['high', 'High'],
  ['medium', 'Medium'],
  ['low', 'Low'],
  ['informational', 'Info'],
];

const ASE_CATEGORIES = [
  ['all', 'All'],
  ['authentication', 'Authentication'],
  ['plugins', 'Plugins'],
  ['themes', 'Themes'],
  ['rest', 'REST'],
  ['files', 'Files'],
  ['infrastructure', 'Infrastructure'],
  ['vulnerabilities', 'Vulnerabilities'],
];

const ASE_ICONS = {
  core: '◉',
  authentication: '🔑',
  rest: '↔',
  plugins: '🧩',
  themes: '🎨',
  files: '📄',
  headers: '🛡',
  vulnerabilities: '⚠️',
  infrastructure: '🖥',
};

function aseNormUrl(s) {
  return String(s || '').trim().replace(/\/+$/, '').toLowerCase();
}

window.AttackSurfaceExplorer = (() => {
  const state = {
    url: '',
    data: null,
    el: null,
    severity: 'all',
    category: 'all',
    query: '',
    expanded: new Set(),
    pending: new Set(), // toolIds being run via the missing-source Run buttons
    timer: null,
    running: false,
    dirty: false,
    bound: false,
  };

  function sameUrl(a, b) {
    return aseNormUrl(a) === aseNormUrl(b);
  }

  function debounceRefetch() {
    clearTimeout(state.timer);
    state.timer = setTimeout(refetch, 400);
  }

  async function refetch() {
    if (!state.url || !state.el || !document.contains(state.el)) return;
    if (state.running) {
      state.dirty = true; // coalesce: run again right after the in-flight one
      return;
    }
    state.running = true;
    state.dirty = false;
    try {
      const res = await fetch(
        `/api/tools/attack-surface?url=${encodeURIComponent(state.url)}`
      );
      const data = await res.json();
      state.data = data;
      if (document.contains(state.el)) {
        state.el.innerHTML = buildAttackSurfaceHtml(data);
      }
    } catch {
      /* keep current view on transient errors */
    } finally {
      state.running = false;
      if (state.dirty) debounceRefetch();
    }
  }

  // One shared SSE subscription; guards against stale/detached panes.
  function bind(root, data) {
    state.url = data.url || '';
    state.data = data;
    state.el = root;
    if (state.bound) return;
    state.bound = true;

    const tracked = (evt) => {
      const tool = evt.job && evt.job.tool;
      if (!ASE_TRACKED_TOOLS.includes(tool)) return false;
      const jurl = (evt.job.params && evt.job.params.url) || '';
      return sameUrl(jurl, state.url);
    };

    ['job.completed', 'job.failed', 'job.cancelled'].forEach((type) => {
      window.VaultJobs.on(type, (evt) => {
        if (!tracked(evt)) return;
        state.pending.delete(evt.job.tool);
        debounceRefetch();
      });
    });
  }

  function toggleNode(id) {
    if (state.expanded.has(id)) state.expanded.delete(id);
    else state.expanded.add(id);
    render();
  }

  function setSeverity(v) {
    state.severity = v;
    render();
  }
  function setCategory(v) {
    state.category = v;
    render();
  }
  function setQuery(q) {
    state.query = q || '';
    render();
  }

  function render() {
    if (!state.el || !document.contains(state.el) || !state.data) return;
    if (state.data.missing) {
      const missingIds = new Set(state.data.missing.map((m) => m.tool));
      for (const t of [...state.pending]) {
        if (!missingIds.has(t) && !toolRunningOn(t, state.url)) state.pending.delete(t);
      }
    }
    const box = state.el.querySelector('[data-ase-search]');
    const focused = box && document.activeElement === box;
    const caret = focused ? box.selectionStart : 0;
    state.el.innerHTML = buildAttackSurfaceHtml(state.data);
    if (focused) {
      const nb = state.el.querySelector('[data-ase-search]');
      if (nb) {
        nb.focus();
        const end = Math.min(caret, nb.value.length);
        nb.setSelectionRange(end, end);
      }
    }
  }

  function toolRunningOn(toolId, url) {
    const jobs = window.VaultJobs.state.byTool(toolId);
    return jobs.some((j) => {
      if (window.VaultJobs.isTerminal(j.status)) return false;
      return sameUrl((j.params && j.params.url) || '', url);
    });
  }

  async function runMissing(toolId) {
    const tool = window.VAULT_TOOLS.getTool(toolId);
    if (!tool || !state.url) return;
    if (state.pending.has(toolId)) return; // already starting
    const url = state.url;
    const keepPending = !!tool.async; // async stays "Running…" until terminal SSE

    state.pending.add(toolId);
    render(); // flip the row to "Running…"

    try {
      if (tool.async) {
        if (toolRunningOn(toolId, url)) {
          window.VaultJobs.toast(`${tool.label} is already running for this target`, {
            type: 'warn',
            body: 'Open the Job Center to watch live progress.',
          });
          return;
        }
        const job = await window.VaultJobs.submit(toolId, { url });
        window.VaultJobs.toast(`Started ${tool.label}`, {
          type: 'info',
          body: `${job.id} · the tree refreshes when it completes.`,
          timeout: 4500,
        });
      } else {
        const res = await fetch(`${tool.endpoint}?url=${encodeURIComponent(url)}`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        await refetch();
        window.VaultJobs.toast(`${tool.label} done — Attack Surface refreshed`, {
          type: 'ok',
          body: '',
          timeout: 3500,
        });
      }
    } catch (err) {
      window.VaultJobs.toast(`Failed to run ${tool.label}`, {
        type: 'err',
        body: err.message,
      });
    } finally {
      if (!keepPending) state.pending.delete(toolId);
      render();
    }
  }

  function expandAll() {
    if (!state.data) return;
    for (const n of state.data.nodes || []) state.expanded.add(n.id);
    render();
  }

  function collapseAll() {
    state.expanded.clear();
    render();
  }

  return {
    bind,
    toggleNode,
    setSeverity,
    setCategory,
    setQuery,
    refetch,
    runMissing,
    expandAll,
    collapseAll,
    state,
  };
})();

function aseState() {
  return window.AttackSurfaceExplorer.state;
}

function aseToolRunning(tool, url) {
  if (!tool || !tool.async || !window.VaultJobs) return false;
  const jobs = window.VaultJobs.state.byTool(tool.id);
  return jobs.some((j) => {
    if (window.VaultJobs.isTerminal(j.status)) return false;
    const jurl = (j.params && j.params.url) || '';
    return aseNormUrl(jurl) === aseNormUrl(url);
  });
}

function buildAttackSurfaceHtml(d) {
  const s = aseState();
  const sum = d.summary || {};
  const overall = sum.overall || 'none';

  const summaryCards = [
    ['Core', escapeHtml(sum.core || '—')],
    ['Plugins', String(sum.plugins ?? 0)],
    ['Themes', String(sum.themes ?? 0)],
    ['REST routes', String(sum.rest_routes ?? 0)],
    ['Auth risks', String(sum.authentication ?? 0)],
    ['Sensitive files', String(sum.sensitive_files ?? 0)],
    ['Known vulns', String(sum.known_vulns ?? 0)],
    ['Total findings', String(sum.total_findings ?? 0)],
  ]
    .map(
      ([l, v]) =>
        `<div class="ase-sum-card"><span>${l}</span><b>${v}</b></div>`
    )
    .join('');

  const sevChips = ASE_SEVERITIES.map(
    ([v, l]) =>
      `<button type="button" class="ase-chip ${s.severity === v ? 'active' : ''}" data-ase-sev="${v}">${l}</button>`
  ).join('');
  const catChips = ASE_CATEGORIES.map(
    ([v, l]) =>
      `<button type="button" class="ase-chip ${s.category === v ? 'active' : ''}" data-ase-cat="${v}">${l}</button>`
  ).join('');

  const nodes = (d.nodes || [])
    .map((n) => aseNodeHtml(n))
    .filter(Boolean)
    .join('');

  const missing = (d.missing || [])
    .map((m) => {
      const tool = window.VAULT_TOOLS.getTool(m.tool);
      const desc = tool ? tool.desc : '';
      const running =
        s.pending.has(m.tool) || (tool && tool.async && aseToolRunning(tool, d.url));
      const btn = running
        ? `<button type="button" class="btn-ghost btn-xs" disabled><span class="ase-run-spin"></span> Running…</button>`
        : `<button type="button" class="btn-ghost btn-xs" data-ase-run-missing="${escapeHtml(m.tool)}">Run</button>`;
      return `<div class="ase-missing-row ${running ? 'running' : ''}">
        <span class="ase-missing-name">${escapeHtml(m.label || m.tool)}</span>
        <span class="ase-missing-desc">${escapeHtml(desc)} — not run yet for this target</span>
        ${btn}
      </div>`;
    })
    .join('');

  return `
    <div class="ase" data-ase-root>
      <div class="ase-summary">
        <div class="ase-summary-top">
          <span class="ase-overall sev-${escapeHtml(overall)}">Overall: ${escapeHtml(overall)}</span>
          <span class="ase-url mono" title="${escapeHtml(d.url)}">${escapeHtml(d.url)}</span>
          <span class="ase-meta">generated ${escapeHtml(String(d.generated_at || '').replace('T', ' ').slice(0, 19))}</span>
          <button type="button" class="btn-ghost btn-xs" data-ase-refresh>↻ Refresh</button>
        </div>
        <div class="ase-summary-grid">${summaryCards}</div>
      </div>
      <div class="ase-controls">
        <input class="ase-search" type="search" placeholder="Search nodes, components, CVEs…" value="${escapeHtml(s.query)}" data-ase-search>
        <div class="ase-chip-row">${sevChips}</div>
        <div class="ase-chip-row">${catChips}</div>
        <div class="ase-bulk">
          <button type="button" class="btn-ghost btn-xs" data-ase-expand-all>Expand all</button>
          <button type="button" class="btn-ghost btn-xs" data-ase-collapse-all>Collapse all</button>
        </div>
      </div>
      <div class="ase-tree">${nodes || '<div class="tool-empty">No nodes match the current filters.</div>'}</div>
      ${missing ? `<div class="ase-missing"><div class="ase-missing-title">Sources not yet scanned for this target</div>${missing}</div>` : ''}
    </div>`;
}

function aseNodeHtml(n) {
  const s = aseState();
  const sev = s.severity;
  const cat = s.category;
  const q = s.query.trim().toLowerCase();

  if (cat !== 'all' && n.category !== cat) return '';

  const items = (n.items || []).filter((i) => {
    if (sev !== 'all' && i.severity !== sev) return false;
    if (q) {
      const hay =
        `${i.label} ${i.value || ''} ${i.detail || ''} ` +
        (i.meta || []).map(([k, v]) => `${k} ${v}`).join(' ');
      if (!hay.toLowerCase().includes(q)) return false;
    }
    return true;
  });
  if (!items.length) return '';

  const open = s.expanded.has(n.id) || s.query.trim() !== '';
  const icon = ASE_ICONS[n.id] || '•';
  const rows = items.map(aseItemHtml).join('');

  return `
    <div class="ase-node ${open ? 'open' : ''}" data-ase-node data-node-id="${escapeHtml(n.id)}">
      <button type="button" class="ase-node-head" data-ase-toggle data-node="${escapeHtml(n.id)}">
        <span class="ase-chev">▸</span>
        <span class="ase-node-icon">${icon}</span>
        <span class="ase-node-label">${escapeHtml(n.label)}</span>
        <span class="ase-node-note">${escapeHtml(n.note)}</span>
        <span class="ase-node-count">${items.length}</span>
        <span class="ase-node-status sev-${escapeHtml(n.status)}">${escapeHtml(n.status)}</span>
      </button>
      <div class="ase-node-body" ${open ? '' : 'hidden'}>${rows}</div>
    </div>`;
}

function aseItemHtml(i) {
  const meta = (i.meta || []).length
    ? `<details class="ase-item-meta"><summary>details</summary><div class="ase-meta-grid">${(i.meta || [])
        .map(
          ([k, v]) =>
            `<div class="ase-meta-kv"><span>${escapeHtml(k)}</span><b>${escapeHtml(v)}</b></div>`
        )
        .join('')}</div></details>`
    : '';
  return `
    <div class="ase-item sev-${escapeHtml(i.severity)}">
      <span class="ase-item-sev">${escapeHtml(i.severity)}</span>
      <div class="ase-item-main">
        <div class="ase-item-label">${escapeHtml(i.label)}</div>
        ${i.value ? `<div class="ase-item-value mono">${escapeHtml(i.value)}</div>` : ''}
      </div>
      <div class="ase-item-right">
        ${i.detail ? `<div class="ase-item-detail">${escapeHtml(i.detail)}</div>` : ''}
        ${meta}
      </div>
    </div>`;
}
