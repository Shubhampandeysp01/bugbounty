# 0-Day & Active Exploit Reports

## ⚠️ IMPORTANT NOTICE
These vulnerabilities are **actively exploited or recently disclosed with partial/no patch**.
This information is for **educational and defensive research only**. Do not use these for any illegal purpose.

## Current Active Exploits (as of July 2026)

| # | CVE | Product | Type | Status |
|---|-----|---------|------|--------|
| 1 | CVE-2026-41940 | cPanel & WHM | Auth Bypass (CVSS 9.8) | Mass exploitation ongoing; patch released but 1.5M+ servers targeted by "Sorry" ransomware |
| 2 | CVE-2026-2441 | Google Chrome (CSS) | RCE via crafted HTML | In-the-wild exploit confirmed; patched Feb 2026 |
| 3 | CVE-2025-48595 | Android Framework | Integer Overflow → EoP | "Limited targeted exploitation" per Google; June 2026 patch |
| 4 | CVE-2025-30080 | Windows Kernel | Privilege Escalation | Actively exploited before Nov 2025 patch |
| 5 | CVE-2026-21509 | Microsoft Office | OLE RCE | In-the-wild; patched Jan 2026 |
| 6 | CVE-2026-24858 | FortiOS | SAML SSO Authentication Bypass | Active exploitation; partial fix (bypass of previous patch) |
| 7 | CVE-2025-53770/53771 | Microsoft SharePoint | Deserialization + Header Spoofing → RCE Chain | "ToolShell" exploit chain; July 2025 patch |
| 8 | CVE-2025-6558 | Google Chrome (ANGLE) | Sandbox Escape | In-the-wild; July 2025 patch |
| 9 | CVE-2025-7775 | Citrix NetScaler | Memory Overflow → RCE | Unauthenticated network attack; August 2025 patch |
| 10 | CVE-2026-15410 | SonicWall SMA1000 | Active (CISA KEV) | Added to CISA KEV June 2026 |

## How to Use This Folder

Each file follows the same structure:
1. **Status overview** — patched? patch-available? no-patch?
2. **Root cause** — what made the code wrong
3. **Exploitation** — how it works step-by-step
4. **Detection** — IoCs and log analysis
5. **Mitigation** — workarounds if no patch exists
6. **Pattern recognition** — what to look for in code review

## CISA KEV Reference
The CISA Known Exploited Vulnerabilities Catalog is the **single best source** for verifying if a vulnerability is under active exploitation. Check it daily:
cisa.gov/known-exploited-vulnerabilities-catalog
