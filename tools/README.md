# Local security tools

Installed CLIs + Vault UI wrappers. **Each tool is modular** — delete one without touching the rest.

## Categories

| Category | Tools | What for |
|----------|--------|----------|
| **Recon** | Subdomain Enum (subfinder), Archive URLs (waybackurls), Crawler (katana), JS Analysis (builtin) | Asset discovery, surface mapping |
| **WordPress** | Version Check, User Enum, Plugin Enum, Theme Enum, XML-RPC Probe, Sensitive Paths, REST Surface, WP Nuclei Scan, **WF Vuln Scanner** (Wordfence Intelligence) | Full WP recon + CVE matching |
| **Websites** | Live Probe (httpx), Vuln Scan (nuclei), Path Fuzz (ffuf), CORS Check (builtin), Open Redirect (builtin), **Security Headers & Cookies** (builtin) | Target websites |
| **Local Files** | Secrets (gitleaks), FS Vuln (trivy) | Scan a folder/repo on disk |

Builtin tools (JS Analysis, CORS Check, Open Redirect, all WordPress HTTP tools, WF Vuln Scanner) need **no external binaries**.

## Installed binaries

```bash
brew install nuclei httpx ffuf gitleaks trivy
nuclei -update-templates   # first time / periodically
```

| Binary | Used by |
|--------|---------|
| `nuclei` | Vuln Scan, WP Nuclei Scan |
| `httpx` | Live Probe |
| `ffuf` | Path Fuzz |
| `gitleaks` | Secrets Scan |
| `trivy` | FS Vuln Scan |

### Recon binaries (Go)

```bash
go install github.com/projectdiscovery/subfinder/v2/cmd/subfinder@latest
go install github.com/tomnomnom/waybackurls@latest
go install github.com/projectdiscovery/katana/cmd/katana@latest
```

> Add `$(go env GOPATH)/bin` to `PATH` so the server finds them.

> **waybackurls note:** the stock build hardcodes `http://` for the CDX API. This Vault expects the **https-patched** build (CDX is unreachable/flaky over plain http on many networks). Patch `main.go` (both CDX URLs → `https://`) and rebuild, or vendor the binary at `~/go/bin/waybackurls`.

## Wordlist

`tools/wordlists/common-paths.txt` — small default list for ffuf. Replace with SecLists for serious fuzzing.

## Delete a tool (easy)

See **[DELETE.md](./DELETE.md)**.

## Legal

Only scan systems you own or have permission to test.
