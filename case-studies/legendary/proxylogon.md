# ProxyLogon (CVE-2021-26855)

## Classification
- **CVE chain**: CVE-2021-26855 (SSRF auth bypass), CVE-2021-26857 (deserialization), CVE-2021-26858 (file write), CVE-2021-27065 (file write)
- **CVSS**: 9.8 (Critical) — pre-auth RCE chain
- **Type**: Exchange Server SSRF + Auth Bypass + Arbitrary File Write → Pre-auth RCE
- **Discoverer**: Orange Tsai (DEVCORE) + Volexity + MSTIC (Microsoft)
- **Wild Impact**: 250,000+ Exchange servers compromised within days; Chinese APT (HAFNIUM) exploited as 0-day since January 2021

## Root Cause

### CVE-2021-26855 — The Core SSRF
Exchange has a front-end proxy that routes requests to back-end services. The proxy uses **cookies** to determine the backend server URL. An attacker could craft a cookie containing an **arbitrary URL**, and the front-end would proxy the request to that URL — including to internal services.

```csharp
// Simplified from Exchange FrontEnd proxy
// The vulnerability: Cookie values are used directly as backend URLs

public string GetTargetBackEndServerUrl(HttpRequest request) {
    // The attacker sets cookie: "X-BEResource=localhost/ecp/DDI/DDIService.svc"
    var cookie = request.Cookies["X-BEResource"];
    // NO VALIDATION of the cookie value
    return cookie;  // "localhost/ecp/DDI/DDIService.svc" — INTERNAL URL!
}
```

### The Full Chain
1. **CVE-2021-26855** (SSRF): Pre-auth, any internet attacker can send forged cookies to bypass authentication and call Exchange PowerShell cmdlets as admin
2. **CVE-2021-26857** (Deserialization): Post-auth, insecure deserialization in Unified Messaging service → code execution as SYSTEM
3. **CVE-2021-26858/27065** (File Write): Post-auth, write arbitrary files to the server filesystem — attacker drops a web shell

## Exploit Chain

**Step 1: Auth Bypass**
```
GET /owa/auth/logon.aspx HTTP/1.1
Cookie: X-BEResource=localhost/ecp/DDI/DDIService.svc; X-BEBackEndUrl=localhost
```
The front-end proxy forwards to the back-end ECP (Exchange Control Panel) — attacker impersonates admin without credentials.

**Step 2: Get Admin SID**
Use the SSRF to enumerate Exchange servers and extract the administrator's Security Identifier (SID).

**Step 3: Construct Admin Session**
Use the extracted SID to forge Cookie: `X-Role=Admin; X-AdminSID=<extracted-sid>`

**Step 4: Exploit Post-Auth Vulns**
With admin access:
- **Option A (CVE-2021-27065)**: Modify OAB (Offline Address Book) VirtualDirectory properties → writes web shell to disk
- **Option B (CVE-2021-26858)**: Write files via mailbox export functionality

**Step 5: Access Web Shell**
`GET /ecp/<webshell>.aspx` — cmd.exe execution at SYSTEM level

**Step 6: Persistence**
- Dump mailboxes via Exchange PowerShell
- Install Cobalt Strike beacon
- Pivot to Active Directory → Domain Admin

## Why It's Legendary
- **Default vulnerable**: Every Exchange server was exploitable with no config change needed
- **No user interaction**: No phishing, no creds, just port 443 and a crafted HTTP request
- **Scale of compromise**: 250K+ servers within a week of disclosure (attackers had been exploiting as 0-day since January)
- **Government + enterprise + SMB**: Every org using on-prem Exchange was equally affected
- **Orange Tsai's methodology**: DEVCORE found this by understanding Exchange's internal architecture deeply, not by random fuzzing

## Pattern Recognition
1. **Front-end/back-end architecture flaws**: Any proxy that forwards based on user-controlled values
2. **Cookie-based routing**: X-BEResource / X-BEBackEndUrl style patterns
3. **Missing validation on internal routing**: The server "knew" the cookie should come from another Exchange server but never verified
4. **Chain of multiple low-severity bugs**: Individually, each CVE was less severe — combined they're catastrophic
5. **Default be default**: "Vulnerable by default" — not an opt-in feature, but always-on

## Variants
- **ProxyShell** (CVE-2021-34473 + CVE-2021-34523 + CVE-2021-31207): Pwn2Own 2021 winning chain — different but equivalent path to RCE
- **ProxyOracle** (CVE-2021-31196): Oracle padding oracle in Exchange OWA
- **ProxyToken** (CVE-2021-33766): Token replay on Exchange
- **ProxyNotShell** (CVE-2022-41040 + CVE-2022-41082): Server-Side Request Forgery in Exchange 2022

## Mitigation Status
- **Patch**: March 2021 security updates (KB5000871+)
- **Workaround**: URL Rewrite rules to block the SSRF (issued by Microsoft same day)
- **Detection**: IIS logs show anomalous cookie patterns, China Chopper webshells
- **2026 status**: Shodan still shows 20K+ unpatched Exchange servers

## PoC & References
- **Official site**: proxylogon.com (Orange Tsai / DEVCORE)
- **Full exploit chain**: github.com/herwonowr/exprolog (Python PoC)
- **Praetorian analysis**: praetorian.com/blog/reproducing-proxylogon-exploit (full reverse engineering)
- **Project Zero RCA**: googleprojectzero.github.io/0days-in-the-wild/0day-RCAs/2021/CVE-2021-26855.html
- **Google Cloud / Mandiant**: ProxyShell follow-up analysis
- **KrebsOnSecurity**: "HAFNIUM Breach of 30K+ US Organizations"
- **CISA Emergency Directive**: ED 21-02 (24-hour patch mandate)
