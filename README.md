# Bug Bounty Research Vault

> A curated collection of security research resources **plus** a local web app for learning and running security tools.

Two paths when you open the site:

1. **Learn** — guides, references, case studies  
2. **Tools** — live scanners (WordPress recon, web probe/scan/fuzz, local secrets/vulns, Wordfence CVE matching)

---

## Quick start

```bash
# From the repo root
cd bugbounty

# Build the Rust server (first time / after code changes)
cd server && cargo build --release && cd ..

# Start (binds http://localhost:3000)
./start.sh
```

Open **http://localhost:3000**

### Only one server at a time

The search index (Tantivy) uses a **file lock**. If you start a second instance while one is already running, you get:

```text
Failed to build search index: LockFailure(LockBusy, ...)
```

**Fix:**

```bash
# Stop whatever is on port 3000
lsof -i :3000 -t | xargs kill

# If it still panics with nothing listening, remove stale locks:
rm -f .search_index/.tantivy-writer.lock .search_index/.tantivy-meta.lock

./start.sh
```

Do **not** run `./start.sh` twice without stopping the first process.

---

## Structure

```
bugbounty/
├── README.md
├── start.sh                     ← launches release server
├── frontend/                    ← Vault UI (Learn + Tools)
│   ├── index.html
│   ├── app.js
│   ├── style.css
│   └── tools/
│       ├── registry.js          ← tool catalog (categories)
│       └── runners.js           ← result renderers
├── server/                      ← Rust API (axum)
│   └── src/
│       ├── main.rs
│       └── tools/               ← one module per tool (easy to delete)
├── tools/
│   ├── README.md                ← tool install / categories
│   ├── DELETE.md                ← how to remove a tool cleanly
│   ├── wordlists/               ← ffuf default list
│   └── data/wordfence/          ← local Wordfence DB (gitignored)
├── .secrets/                    ← API keys (gitignored)
├── guides/                      ← learning guides
├── references/                  ← papers, books, feeds
└── case-studies/                ← legendary + 0-day reports
```

---

## Learn path

### Suggested order

| Step | Focus | Resource |
|------|-------|----------|
| 1 | Bug bounty fundamentals | `guides/01-bug-bounty-methodology.md` |
| 2 | Web application security | `guides/02-web-application-security.md` |
| 3 | GitHub learning resources | `references/03-github-learning-resources.md` |
| 4 | Binary exploitation | `guides/03-binary-exploitation.md` |
| 5 | Browser exploitation | `guides/04-browser-exploitation.md` |
| 6 | Cloud & container security | `guides/05-cloud-container-security.md` |
| 7 | Mobile security | `guides/06-mobile-security.md` |
| 8 | Kernel fuzzing | `guides/07-kernel-fuzzing.md` |
| 9 | Windows exploitation | `guides/08-windows-exploitation.md` |
| 10 | Linux kernel exploitation | `guides/09-linux-kernel-exploitation.md` |
| 11 | EDR bypass & defense evasion | `guides/10-edr-bypass-defense-evasion.md` |

### References & case studies

- **Books**: `references/02-essential-books.md`
- **Papers**: `references/01-research-papers.md`
- **GitHub / labs**: `references/03-github-learning-resources.md`
- **Monitoring feeds**: `references/04-monitoring-feeds.md`
- **Legendary exploits**: `case-studies/legendary/`
- **Recent 0-days**: `case-studies/0day-reports/`
- **WordPress track**: `guides/wordpress/README.md`

In the UI: use the sidebar tree, or **⌘K** / **Ctrl+K** to search.

---

## Tools path

Tools are grouped into **three categories** (not dozens of menus):

| Category | Tools |
|----------|--------|
| **WordPress** | Version Check, User Enum, Plugin Enum, Theme Enum, XML-RPC Probe, Sensitive Paths, REST Surface, WP Nuclei Scan, **WF Vuln Scanner** (Wordfence) |
| **Websites** | Live Probe (httpx), Vuln Scan (nuclei), Path Fuzz (ffuf) |
| **Local Files** | Secrets Scan (gitleaks), FS Vuln Scan (trivy) |

Each tool is **modular** — one server file + one registry entry. See **[tools/DELETE.md](tools/DELETE.md)** to remove a tool without breaking others.

Full install notes: **[tools/README.md](tools/README.md)**

### Optional CLI binaries (Homebrew)

```bash
brew install nuclei httpx ffuf gitleaks trivy
nuclei -update-templates   # first time / periodically
```

| Binary | Used by |
|--------|---------|
| `nuclei` | Websites → Vuln Scan, WordPress → WP Nuclei Scan |
| `httpx` | Websites → Live Probe |
| `ffuf` | Websites → Path Fuzz |
| `gitleaks` | Local Files → Secrets Scan |
| `trivy` | Local Files → FS Vuln Scan |

Builtin WordPress HTTP tools need **no** extra binaries.

### Wordfence Vulnerability Scanner (WF Vuln Scanner)

Detects WordPress **core / plugins / themes**, then matches versions against a **local** Wordfence Intelligence database (CVE, CVSS, remediation).

**API key** (first match wins):

1. Environment: `export WORDFENCE_API_KEY='…'`
2. File: `.secrets/wordfence_api_key` (chmod 600, gitignored)

**Local DB** (gitignored):

- `tools/data/wordfence/feed.json` — full feed  
- `tools/data/wordfence/meta.json` — update timestamp / count  

In the UI: open **Tools → WordPress → WF Vuln Scanner** → **Refresh vulnerability DB** to download/update the feed (Wordfence v3 API, Bearer auth). Then **Run** against a target URL.

```bash
# Status / refresh via API (optional)
curl -s http://localhost:3000/api/tools/wordpress-vuln-db/status | jq
curl -s http://localhost:3000/api/tools/wordpress-vuln-db/refresh | jq
curl -s "http://localhost:3000/api/tools/wordpress-vuln-scan?url=https://example.com" | jq
```

---

## Rebuild after code changes

```bash
cd server && cargo build --release && cd ..
# restart (stop old process first)
lsof -i :3000 -t | xargs kill
./start.sh
```

---

## Secrets & gitignore

Never commit:

- `.secrets/`
- `tools/data/wordfence/`
- `.env` / `.env.local`
- `.search_index/` (generated search index)

---

## Legal

Only scan systems you **own** or have **explicit permission** to test. These tools are for research and authorized assessment only.

---

**Last updated:** July 2026
