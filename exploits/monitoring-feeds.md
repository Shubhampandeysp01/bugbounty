# Live 0-Day Monitoring & Threat Intelligence Feeds

## Purpose
Stay current with active zero-day exploitation and vulnerability disclosure. These feeds cover exploit announcements, CVE disclosures, in-the-wild exploitation reports, and threat intelligence.

---

## 🔴 Critical Feeds (Check Daily)

### CISA Known Exploited Vulnerabilities (KEV) Catalog
- **URL**: cisa.gov/known-exploited-vulnerabilities-catalog
- **Format**: CSV (`/sites/default/files/csv/known_exploited_vulnerabilities.csv`), JSON
- **Contains**: All vulnerabilities confirmed as actively exploited in the wild
- **Why**: If CISA adds it, it's under active attack — patch immediately

### Google Project Zero — 0-days In-the-Wild
- **URL**: googleprojectzero.github.io/0days-in-the-wild/
- **Content**: Full Root Cause Analyses (RCAs) of every 0-day tracked being exploited in the wild
- **Why**: Best technical writeups in the industry

### Google TAG — Zero-Day Review
- **URL**: cloud.google.com/blog/topics/threat-intelligence
- **Content**: Annual 0-day exploitation summary with vendor/country breakdowns
- **2025 result**: 90 zero-days exploited in the wild (48% targeting enterprise)

---

## 📡 Real-Time Feeds

### Automated CVE Alerting
| Service | URL | Notes |
|---------|-----|-------|
| **OpenCVE** | opencve.io | Self-hosted CVE monitoring (free) |
| **NVD Feeds** | nvd.nist.gov/vuln/data-feeds | RSS/JSON for all CVEs |
| **0day.cz** | zero-day.cz/database/ | Zero-day-specific database |
| **VulDB** | vuldb.com | Dashboard of active exploitation |
| **Vulert** | vulert.com | Open-source vulnerability monitoring |

### Vendor Security Pages
| Vendor | URL | Check For |
|--------|-----|-----------|
| **Microsoft MSRC** | msrc.microsoft.com/update-guide | Patch Tuesday + advisories |
| **Google Chrome** | chromereleases.googleblog.com | In-the-wild zero-days |
| **Apple Security** | support.apple.com/en-us/HT201222 | iOS/macOS/watchOS patches |
| **Android Security** | source.android.com/docs/security/bulletin | Monthly + in-the-wild |
| **Mozilla** | mozilla.org/security/advisories | Firefox/Thunderbird |
| **Cisco PSIRT** | tools.cisco.com/security/center/publicationListing.x | Network device vulns |
| **Fortinet PSIRT** | fortiguard.com/psirt | FortiGate/FortiOS |
| **Palo Alto** | security.paloaltonetworks.com | PAN-OS advisories |
| **Cloudflare** | blog.cloudflare.com/tag/disclosure | DDoS/mitigation data |
| **OpenSSL** | openssl.org/news/vulnerabilities.html | Crypto library vulns |

### Pwn2Own / Exploit Contests
| Event | URL | Notes |
|-------|-----|-------|
| **Pwn2Own Vancouver** | thezdi.com | Browser/OS/VM targets |
| **Pwn2Own Tokyo** | thezdi.com | Mobile/IoT targets |
| **Tianfu Cup** | tianfucup.cn | Chinese exploitation contest |
| **Pwn2Own Automotive** | thezdi.com | Vehicle exploitation |
| **Pwn2Own Ireland** | thezdi.com | Industrial/SCADA targets |

---

## 🧠 Intelligence Sources

### Threat Research Blogs
| Blog | Focus |
|------|-------|
| **Mandiant** (cloud.google.com/blog/topics/threat-intelligence) | APT campaigns, in-the-wild 0-days |
| **Unit 42** (unit42.paloaltonetworks.com) | Cloud/network/container threats |
| **Talos** (blog.talosintelligence.com) | Cisco threat research |
| **Elastic Security** (elastic.co/security-labs) | EDR/malware research |
| **Trend Micro ZDI** (thezdi.com/blog) | Pwn2Own + disclosed 0-days |
| **OffSec / Exploit-DB** (exploit-db.com) | Public exploit archive |
| **SANS ISC** (isc.sans.edu) | Internet Storm Center daily diary |
| **Praetorian** (praetorian.com/blog) | Offensive security research |
| **Volexity** (volexity.com/blog) | APT threat hunting |
| **Symantec** (symantec-enterprise-blogs.security.com) | Global threat intel |

### Mailing Lists
| List | Subscribe |
|------|-----------|
| **full-disclosure** | lists.grok.org.uk |
| **bugtraq** | seclists.org/bugtraq |
| **oss-security** | openwall.com/lists/oss-security |
| **linux-distros** | openwall.com/lists/linux-distros |
| **Daily Dave** | x.com/bwya77 (blog format) |
| **SANS NewsBites** | sans.org/newsletters |

### Telegram Channels
- **0day Alert**: Real-time 0-day CVE alerts
- **CVE Alerts**: Automated CVE feed  
- **Exploit Database Bot**: Auto-posts new exploit-db entries
- **Threat Intel**: APT and targeted attack alerts
- **VulnAlert**: Vulnerability announcements with PoC
- **Ransomware Watch**: Ransomware group tracking

---

## 📊 Vulnerability Statistics 2026

### Key Metrics (from Google TAG, Mandiant, CrowdStrike, NVD)
| Metric | Value | Source |
|--------|-------|--------|
| CVEs published 2025 | 48,185 (+20.6% YoY) | NVD/JerryGamblin |
| 0-days exploited in wild 2025 | 90 | Google TAG |
| Enterprise 0-day share | 48% (all-time high) | Google TAG |
| Exploitation before public disclosure | 42% | CrowdStrike 2026 |
| KEV catalog entries | 1,484 | CISA |
| % of breaches via vuln exploitation | 20% (+34% YoY) | Verizon DBIR 2025 |
| Edge device exploitation increase | 8x growth | Verizon DBIR |
| IAB-to-ransomware handoff time | 22 seconds avg | Mandiant M-Trends 2026 |

---

## 📚 Additional Resources

### Detection and Response
- **Sigma rules**: github.com/SigmaHQ/sigma
- **YARA rules**: github.com/YARA-Rules/rules
- **MITRE ATT&CK**: attack.mitre.org
- **Detection Engineering**: github.com/0xAnalytics/Detection-Engineering

### Data Sources
- **Shodan**: shodan.io — internet-connected device search
- **Censys**: censys.io — certificate/IP intelligence
- **GreyNoise**: greynoise.io — internet background noise filtering
- **Spur**: spur.us — contextual threat data
- **URLhaus**: urlhaus.abuse.ch — malicious URL tracking
- **AbuseIPDB**: abuseipdb.com — IP reputation
