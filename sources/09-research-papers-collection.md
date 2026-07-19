# Academic Research Papers Collection

## Top Security Conferences

### S&P (IEEE Symposium on Security and Privacy)
- "Where URLs Become Weapons: Automated Discovery of SSRF Vulnerabilities in Web Applications" (2024)
- "SoK: Automating Kernel Vulnerability Discovery and Exploit Generation" (WOOT 2025)
- State-of-the-art 0-day discovery and automated exploit generation

### USENIX Security
- "Playing for K(H)eaps: Understanding and Improving Linux Kernel Exploit Reliability" (2022)
- "Subversive-C: COOP-style exploits for Objective-C" (2016)
- "kAFL: Hardware-Assisted Feedback Fuzzing for OS Kernels" (2017)
- "PACtighter: ARM Pointer Authentication defenses"

### NDSS
- "DUMPLING: Fine-grained Differential JavaScript Engine Fuzzing" (2025)
- "HFL: Hybrid Fuzzing on the Linux Kernel" (2020)

### ACM CCS
- "SyzGen: Automated Generation of Syscall Specification of Closed-Source macOS Drivers" (2021)
- "Snowboard: Finding Kernel Concurrency Bugs through Systematic Inter-thread Communication Analysis" (2021)

### ICSE
- "Tuning Configuration Selection for Continuous Kernel Fuzzing" (2025)

## Kernel Exploitation Papers

### Linux Kernel
- **Page Spray Analysis**: "Take a Step Further: Understanding Page Spray in Linux Kernel Exploitation" (arXiv 2024)
  - DirtyPage exploit model for kernel page-level exploitation
  - Root cause analysis of Page Spray in Linux Kernel
  - Lightweight mitigation approach proposed
- **Kernel Heap Exploit Reliability**: "Playing for K(H)eaps" (USENIX 2022)
  - Systematic study of kernel heap exploitation stabilization
  - 135.53% reliability improvement with composite stabilization
- **Kernel Vulnerability Defense**: "Linux kernel vulnerabilities: State-of-the-art defenses and open problems" (MIT CSAIL)
- **Kernel-Level Rootkits**: "Detecting Kernel-Level Rootkits Through Binary Analysis"
- **Comprehensive Review**: "A Systematic Review of Kernel-Level Security Mechanisms, Vulnerability Detection and Mitigation in Modern Operating Systems" (Sensors 2026)

### Windows Kernel
- **CVE-2024-30085**: Heap Buffer Overflow in cldflt.sys
  - ER Series Article 08: I/O Ring exploitation technique (91-page deep dive)
  - ER Series Article 09: PreviousMode flip + PPL bypass (106-page deep dive)
- **Windows Exploitation Tricks**: "Trapping Virtual Memory Access" — James Forshaw (Google Project Zero, 2021/2025 update)

## Browser Exploitation Papers
- "PatchFuzz: Patch Fuzzing for JavaScript Engines" (arXiv 2025)
- "DUMPLING: Fine-grained Differential JavaScript Engine Fuzzing" (NDSS 2025)
- "CVE-2024-2887: A Pwn2Own Winning Bug in Google Chrome" — Manfred Paul (ZDI 2024)
- V8 Sandbox Bypass Collection — xvonfers (2025)

## Fuzzing Research

### syzkaller-based Research
| Paper | Venue |
|-------|-------|
| Unlocking Low Frequency Syscalls with Dependency-Based RAG | ACM 2025 |
| Tuning Configuration Selection for Continuous Kernel Fuzzing | ICSE 2025 |
| SyzDirect: Directed Greybox Fuzzing for Linux Kernel | SOSP 2023 |
| KIT: Testing OS-Level Virtualization for Functional Interference Bugs | SOSP 2023 |
| SyzGen: Automated Syscall Generation for macOS Drivers | CCS 2021 |
| HFL: Hybrid Fuzzing on the Linux Kernel | NDSS 2020 |
| Towards LLM Guided Kernel Direct Fuzzing (SyzAgent) | arXiv 2025 |

### General Fuzzing
- "Fuzzing: A Survey" — Chen et al., 2018 (comprehensive review)
- "Revealing the Exploitability of Heap Overflow through PoC" — Hoee
- "Hydra: Finding Semantic Bugs in File Systems with an Extensible Fuzzing Framework" (SOSP 2019)
- "Janus: Fuzzing File Systems via Two-Dimensional Input Space Exploration" (SOSP 2019)
- "KRACE: Data Race Fuzzing for Kernel File Systems"
- "kAFL: Hardware-Assisted Feedback Fuzzing for OS Kernels" (USENIX 2017)

## EDR Bypass Research
- "HookChain: A new perspective for Bypassing EDR Solutions" — Helvio Carvalho Junior (arXiv 2024)
  - IAT Hooking + dynamic SSN resolution + indirect system calls
  - Evades ntdll-only EDR hooks
- "EDR Kernel-Mode Exploitation Exposed" — Undercode Testing 2026
- "EDR Tradecraft: Internals, Detection, Evasion & Advanced Research" — 0xDbgMan 2026

## Cloud Security Research
- "Global Cybersecurity Outlook 2026" — World Economic Forum + Accenture
- "Blinding the Watchmen: Cloud Logging as an Attack Surface" — CSA 2026
- "CloudImposer: RCE on GCP via dependency confusion" — Tenable (Black Hat 2024)
- "From Container to Cluster: Chained Escape Attacks in Kubernetes"
- "Container Breakouts: Escape Techniques in Cloud Environments" — Unit42

## AI / ML Security Research
- "JailbreakRadar: Comprehensive Assessment of Jailbreak Attacks Against LLMs" (ACL 2025)
- "Advances in Cybersecurity: A Literature Review" (ResearchGate 2025)
- "Artificial Intelligence in Cybersecurity: A Comprehensive Review and Future Direction" (2024)
- "Advancing cybersecurity: a comprehensive review of AI-driven detection techniques" (Journal of Big Data 2024)

## Exploit Reversing Series
- Alexandre Borges' ER Series:
  - Article 08 (2026): CVE-2024-30085 exploitation with I/O Ring (91 pages)
  - Article 09 (2026): CVE-2024-30085 with PreviousMode + PPL bypass (106 pages)
  - Additional coverage: Windows kernel, Chrome, iOS

## Research Paper Collections
- **FuzzingPaper**: github.com/wcventure/FuzzingPaper
- **Linux Kernel Exploitation**: github.com/xairy/linux-kernel-exploitation
- **Syzkaller Research**: github.com/google/syzkaller/blob/master/docs/research.md
- **V8 Sandbox Bypass**: deepwiki.com/xv0nfers/V8-sbx-bypass-collection
