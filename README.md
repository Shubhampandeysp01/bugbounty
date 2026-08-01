# Bug Bounty Research Vault

> A curated security-research library **plus** a local web app for running recon & scanning tools. Built-in full-text search, a modular tool system, and no external SaaS.

Two paths in one UI:

1. **Learn** — searchable guides, references, and case studies
2. **Tools** — live scanners (recon, WordPress, web, local files)

---

## Architecture

```
┌─────────────┐   HTTP/JSON (localhost:3000)   ┌──────────────────────────┐
│   Browser   │ ◄─────────────────────────────► │      Rust server (axum)  │
│  frontend/  │                                 │           server/        │
│ vanilla JS  │                                 └────────────┬─────────────┘
└─────────────┘                                              │
                                    ┌─────────────────────────┼───────────────────┐
                                    ▼                         ▼                   ▼
                          ┌────────────────────┐   ┌───────────────────┐   ┌──────────────────┐
                          │  Knowledge layer   │   │  Tool layer        │   │  Local binaries   │
                          │  index + markdown  │   │  /api/tools/*      │   │  (opt. CLI tools) │
                          └────────────────────┘   └───────────────────┘   └──────────────────┘
```

**Server** (`server/`, Rust / axum / Tokio / Tantivy):
- Serves the static UI **and** a JSON API on `0.0.0.0:3000`.
- Builds a **Tantivy full-text index** of all `*.md` files at startup; a `notify` file watcher re-indexes on edits (thread-based, debounced).
- `/api/tree`, `/api/file`, `/api/search`, `/api/stats` — knowledge layer.
- `/api/tools/*` — one route per tool, merged from `tools::routes()`.
- **CORS is locked to `http://localhost:3000` / `127.0.0.1`** — the API can read local files and run scanners, so no arbitrary web page may call it.

**Frontend** (`frontend/`, vanilla JS, no build step):
- `app.js` — SPA routing (history-aware `nav()`), sidebar tree, ⌘K/Ctrl-K search, tool panes.
- `tools/registry.js` — single source of truth for tool catalog (categories, inputs, endpoints).
- `tools/runners.js` — per-tool result renderers.

**Tool layer** (`server/src/tools/`): each tool is **one module + one route + one registry entry** — modular by design, so tools can be deleted without touching the rest (see `tools/DELETE.md`).

**Two tool kinds:**

| Kind | How it runs | Needs |
|------|-------------|-------|
| **Builtin** | Native HTTP requests straight from the Rust server | network only |
| **CLI wrapper** | Spawns an external binary (never via shell) | binary on `PATH` |

External tools are spawned with `std`/`tokio` `Command` (no shell interpolation), `kill_on_drop`, and a per-tool timeout; URL/domain/path inputs are validated (character allow-list, length caps) before use.

---

## Structure

```
bugbounty/
├── start.sh                     ← launch (run from repo root)
├── frontend/                    ← UI (Learn + Tools), vanilla JS
│   ├── index.html / app.js / style.css
│   └── tools/registry.js        ← tool catalog
│       tools/runners.js         ← result renderers
├── server/                      ← Rust API (axum + tantivy)
│   └── src/
│       ├── main.rs              ← knowledge layer + watcher
│       └── tools/               ← one module per tool
│           ├── common.rs        ← HTTP client, run_cli, validation
│           ├── status.rs        ← install-status catalog
│           └── <tool>.rs        ← per-tool handlers
├── tools/
│   ├── README.md / DELETE.md    ← install notes / removal guide
│   ├── wordlists/               ← default ffuf list
│   └── data/wordfence/          ← local Wordfence DB (gitignored)
├── guides/  references/  case-studies/   ← learning content (markdown)
└── .secrets/                    ← API keys (gitignored)
```

---

## Quick start

```bash
cd bugbounty
cd server && cargo build --release && cd ..   # build (first time / after changes)
./start.sh                                    # serves http://localhost:3000
```

Optional CLI tools (the recon trio is Go-based):

```bash
brew install nuclei httpx ffuf gitleaks trivy
nuclei -update-templates                      # first time / periodically

# Recon binaries (Go):
go install github.com/projectdiscovery/subfinder/v2/cmd/subfinder@latest
go install github.com/tomnomnom/waybackurls@latest
go install github.com/projectdiscovery/katana/cmd/katana@latest
```

> **PATH note:** if you used `go install`, add `$(go env GOPATH)/bin` to `PATH` so the server can find the binaries.

### Constraints

- **One server at a time.** Tantivy's index uses a file lock — a second instance fails with `LockFailure(LockBusy)`. Fix: `lsof -i :3000 -t | xargs kill`, and `rm -f .search_index/.tantivy-*.lock` if a stale lock lingers.
- Server is **local-only by design** (CORS-restricted; reads local files, runs local scanners). Don't expose it publicly.
- **Only scan systems you own or have explicit permission to test.**

---

## Features

### Knowledge layer

- Markdown guides, references, and case studies rendered as HTML.
- **Full-text search** (Tantivy) over all `.md` files — ⌘K / Ctrl+K or the search box.
- **Live sidebar tree** with auto re-index when files change on disk.
- `/api/stats` — file counts by category.

Suggested learning path lives in `guides/` (methodology → web appsec → exploitation → cloud/mobile/kernel). WordPress-specific track in `guides/wordpress/`.

### Tool layer

20 tools across 4 categories. Builtin tools need **no binaries**; CLI tools show an install hint in the UI if the binary is missing.

| Category | Tools |
|----------|-------|
| **Recon** | Subdomain Enum, Archive URLs, Crawler, JS Analysis |
| **WordPress** | Version Check, User Enum, Plugin Enum, Theme Enum, XML-RPC Probe, Sensitive Paths, REST Surface, WP Nuclei Scan, WF Vuln Scanner |
| **Websites** | Live Probe, Vuln Scan, Path Fuzz, CORS Check, Open Redirect |
| **Local Files** | Secrets Scan, FS Vuln Scan |

---

## Tools — how they work & constraints

### Recon

| Tool | Kind | Input | How it works | Constraints |
|------|------|-------|--------------|-------------|
| **Subdomain Enum** (`subfinder`) | CLI | `url` = bare domain, optional `all=1` | Passive enumeration from public sources, dedup + sort | Slow sources bounded: `-timeout 10 -max-time 1` (~≤1 min). `all=1` needs provider API keys for best results. |
| **Archive URLs** (`waybackurls`) | CLI | `url` = bare domain | Queries Internet Archive CDX (`https://`, **patched from the stock `http://` build**), dedup + sort | Capped at 4,000 URLs. CDX is slow/flaky (~45s, occasional timeouts) — not fixable on our side. |
| **Crawler** (`katana`) | CLI | `url`, optional `depth` (default 2) | Crawl with JS execution enabled, `-kf all`, 15 workers | Capped at 3,000 URLs; 150s timeout. JS-heavy SPAs may need the JS Analysis tool instead. |
| **JS Analysis** | Builtin | `url` = page **or** direct `.js` | Fetches page + up to 30 scripts (2 MB each, 3 MB page), regex-mines endpoints, API routes (`/api/`, `/graphql`, `/wp-json`), and secrets (AWS, Google, OpenAI, GitHub, Slack, JWT, keys, Mongo/S3 URIs) | Responses truncated (300 endpoints / 40 secrets). Regex has **no lookaround** (Rust `regex` crate) so exotic patterns may be missed. Don't trust flagged "secrets" without verification. |

### WordPress

| Tool | Kind | How it works | Constraints |
|------|------|--------------|-------------|
| **Version Check** | Builtin | Probes `generator` meta, `/wp-json/`, `readme.html` | — |
| **User Enum** | Builtin | REST `/wp-json/wp/v2/users`, `?author=N` archives, post authors | — |
| **Plugin / Theme Enum** | Builtin | Probes popular paths + `readme.txt` / `style.css`, parses HTML | |
| **XML-RPC Probe** | Builtin | `system.listMethods`, multicall, pingback | |
| **Sensitive Paths** | Builtin | Backups, `debug.log`, `.git`, config dumps | |
| **REST Surface** | Builtin | Maps namespaces, flags risky routes | |
| **WP Nuclei Scan** | CLI | `nuclei` with WordPress tags | Needs `nuclei` + updated templates |
| **WF Vuln Scanner** | Builtin | Detects core/plugins/themes, matches **local** Wordfence DB (CVE, CVSS, remediation) | Needs a **Wordfence API key** + one-time DB refresh. Key: `export WORDFENCE_API_KEY='…'` or `.secrets/wordfence_api_key`. DB: `tools/data/wordfence/` (gitignored). |

### Websites

| Tool | Kind | How it works | Constraints |
|------|------|--------------|-------------|
| **Live Probe** (`httpx`) | CLI | Status, title, tech, IP, server | Needs `httpx` |
| **Vuln Scan** (`nuclei`) | CLI | Template-based scan (medium+ by default, no Interactsh) | Needs `nuclei` + templates; first run `nuclei -update-templates` |
| **Path Fuzz** (`ffuf`) | CLI | Directory fuzz with bundled `tools/wordlists/common-paths.txt` | Swap the wordlist for SecLists on real targets |
| **CORS Check** | Builtin | 6 origin probes (arbitrary, null, subdomain, prefix/suffix bypass, scheme swap) inspecting `Access-Control-Allow-Origin` / `Allow-Credentials` | Verdicts: ok/low/medium/high/critical. Reads headers only — confirm impact manually. |
| **Open Redirect** | Builtin | Appends 18 common redirect params (`url`, `redirect`, `next`, …) with `//evil.example`, reads 3xx `Location` | Flags any off-site redirect; verify manually (open redirects are often filtered by exact-match allowlists). |

### Local Files

| Tool | Kind | How it works | Constraints |
|------|------|--------------|-------------|
| **Secrets Scan** (`gitleaks`) | CLI | Scans a folder/git repo for secrets; default path `.` | Needs `gitleaks`. Paths are resolved against the repo root; `..` escapes are rejected |
| **FS Vuln Scan** (`trivy`) | CLI | `trivy fs` for HIGH/CRITICAL vulns, secrets, misconfigs | Needs `trivy`; first run downloads the DB (network). First run can be slow |

### Tool inputs & safety

- All CLI tools run via `Command` with **no shell**; arguments are constructed from validated input (charset + length checks).
- Builtin tools use a shared `reqwest` client with per-tool timeouts and a browser-like user agent.
- `normalize_url` adds `https://` if missing; `normalize_domain` strips scheme/path/port.
- Unknown/missing binaries surface a clear install hint in the UI (and in `/api/tools/status`).

---

## API summary

```
GET /api/tree           knowledge tree
GET /api/file?path=..   rendered markdown
GET /api/search?q=..    full-text search
GET /api/stats          file counts
GET /api/tools/status   install status of every tool
GET /api/tools/<tool>?<input>   run a tool (see registry for params)
```

Example: `curl -s "http://localhost:3000/api/tools/subfinder?url=example.com" | jq`

---

## Rebuild / restart

```bash
cd server && cargo build --release && cd ..
lsof -i :3000 -t | xargs kill   # stop old instance first
./start.sh
```

## Secrets & gitignore

Never commit: `.secrets/`, `tools/data/wordfence/`, `.env*`, `.search_index/`.

---

## Legal

Authorized use only. The tools here can actively scan, fuzz, and read local files — point them at systems you own or are explicitly permitted to test.

---

**Last updated:** August 2026
