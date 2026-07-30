# WordPress Security Research

> Focused resources for advanced WordPress vulnerability research and bug bounty hunting.

## Contents

| # | File | Description |
|---|------|-------------|
| 01 | `01-recon-enumeration.md` | WordPress recon, version detection, user enumeration, plugin/theme fingerprinting |
| 02 | `02-core-vulnerabilities.md` | WordPress core CVEs — SQLi, RCE, Auth Bypass, XSS, CSRF, Deserialization |
| 03 | `03-plugin-theme-vulns.md` | Plugin & theme vulnerability research methodology |
| 04 | `04-exploit-development.md` | Writing WordPress exploits — PoC chains, REST API abuse, WAF bypass |
| 05 | `05-tools-resources.md` | Tools, scanners, GitHub repos, research papers, CVE databases |

## Quick Reference

- **Current WordPress version**: 7.0.2 (as of July 2026)
- **REST API namespace**: `/wp-json/wp/v2/`
- **Batch API**: `/wp-json/batch/v1`
- **XML-RPC**: `/xmlrpc.php`
- **Key files**: `wp-config.php`, `.htaccess`, `readme.html`, `license.txt`
- **Common plugin dirs**: `/wp-content/plugins/`, `/wp-content/themes/`, `/wp-content/uploads/`
