# EDR Bypass & Defense Evasion

## EDR Architecture

### Components
- **User-mode service**: Management, configuration, telemetry upload
- **Injected DLL (ntdll hooking)**: Intercept Win32 API calls
- **Kernel-mode driver**: Mini-filter, process/thread callbacks, ETW consumer
- **ETW (Event Tracing for Windows)**: Provider-based telemetry collection
- **Cloud back-end**: Telemetry correlation and AI-based detection

### Detection Engines
1. **Signature-based**: Static hash/pattern matching
2. **Behavioral**: Syscall sequence analysis, anomaly detection
3. **Heuristic**: ML models for suspicious patterns
4. **Kernel-level**: Callbacks from ProcessMon, ThreadMon, ImageLoad, Registry

## EDR Bypass Techniques

### User-Mode Bypass

**IAT Hook Bypass:**
- **Direct syscalls**: Execute `syscall` instruction directly (SysWhispers, SysWhispers2/3/4)
- **Indirect syscalls**: Call `syscall` via a valid ntdll address to evade call-stack checks
- **API hashing**: Resolve API addresses dynamically by hashed names
- **Manual mapping**: Map ntdll fresh copy from disk to bypass hooks
- **Hell's Gate / Halo's Gate / RecycledGate / FreshyCalls**: Syscall number resolution techniques

**HookChain** (arXiv 2024):
- IAT Hooking + dynamic SSN resolution + indirect syscalls
- Intercept at subsystem level to bypass EDR hooks on ntdll.dll
- 94% of EDRs do not monitor above ntdll

**ETW Evasion:**
- Patch `EtwEventWrite` to disable ETW provider
- Hardware breakpoint on ETW functions
- Use Frida to hook and nullify ETW calls
- Patchless AMSI bypass via VEH

**Call Stack Spoofing:**
- **SilentMoonwalk**: Spoof return addresses on call stack
- **VulcanRaven**: Advanced call stack spoofing
- Use ROP to manipulate stack frames to appear benign

**Memory Evasion:**
- Sleep obfuscation: **Ekko**, **FOLIAGE**, **DreamWalkers** (encrypt in-memory payloads during sleep)
- Beacon object files (BOF): Execute in-memory only
- Process hollowing / Herpaderping: Spoof legitimate processes

### Kernel-Mode Bypass

**Kernel Callback Removal:**
- Unlink from `PspCreateProcessNotifyRoutine`, `PspCreateThreadNotifyRoutine`
- Disable `CmRegisterCallback`, `ObRegisterCallbacks`
- Patch kernel structures to nullify EDR callbacks

**BYOVD (Bring Your Own Vulnerable Driver):**
- Load a legitimate but vulnerable signed driver
- Exploit IOCTL to read/write kernel memory
- Kill EDR process from kernel mode
- Blocklist bypass: Retro-signing, cross-signed certs

**I/O Ring Attacks:**
- Use Windows I/O Ring for arbitrary kernel read/write without syscall interception
- CVE-2024-30085 exploitation technique

**Driver IOCTL Fuzzing:**
- Send malformed IOCTL buffers to EDR device objects
- Exploit insufficient input validation for privilege escalation
- Tools: IOCTL fuzzer, WinObj, AccessChk, WinDbg

## Research Papers
- "HookChain: A new perspective for Bypassing EDR Solutions" - arXiv 2404.16856
- "EDR Kernel-Mode Exploitation Exposed" - Undercode Testing 2026
- "EDR Tradecraft: Internals, Detection, Evasion & Advanced Research" - 0xDbgMan 2026
- "EDR/XDR Bypass and Detection Evasion Techniques" - Dev.to 2026

## Tools
- **SysWhispers/2/3/4**: Direct syscall generation
- **FreshyCalls**: Syscall resolution with dynamic number fetching
- **RecycledGate**: Recycle syscall gates
- **Acheron**: Another syscall gate technique
- **SilentMoonwalk**: Call stack spoofing
- **InlineWhispers**: Syscalls for Cobalt Strike BOFs
- **Krueger**: Exploit Windows Defender to neutralize EDR
- **Terminator**: EDR killer using BYOVD

## Resources
- **Awesome EDR Bypass**: github.com/tkmru/awesome-edr-bypass
- **Awesome AV/EDR/XDR Bypass**: github.com/MrEmpy/Awesome-AV-EDR-XDR-Bypass
- **Evading EDR** (No Starch Press) - book
- **EDR Internals for macOS and Linux** - Outflank Security Blog
