# How to remove a tool cleanly

Each tool is three places max. Remove all matching rows.

## Map

| Tool ID | Server module | Frontend registry | Binary (optional) |
|---------|---------------|-------------------|-------------------|
| `subfinder-enum` | `server/src/tools/subfinder.rs` | registry entry | `subfinder` |
| `waybackurls-mine` | `server/src/tools/waybackurls.rs` | registry entry | `waybackurls` |
| `katana-crawl` | `server/src/tools/katana.rs` | registry entry | `katana` |
| `js-analysis` | `server/src/tools/js_analysis.rs` | registry entry | _(builtin)_ |
| `wordpress-check` | `server/src/tools/wordpress.rs` | registry entry | _(builtin)_ |
| `wordpress-users` | `server/src/tools/wordpress_users.rs` | registry entry | _(builtin)_ |
| `wordpress-plugins` | `server/src/tools/wordpress_plugins.rs` | registry entry | _(builtin)_ |
| `wordpress-themes` | `server/src/tools/wordpress_themes.rs` | registry entry | _(builtin)_ |
| `wordpress-xmlrpc` | `server/src/tools/wordpress_xmlrpc.rs` | registry entry | _(builtin)_ |
| `wordpress-paths` | `server/src/tools/wordpress_paths.rs` | registry entry | _(builtin)_ |
| `wordpress-rest` | `server/src/tools/wordpress_rest.rs` | registry entry | _(builtin)_ |
| `wordpress-nuclei` | `server/src/tools/wordpress_nuclei.rs` | registry entry | `nuclei` |
| `wordpress-vuln-scan` | `server/src/tools/wordpress_vuln_scan.rs` | registry entry | Wordfence API key + local DB |
| `httpx-probe` | `server/src/tools/httpx.rs` | registry entry | `httpx` |
| `nuclei-scan` | `server/src/tools/nuclei.rs` | registry entry | `nuclei` |
| `ffuf-fuzz` | `server/src/tools/ffuf.rs` | registry entry | `ffuf` |
| `cors-check` | `server/src/tools/cors_check.rs` | registry entry | _(builtin)_ |
| `open-redirect` | `server/src/tools/open_redirect.rs` | registry entry | _(builtin)_ |
| `gitleaks-scan` | `server/src/tools/gitleaks.rs` | registry entry | `gitleaks` |
| `trivy-scan` | `server/src/tools/trivy.rs` | registry entry | `trivy` |

## Steps (example: remove Path Fuzz / ffuf)

1. Delete `server/src/tools/ffuf.rs`
2. In `server/src/tools/mod.rs`:
   - remove `pub mod ffuf;`
   - remove the `.route("/api/tools/ffuf", ...)` line
3. In `server/src/tools/status.rs` remove the `("ffuf-fuzz", ...)` catalog row
4. In `frontend/tools/registry.js` remove the `ffuf-fuzz` object
5. In `frontend/tools/runners.js` remove the `ffuf` renderer
6. Rebuild server: `cd server && cargo build --release`
7. Optional: `brew uninstall ffuf`
8. Optional: remove wordlist if unused: `tools/wordlists/`

> `status.rs` is the tool catalog — **always** remove the row there too, or the status endpoint will still list the tool.

## Uninstall all CLIs

```bash
brew uninstall nuclei httpx ffuf gitleaks trivy
# and/or
rm "$(go env GOPATH)/bin"/subfinder "$(go env GOPATH)/bin"/waybackurls "$(go env GOPATH)/bin"/katana
```

Builtin tools still work after that (no external dep).
