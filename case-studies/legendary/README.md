# Legendary Exploits

These are the exploits that shaped cybersecurity — breakthrough techniques, world-shaking breaches, and pattern-defining vulnerabilities that every serious hacker must understand.

## Index

| # | Exploit | Year | Type | Impact | Key Pattern |
|---|---------|------|------|--------|-------------|
| 1 | **EternalBlue** (MS17-010) | 2017 | SMB Buffer Overflow | WannaCry: 200K+ systems, $4B+ damage | DWORD→WORD truncation + pool grooming |
| 2 | **Heartbleed** (CVE-2014-0160) | 2014 | TLS Heartbeat OOB Read | Massive private key + memory leak | Missing bounds check on user-controlled length |
| 3 | **Shellshock** (CVE-2014-6271) | 2014 | Bash CGI RCE | ~500M systems affected | Function definition → trailing command injection |
| 4 | **Log4Shell** (CVE-2021-44228) | 2021 | Log4j JNDI RCE | ~50% corporate networks, months of remediation | JNDI lookup on unsanitized log input |
| 5 | **Dirty COW** (CVE-2016-5195) | 2016 | Linux Kernel Race → PrivEsc | All Linux kernels 2.6.22+ (12 years) | Race between madvise and get_user_page |
| 6 | **Spectre/Meltdown** | 2018 | CPU Speculative Execution | Every modern CPU (2018-) | Transient execution + cache side-channel |
| 7 | **ProxyLogon** (CVE-2021-26855) | 2021 | Exchange SSRF → RCE Chain | 250K+ servers compromised in days | SSRF + auth bypass + arbitrary file write |
| 8 | **PrintNightmare** (CVE-2021-34527) | 2021 | Windows Print Spooler RCE | All Windows versions | Arbitrary DLL load by unprivileged user |
| 9 | **ZeroLogon** (CVE-2020-1472) | 2020 | Netlogon Crypto Bypass | Domain controller takeover | 128 zero-byte nonce → auth bypass |
| 10 | **Stuxnet** | 2010 | Multi-0-day ICS Sabotage | First known cyber-weapon | 4 Windows 0-days + 2 industrial 0-days |
| 11 | **Stagefright** (CVE-2015-1538) | 2015 | Android MMS RCE | ~950M Android devices | Integer overflow in MP4 parsing |
| 12 | **BlueKeep** (CVE-2019-0708) | 2019 | RDP RCE (wormable) | ~1M exposed systems | Use-after-free in RDP termdd.sys |
| 13 | **checkm8** | 2019 | iOS BootROM Exploit | A5-A11 iPhones (unpatchable) | USB DFU race condition in BootROM |
| 14 | **Follina** (CVE-2022-30190) | 2022 | MSDT Word RCE | All Windows | ms-msdt:// protocol handler → PowerShell RCE |

## How to Study

1. **Read the root cause** — understand the exact bug pattern (off-by-one, type confusion, race, etc.)
2. **Trace the exploit chain** — how did the attacker go from bug to shell?
3. **Identify the bypass** — what security boundary was crossed?
4. **Look for variants** — what other systems have the same pattern?
5. **Write a PoC** — recreate the exploit in a lab environment
