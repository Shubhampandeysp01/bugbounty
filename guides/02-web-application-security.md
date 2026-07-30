# Web Application Security

## Server-Side Request Forgery (SSRF)

### Attack Vectors
- **Basic SSRF**: Access internal services via `localhost`, `127.0.0.1`, `169.254.169.254`
- **Cloud Metadata Attacks**: AWS `169.254.169.254`, Azure `169.254.169.254`, GCP metadata
- **DNS Rebinding**: Bypass hostname validation by resolving to different IPs
- **Blind SSRF**: Out-of-band detection using Collaborator / interactsh
- **Second-Order SSRF**: Stored URL fetched later by internal component
- **PDF Generator SSRF**: Inject `<iframe>` or JS into generated PDFs for internal network access

### Bypass Techniques
- URL parsing inconsistencies: `http://127.0.0.1#@evil.com`, `http://evil.com:80@127.0.0.1`
- DNS over HTTPS: `http://127.0.0.1.nip.io/`
- IPv6: `http://[::1]/`, `http://[0:0:0:0:0:ffff:7f00:1]/`
- Redirect bypass: Use open redirect on trusted domains
- Protocol smuggling: `file:///`, `gopher://`, `dict://`, `ftp://`

### Key Research
- "Where URLs Become Weapons: Automated Discovery of SSRF Vulnerabilities" - IEEE S&P 2024
- "SSRFReaper: An SSRF Vulnerability Discovery Technique" - ACM 2026

## Deserialization Attacks

### Java
- **ysoserial**: CommonsCollections, FastJson, Jackson, XStream gadget chains
- **JNDI Injection**: log4shell (CVE-2021-44228)
- **JRMP / JNDI Bypass**: Bypass JEP-290 via JDK 8u20

### PHP
- **PHP Object Injection**: `unserialize()` with POP chains
- **Phar Deserialization**: `phar://` wrapper triggers deserialization on `file_exists()`, `is_dir()`

### Python
- **Pickle**: Malicious `__reduce__` for RCE
- **PyYAML**: `yaml.load()` without `Loader=yaml.SafeLoader`
- **Flask / Django**: Session deserialization

### .NET / JavaScript
- **ViewState**: MachineKey brute-force / validationKey leak
- **Node.js**: `node-serialize`, `prototype-pollution` leading to RCE

## API Security
- **Mass Assignment**: Extra parameters in JSON body (e.g., `"isAdmin": true`)
- **Rate Limiting Bypass**: Race conditions, IP rotation, header manipulation
- **GraphQL Injection**: Introspection, batching attacks, N+1 DoS
- **JWT Attacks**: `alg: none`, RS256->HS256 confusion, KID injection
- **OAuth / OIDC Flows**: CSRF on redirect_uri, stolen authorization codes

## SQL Injection (Advanced)
- **Out-of-Band SQLi**: DNS/HTTP exfiltration
- **Second-Order SQLi**: Stored injection executed later
- **WAF Bypass**: Encoding, heavy nesting, HTTP parameter pollution
- **NoSQL Injection**: MongoDB $where/$ne injection, blind NoSQL regex

## Tools
- **Burp Suite Pro**: Repeater, Intruder, Scanner, Collaborator
- **Caido**: Modern web proxy alternative
- **ffuf / wfuzz**: Fuzzing for endpoints and parameters
- **Nuclei**: Template-based vulnerability scanning
- **Amass / Subfinder**: Subdomain enumeration
- **Katana / gospider**: Crawling

## GitHub Learning Links
- [PortSwigger Academy](https://portswigger.net/web-security) — free interactive labs by vulnerability class
- [swisskyrepo/PayloadsAllTheThings](https://github.com/swisskyrepo/PayloadsAllTheThings) — payloads & bypasses
- [Hacker0x01/hacker101](https://github.com/Hacker0x01/hacker101) — HackerOne free training
- [projectdiscovery/nuclei-templates](https://github.com/projectdiscovery/nuclei-templates) — detection templates to study patterns
- [OWASP/CheatSheetSeries](https://github.com/OWASP/CheatSheetSeries) — defensive & offensive checklists
- [digininja/DVWA](https://github.com/digininja/DVWA) — local vulnerable web app (lab only)
- [juice-shop/juice-shop](https://github.com/juice-shop/juice-shop) — modern OWASP vulnerable app
