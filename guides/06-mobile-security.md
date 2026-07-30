# Mobile Security

## iOS Exploitation

### BootROM Exploits
- **checkm8** (2019): Permanent unpatchable bootrom exploit for A5–A11 devices
  - USB-based, requires physical access
  - Source: github.com/axi0mX/ipwndfu
  - Used by: jailbreak tools (checkra1n)
- **Usbliter8** (2026): New BootROM bypass for millions of iPhones
  - Exploits DFU mode USB handling
  - Cannot be patched via software update

### Jailbreak Techniques
- **Semi-tethered**: Requires re-jailbreak on reboot (e.g., checkra1n, unc0ver)
- **PPL bypass**: Patch Protection Layer in modern iOS
- **PAC bypass**: Pointer Authentication bypass via kernel exploit
- **KTRR bypass**: Kernel Text Read-Only Region bypass

### iOS App Hacking
- **Frida**: SSL pinning bypass, method tracing, runtime manipulation
- **Objection**: Automated mobile security testing
- **Class-dump**: Extract Objective-C class info from binaries
- **IPA extraction**: Decrypt App Store binaries
- **Anti-debugging bypass**: Defeat PTRACE, sysctl, etc.

### iOS Research Resources
- iOS Security Research: github.com/OutrageousStorm/ios-security-research
- Jailbreak Landscape (2025-2026): Current state documentation
- Corellium: Virtual iOS devices for security testing

## Android Exploitation

### Boot/ROM Exploitation
- **Qualcomm bootrom exploits**: Aboot, LK bootloader bypass
- **Verified Boot bypass**: dm-verity disable for persistent root
- **Treble/Project Mainline**: Reduced attack surface but split system images

### Android App Hacking
- **APK analysis**: JADX, APKTool, JEB Decompiler
- **Smali patching**: Modify application logic at bytecode level
- **Intent abuse**: Intent redirection, component export exploitation
- **Content Provider**: SQL injection, directory traversal
- **WebView**: JavaScript bridge abuse, `addJavascriptInterface` RCE
- **Frida for Android**: Root detection bypass, SSL unpinning

### Common Vulnerabilities
- **Insecure Direct Object Reference (IDOR)**: Access other users' data
- **Deep Link abuse**: Call arbitrary components
- **Backup exploit**: Android Backup (adb backup) leaks sensitive data
- **Tapjacking**: Overlay attacks on sensitive UI elements
- **Logging**: Debug logs leaking credentials/tokens

## Mobile Security Tools
- **Frida**: Cross-platform runtime instrumentation (Windows/macOS/Linux/iOS/Android)
- **Objection**: Frida-based mobile testing toolkit
- **MobSF**: Mobile Security Framework (static + dynamic analysis)
- **JADX**: DEX to Java decompiler
- **APKTool**: APK reverse engineering
- **Corellium**: Virtual devices for mobile security research
- **radare2 / Ghidra**: Binary analysis and reverse engineering

## Key Research
- "Drill the Apple Core: Up & Down" - Black Hat EU
- "iOS mobile malware analysis: state-of-the-art" - PMC 2023
- Mobile Security Writeups: lautarovculic.github.io
- "Racing Against the Lock: Exploiting Spinlock UAF in the Android Kernel" - Moshe Kol, JSOF

## GitHub Learning Links
- [OWASP/owasp-mastg](https://github.com/OWASP/owasp-mastg) — Mobile App Security Testing Guide
- [MobSF/Mobile-Security-Framework-MobSF](https://github.com/MobSF/Mobile-Security-Framework-MobSF)
- [frida/frida](https://github.com/frida/frida)
- [withsecurelabs/android-keystore-audit](https://github.com/withsecurelabs) — search Android security labs
