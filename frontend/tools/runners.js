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
      if (!v) {
        primary.focus();
        return;
      }
      params.set(tool.input.name, v);
    }
    formEl.querySelectorAll('[data-extra]').forEach((el) => {
      const v = el.value.trim();
      if (v) params.set(el.dataset.extra, v);
    });

    resultsEl.classList.add('hidden');
    resultsEl.innerHTML = '';
    spinnerEl.classList.remove('hidden');

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
        list = `<div class="findings-list">${items
          .map((p) => {
            const ver = p.version ? ` v${p.version}` : '';
            return `<div class="finding-row sev-info">
              <span class="finding-sev">${escapeHtml(p.evidence || 'plugin')}</span>
              <div class="finding-body">
                <div class="finding-name mono">${escapeHtml(p.slug)}${escapeHtml(ver)}</div>
                <div class="finding-meta">${escapeHtml(p.path || '')} · HTTP ${escapeHtml(String(p.status || ''))}</div>
              </div>
            </div>`;
          })
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
        list = `<div class="findings-list">${items
          .map((t) => {
            const label = t.theme_name
              ? `${t.theme_name} (${t.slug})`
              : t.slug;
            const ver = t.version ? ` v${t.version}` : '';
            return `<div class="finding-row sev-info">
              <span class="finding-sev">${escapeHtml(t.evidence || 'theme')}</span>
              <div class="finding-body">
                <div class="finding-name">${escapeHtml(label)}${escapeHtml(ver)}</div>
                <div class="finding-meta mono">${escapeHtml(t.path || '')}</div>
              </div>
            </div>`;
          })
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
        vulnList = `<p class="tool-note">Vulnerabilities — click a row for full details</p>
          <div class="findings-list vuln-short-list">${findings
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
              const score =
                f.cvss_score != null ? Number(f.cvss_score).toFixed(1) : '—';
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

    trivy(data) {
      if (!data.installed) {
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

    generic(data) {
      return (
        metaBar(data) +
        (data.error ? `<div class="tool-error">${escapeHtml(data.error)}</div>` : '') +
        rawBlock(JSON.stringify(data, null, 2))
      );
    },
  },
};

function escapeHtml(s) {
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
