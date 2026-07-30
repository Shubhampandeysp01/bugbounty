# cPanel & WHM Authentication Bypass (CVE-2026-41940)

## Status
- **CVE**: CVE-2026-41940
- **CVSS**: 9.8 (Critical)
- **Status**: ⚠️ **Mass exploitation underway** — "Sorry" ransomware campaign
- **Disclosure**: cPanel, April 28, 2026 (exploited in wild since ~Feb 2026)
- **Affected**: cPanel & WHM all versions 11.40 → 11.136.0.4
- **Ransomware**: "Sorry" ransomware family — targets unpatched cPanel servers

## Root Cause
Authentication bypass in cPanel's authentication framework — attacker can bypass login checks and gain administrative access to cPanel/WHM without credentials. Exact mechanism TBD (vendor disclosure pending full analysis), but likely relates to session handling or API authentication validation.

## Exploitation
1. Attacker sends crafted HTTP request to cPanel/WHM login endpoint
2. Server incorrectly grants authenticated session without valid credentials
3. Attacker gains WHM root-level access
4. Deploys "Sorry" ransomware web shell or encrypts server data
5. Ransom note demands payment for decryption

## Detection
- Check IIS logs / Apache access logs for anomalous patterns to /cpanel/ or /whm/ endpoints
- Monitor for unauthorized WHM API calls
- Look for new files in /home/ directories with `.sorry` or `.locked` extensions
- CISA KEV added — correlation with known IPs/dropzones

## Mitigation
- **Patch**: cPanel version 11.136.0.5+ (released April 28, 2026)
- **Workaround**: Restrict WHM access by IP whitelist only
- **Emergency**: If unable to patch immediately, disable WHM API or move behind VPN
- **Recovery**: Do NOT pay ransom — restore from clean backup
- **Hardening**: Disable unused cPanel features, enforce strong passwords, 2FA where available

## Pattern Recognition
- Authentication bypass in control panels is a recurring pattern (Plesk, cPanel, Webmin all had variants)
- 0-day silently exploited for 2+ months before disclosure → detection gap
- Ransomware groups aggressively weaponize control panel bugs for scale
- "Sorry" ransomware specifically targets web hosting providers

## References
- cPanel official advisory: cpanel.net/security
- CISA KEV entry: cisa.gov/known-exploited-vulnerabilities-catalog
- Zero-Day Threat Report May 2026 (Carthage Electronics)
- BleepingComputer: "Sorry Ransomware hits unpatched cPanel servers"
