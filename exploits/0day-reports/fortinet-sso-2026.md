# FortiCloud SSO Authentication Bypass (CVE-2026-24858)

## Status
- **CVE**: CVE-2026-24858
- **CVSS**: 8.1 (High)
- **Status**: ⚠️ **Actively exploited in the wild** — bypass of CVE-2025-59718 and CVE-2025-59719
- **Affected**: FortiOS, FortiGate, FortiProxy
- **Component**: FortiCloud SSO — SAML signature verification
- **Patch**: Jan 2026 (incomplete — this bypasses prior fixes)

## Root Cause
**Incomplete fix** for CVE-2025-59718 and CVE-2025-59719. The SAML message signature verification in FortiCloud SSO is still insufficient. An attacker can craft a malicious SAML message that bypasses SSO authentication.

## Exploitation
1. Attacker targets FortiGate/FortiProxy device with FortiCloud SSO enabled
2. Sends crafted SAML message to the SSO endpoint
3. SAML signature verification is bypassed (crypto implementation error)
4. Attacker authenticates as any user (including admin)
5. Full device access — firewall rules can be modified, VPN accessed

## Attack Vector Detail
- "Attacker register" flow — when an admin registers a device to FortiCare from GUI, unless the toggle "Allow administrative login using FortiCloud SSO" is manually disabled, FortiCloud SSO becomes enabled by default
- SAML messages with forged signatures pass validation

## Detection
- Monitor device logs for anomalous SSO authentication events
- Check if FortiCloud SSO is enabled on your FortiGate
- Look for unexpected admin accounts or login times
- CISA KEV entry indicates active exploitation

## Mitigation
- **Patch**: Apply Fortinet's latest firmware for your device
- **Workaround**: Disable FortiCloud SSO if not needed
- **Check**: Verify "Allow administrative login using FortiCloud SSO" toggle is OFF after registration
- **Hardening**: Use local authentication with strong MFA instead of SSO

## Pattern Recognition
- **Incomplete patch syndrome**: CVE fixed → bypass found → new CVE → newer bypass — this is the 3rd iteration
- SAML crypto bugs are notoriously difficult to fix correctly
- "Enabled by default" — the SSO toggle defaults to ON after registration
- FortiOS remains one of the most targeted edge device platforms

## References
- Fortinet PSIRT: fortiguard.com/psirt
- 0day.cz database
- CISA KEV: cisa.gov/known-exploited-vulnerabilities-catalog
