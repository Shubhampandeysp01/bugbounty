# WordPress Security Tools & Resources

## Scanners & Recon Tools

### WPScan (Essential)
```bash
# Install
gem install wpscan

# Basic scan
wpscan --url https://target.com --api-token YOUR_TOKEN

# Aggressive plugin detection
wpscan --url https://target.com --plugins-detection aggressive

# Enumerate users
wpscan --url https://target.com --enumerate u

# Enumerate vulnerable plugins
wpscan --url https://target.com --enumerate vp

# Enumerate vulnerable themes
wpscan --url https://target.com --enumerate vt
```
Get API token: https://wpscan.com/register

### WPForce / WPSeku
```bash
# WPForce - user enumeration + brute force
git clone https://github.com/n00py/WPForce.git

# WPSeku - lightweight scanner
git clone https://github.com/m4ll0k/WPSeku.git
```

### Droopescan
```bash
pip install droopescan
droopescan scan wordpress -u https://target.com
```

### WhatWeb
```bash
whatweb https://target.com
```

## Vulnerability Databases

| Source | URL | Notes |
|--------|-----|-------|
| **WPScan** | https://wpscan.com/ | Largest WP-specific CVE database |
| **Exploit-DB** | https://www.exploit-db.com/ | Search "WordPress" |
| **CVE Mitre** | https://cve.mitre.org/ | Search by plugin name |
| **NVD NIST** | https://nvd.nist.gov/ | CVSS scores, references |
| **Patchstack** | https://patchstack.com/database/ | Commercial, but has public data |
| **Wordfence** | https://www.wordfence.com/threat-intel/ | Detailed vulnerability reports |
| **OpenCVE** | https://www.opencve.io/ | CVE monitoring |

## GitHub Repos

### Exploit Collections
- https://github.com/rastating/wordpress-exploit-framework
- https://github.com/wpscanteam/exploits
- https://github.com/WordPress/security

### Research Tools
- https://github.com/Audi-1/sqli-labs (SQLi practice)
- https://github.com/danielmiessler/SecLists (WordPress wordlists)
- https://github.com/0xInfection/Wordpress-Security-Notes

### Fuzzing
- https://github.com/ffuf/ffuf (directory fuzzing)
- https://github.com/OJ/gobuster (directory/parameter fuzzing)

## Research Papers

### REST API Security
- "Security Analysis of WordPress REST API" — IEEE 2024
- "Attacking and Defending WordPress REST API" — BlackHat 2023

### Plugin Security
- "Large-Scale Analysis of WordPress Plugin Vulnerabilities" — USENIX 2024
- "Automated Discovery of Vulnerabilities in WordPress Plugins" — ACM CCS 2023

### General WordPress Security
- "WordPress Security: A Survey" — ACM Computing Surveys 2024
- "The State of WordPress Security" — Sucuri Annual Report

## Books

| Title | Author | Focus |
|-------|--------|-------|
| WordPress Security Complete | Various | Practical hardening |
| The Web Application Hacker's Handbook | Stuttard & Pinto | General web app (includes WP) |
| Real-World Bug Hunting | Peter Yaworski | Bug bounty methodology |
| The Bug Bounty Hunter's Methodology | Various | General approach |

## Online Training

- **PentesterLab** — WordPress challenges
- **HackTheBox** — WordPress machines
- **TryHackMe** — WordPress rooms
- **PortSwigger Web Security Academy** — General web (applicable to WP)

## Local Testing Environment

```bash
# Quick WordPress setup with Docker
docker run --name wp-test -e WORDPRESS_DB_HOST=db -e WORDPRESS_DB_USER=root \
  -e WORDPRESS_DB_PASSWORD=password -e WORDPRESS_DB_NAME=wordpress \
  -p 8080:80 -d wordpress:7.0.2

# With MySQL
docker run --name wp-db -e MYSQL_ROOT_PASSWORD=password \
  -e MYSQL_DATABASE=wordpress -d mysql:8.0

# Or use Docker Compose
cat > docker-compose.yml << 'EOF'
version: '3'
services:
  db:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: password
      MYSQL_DATABASE: wordpress
  wordpress:
    image: wordpress:7.0.2
    ports:
      - "8080:80"
    environment:
      WORDPRESS_DB_HOST: db
      WORDPRESS_DB_USER: root
      WORDPRESS_DB_PASSWORD: password
      WORDPRESS_DB_NAME: wordpress
EOF
docker-compose up -d
```

## Cheat Sheet: Quick Commands

```bash
# Version detection
curl -s https://target.com/ | grep -oE 'WordPress [0-9.]+'

# User enumeration
curl -s https://target.com/wp-json/wp/v2/users | jq '.[].slug'

# Plugin list from REST API
curl -s https://target.com/wp-json/ | jq '.namespaces'

# Check XML-RPC
curl -s -X POST https://target.com/xmlrpc.php -d '<?xml version="1.0"?><methodCall><methodName>system.listMethods</methodName></methodCall>'

# Check batch endpoint
curl -s -X POST https://target.com/wp-json/batch/v1 -H "Content-Type: application/json" -d '{"requests":[]}'

# Directory brute force
gobuster dir -u https://target.com -w /usr/share/wordlists/dirb/common.txt -x php,html,txt

# Parameter fuzzing
ffuf -u https://target.com/?FUZZ=test -w /usr/share/wordlists/parameters.txt
```
