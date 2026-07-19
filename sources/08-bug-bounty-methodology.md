# Bug Bounty Methodology

## Reconnaissance

### Passive Recon
- **Certificate Transparency**: crt.sh, certspotter — enumerate subdomains
- **DNS Dumpster**: DNS mapping and related domains
- **Wayback Machine**: Historical endpoints, archived files, forgotten API routes
- **Google Dorks**: Advanced search operators for exposed data
- **Shodan**: Internet-connected device/service enumeration
- **Censys**: Certificate and network reconnaissance
- **GitHub/GitLab**: Search for hardcoded credentials, API keys, internal configs

### Active Recon
- **Subdomain Enumeration**: Subfinder, Amass, Assetfinder, Findomain
- **Port Scanning**: masscan (fast), nmap (detailed), RustScan
- **Web Crawling**: Katana, gospider, hakrawler, burp spider
- **Content Discovery**: ffuf, dirsearch, gobuster for hidden files/dirs
- **Parameter Fuzzing**: Arjun, ParamSpider, x8

### JS Analysis
- **LinkFinder**: Extract endpoints from JS files
- **JSParser**: Parse JS for API routes
- **SecretFinder**: Find API keys, tokens in JS
- **JSMon**: Monitor JS for changes
- **jsubfinder**: Find subdomains in JavaScript

## Vulnerability Assessment

### Automation
- **Nuclei**: YAML template-based scanning (fastest growing tool)
- **Nmap NSE**: Script-based service scanning
- **Metasploit**: Module-based exploitation framework
- **Burp Scanner**: Automated web scanning

### Manual Testing
- **Feature analysis**: Map each feature, identify user roles, data flows
- **Logic bugs**: Business logic errors, race conditions, TOCTOU
- **Access control**: IDOR, privilege escalation via role manipulation
- **Input validation**: Injection points, file upload, SSRF
- **State confusion**: Payment manipulation, coupon abuse, multi-step flows

## Advanced Hunting Techniques

### Open-Source Intelligence (OSINT)
- Weaponize historical data from Wayback Machine
- Detect forgotten endpoints from decade-old website versions
- Discover unpatched issues in old forums/developer repositories

### Race Condition Exploitation
- **Last-byte sync**: Synchronize requests at last byte for simultaneous arrival
- **Single-packet attack**: HTTP/2 single packet with multiple requests
- **Turbo Intruder**: Burp Extension for race condition testing
- Common targets: Coupon/point abuse, like/unlike, vote manipulation

### Third-Party Dependency Attacks
- **Dependency Confusion**: Upload package with internal name to public registry
- **Typo-squatting**: Similar package names to popular libraries
- **Dependency chain analysis**: Check outdated vulnerable packages

### Business Logic Vulnerabilities
- **2FA bypass**: Race condition, backup code brute-force, OAuth token reuse
- **Email verification bypass**: Modify response, resend to different email
- **Payment bypass**: Negative amounts, currency manipulation, integer overflow
- **Rate limit bypass**: IP rotation, header manipulation, distributed requests

## Methodology Frameworks

### OWASP Web Security Testing Guide (WSTG)
Comprehensive testing methodology for web application security.

### PortSwigger Web Security Academy
Free interactive labs for every vulnerability class.

### Bug Hunting Roadmaps
- **The Bug Hunter's Methodology**: Jason Haddix
- **Recon to Master**: YouTube Series (STOK, InsiderPhD, NahamSec)
- **Exploit Notes**: exploit-notes.hdks.org — structured exploitation guides

## Tools Ecosystem

### Comprehensive Tools
- **ProjectDiscovery suite**: nuclei, httpx, subfinder, katana, naabu, chaos
- **TomNomNom tools**: waybackurls, httprobe, meg, unfurl, comb
- **hakluke tools**: hakrawler, hakrevdns, haktrails
- **Bash bunny / p4's tools**: Long list of recon and exploitation helpers

### Collaboration & Reporting
- **HackerOne**: Largest bug bounty platform
- **Bugcrowd**: Crowdsourced security testing
- **Intigriti**: European bounty platform
- **YesWeHack**: French/EU-based platform
- **Synack**: Private curated platform (invite-only)

## Key Resources
- **PortSwigger Research**: portswigger.net/research
- **Intigriti Bug Bytes**: Weekly newsletter with curated writeups
- **PentesterLand**: Weekly podcast/newsletter
- **InfoSec Write-ups**: Medium publication
- **Secuna Blog**: Advanced bug hunting techniques for 2025-2026
- **Bug Bounty Hunting Methodology 2026**: github.com/su6osec/Bug-Bounty-Hunting-Methodology-2026
