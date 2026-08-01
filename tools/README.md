# Local security tools

Installed CLIs (Homebrew) + Vault UI wrappers. **Each tool is modular** — delete one without touching the rest.

## Categories

| Category | Tools | What for |
|----------|--------|----------|
| **WordPress** | Version Check, User Enum, Plugin Enum, Theme Enum, XML-RPC Probe, Sensitive Paths, REST Surface, WP Nuclei Scan, **WF Vuln Scanner** (Wordfence Intelligence) | Full WP recon + CVE matching |
| **Websites** | Live Probe (httpx), Vuln Scan (nuclei), Path Fuzz (ffuf) | Target websites |
| **Local Files** | Secrets (gitleaks), FS Vuln (trivy) | Scan a folder/repo on disk |

## Installed binaries

```bash
brew install nuclei httpx ffuf gitleaks trivy
nuclei -update-templates   # first time / periodically
```

| Binary | Version (when installed) | Used by |
|--------|--------------------------|---------|
| `nuclei` | 3.x | Vuln Scan |
| `httpx` | 1.x | Live Probe |
| `ffuf` | 2.x | Path Fuzz |
| `gitleaks` | 8.x | Secrets Scan |
| `trivy` | 0.x | FS Vuln Scan |

WordPress Version Check is **built into the Rust server** (no extra binary).

## Wordlist

`tools/wordlists/common-paths.txt` — small default list for ffuf. Replace with SecLists for serious fuzzing.

## Delete a tool (easy)

See **[DELETE.md](./DELETE.md)**.

## Legal

Only scan systems you own or have permission to test.
