# Bug Bounty Research Repository

> A curated collection of security research resources — learning guides, case studies, and references for bug bounty hunting and vulnerability research.

## Structure

```
bugbounty/
├── README.md                    ← This file
├── guides/                      ← Learning guides by topic
│   ├── 01-bug-bounty-methodology.md
│   ├── 02-web-application-security.md
│   ├── 03-binary-exploitation.md
│   ├── 04-browser-exploitation.md
│   ├── 05-cloud-container-security.md
│   ├── 06-mobile-security.md
│   ├── 07-kernel-fuzzing.md
│   ├── 08-windows-exploitation.md
│   ├── 09-linux-kernel-exploitation.md
│   ├── 10-edr-bypass-defense-evasion.md
│   └── wordpress/               ← WordPress-specific research
│       ├── README.md
│       ├── 01-recon-enumeration.md
│       ├── 02-core-vulnerabilities.md
│       ├── 03-plugin-theme-vulns.md
│       ├── 04-exploit-development.md
│       └── 05-tools-resources.md
├── references/                   ← Curated lists of external resources
│   ├── 01-research-papers.md
│   ├── 02-essential-books.md
│   ├── 03-github-learning-resources.md
│   └── 04-monitoring-feeds.md
└── case-studies/                 ← Real-world exploit analyses
    ├── legendary/                ← Historic exploits that changed security
    │   ├── dirtycow.md
    │   ├── eternalblue.md
    │   ├── heartbleed.md
    │   ├── log4shell.md
    │   ├── printnightmare.md
    │   ├── proxylogon.md
    │   ├── shellshock.md
    │   ├── spectre-meltdown.md
    │   ├── stuxnet.md
    │   └── zerologon.md
    └── 0day-reports/             ← Recent/active exploit analyses
        ├── android-framework-2026.md
        ├── chrome-0day-2026.md
        ├── cpanel-auth-bypass-2026.md
        ├── fortinet-sso-2026.md
        ├── office-ole-2026.md
        └── windows-kernel-2025.md
```

## How to Use

### 🧭 Suggested Learning Path

| Step | Focus | Resource |
|------|-------|----------|
| 1 | Bug bounty fundamentals | `guides/01-bug-bounty-methodology.md` |
| 2 | Web application security | `guides/02-web-application-security.md` |
| 3 | GitHub learning resources | `references/03-github-learning-resources.md` |
| 4 | Binary exploitation | `guides/03-binary-exploitation.md` |
| 5 | Browser exploitation | `guides/04-browser-exploitation.md` |
| 6 | Cloud & container security | `guides/05-cloud-container-security.md` |
| 7 | Mobile security | `guides/06-mobile-security.md` |
| 8 | Kernel fuzzing | `guides/07-kernel-fuzzing.md` |
| 9 | Windows exploitation | `guides/08-windows-exploitation.md` |
| 10 | Linux kernel exploitation | `guides/09-linux-kernel-exploitation.md` |
| 11 | EDR bypass & defense evasion | `guides/10-edr-bypass-defense-evasion.md` |

### 📚 Reference Material
- **Books to read**: `references/02-essential-books.md`
- **Research papers**: `references/01-research-papers.md`
- **GitHub repos & free labs**: `references/03-github-learning-resources.md`
- **0-day monitoring feeds**: `references/04-monitoring-feeds.md`

### 🧪 Case Studies
Study legendary exploits in `case-studies/legendary/` and recent 0-days in `case-studies/0day-reports/` to understand real-world exploit patterns.

### 🔍 WordPress Research
If targeting WordPress sites, start with `guides/wordpress/README.md` for a focused learning path.

---

**Last updated:** July 2026
