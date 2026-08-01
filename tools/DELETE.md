# How to remove a tool cleanly

Each tool is three places max. Remove all matching rows.

## Map

| Tool ID | Server module | Frontend registry | Binary (optional) |
|---------|---------------|-------------------|-------------------|
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
| `gitleaks-scan` | `server/src/tools/gitleaks.rs` | registry entry | `gitleaks` |
| `trivy-scan` | `server/src/tools/trivy.rs` | registry entry | `trivy` |

## Steps (example: remove Path Fuzz / ffuf)

1. Delete `server/src/tools/ffuf.rs`
2. In `server/src/tools/mod.rs`:
   - remove `pub mod ffuf;`
   - remove the `.route("/api/tools/ffuf", ...)` line
3. In `frontend/tools/registry.js` remove the `ffuf-fuzz` object
4. Rebuild server: `cd server && cargo build --release`
5. Optional: `brew uninstall ffuf`
6. Optional: remove wordlist if unused: `tools/wordlists/`

## Uninstall all CLIs

```bash
brew uninstall nuclei httpx ffuf gitleaks trivy
```

WordPress check still works after that (no brew dep).
