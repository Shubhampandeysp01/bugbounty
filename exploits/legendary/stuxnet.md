# Stuxnet

## Classification
- **Type**: Multi-0-day cyber weapon targeting industrial control systems (ICS/SCADA)
- **Discovered**: June 2010 (VirusBlokAda researcher in Belarus)
- **Origin**: Believed to be joint US-Israeli operation (Operation Olympic Games)
- **Target**: Iranian uranium enrichment centrifuges at Natanz

## The Exploit Chain

Stuxnet used **four Windows 0-days** and **two ICS 0-days** — unprecedented sophistication:

### Windows 0-days
1. **CVE-2010-2568** (LNK vulnerability): Auto-execute via USB drive — malware spread through air-gapped networks by exploiting Windows shortcut file parsing
2. **CVE-2010-2729** (Print Spooler): Privilege escalation from user to SYSTEM
3. **CVE-2010-2743** (Win32k.sys keyboard layout): Kernel privilege escalation
4. **CVE-2010-2772** (SMB vulnerability): Propagation within LAN

### Digital Certificates
- Stolen certificates from **Realtek Semiconductor** and **JMicron** to sign kernel drivers (bypassing 64-bit Windows driver signature enforcement)

### Step-and-Repeat Infection (USB + LAN)
1. USB autorun.inf + LNK exploit to jump air gap
2. Print Spooler EoP to get SYSTEM
3. SMB propagation to other Windows machines on network
4. Copy itself to Step-7 projects shared on network
5. Wait for engineer to open Step-7 project on machine with Siemens programming software

### ICS Payload
1. Modify Siemens Step-7 project files (inject code into S7-315/417 PLCs)
2. **Man-in-the-Middle on Profibus**: Intercept and modify communications between Step-7 software and PLC
3. **Centrifuge sabotage**: Rotate centrifuges at frequencies that cause physical destruction
4. **Deception**: Send normal-looking sensor readings back to monitoring systems so operators don't see damage

## Technical Deep Dive

### USB Propagation (CVE-2010-2568)
```c
// Windows automatically parses LNK (.lnk) files found in USB drives
// The bug: LNK files with an icon reference to a DLL can execute the DLL
// Stuxnet's LNK file pointed to a specially crafted DLL
```

### PLC Code Injection
Stuxnet permanently modified the PLC's Step-7 code by overwriting the `s7otbxdx.dll` on the engineering workstation. When the engineer compiled and uploaded code, Stuxnet injected malicious blocks into the upload — blocks the engineer couldn't see in the Step-7 IDE.

### Frequency Manipulation
Centrifuges normally spin at ~63,000 RPM. Stuxnet:
1. Raised speed to **1,410 Hz** (~84,600 RPM) for ~15 minutes — resonant frequency causes bearing damage
2. Dropped to **2 Hz** (120 RPM) for ~50 minutes — violent oscillation
3. Alternated between these extremes, causing cumulative physical damage
4. Repeated over months

## Pattern Recognition

1. **Air-gap is not security**: USB + LNK showed that air gaps are permeable — anything that touches an unsecured endpoint is an infection vector
2. **Multiple 0-days chained**: No single bug was exceptional; the combination was devastating
3. **Supply chain trust**: Signed drivers with stolen certs — digital signature verification is only as good as the CA's security
4. **Knowledge of target environment**: Attackers understood centrifuge physics, PLC programming, Siemens software internals
5. **Deception is critical**: The fake sensor readings kept the attack hidden for months — detection latency is game over for destructive attacks
6. **Physical/digital bridge**: The most sophisticated payloads cross from cyberspace to physical destruction

## Impact
- **~1,000 centrifuges destroyed** (20% of Iran's uranium enrichment capacity)
- **Delayed Iranian nuclear program by ~2 years** (US intelligence assessment)
- **First known cyber weapon**: Demonstrated that cyber attacks can cause kinetic (physical) damage
- **Changed warfare**: Every nation-state now has offensive cyber capabilities modeled after Stuxnet

## References
- **Original discovery**: VirusBlokAda report — "Rootkit.Tmphider"
- **Symantec Stuxnet Dossier**: W32.Stuxnet Dossier (most detailed public analysis)
- **Ralph Langner**: "To Kill a Centrifuge" (definitive analysis by the researcher who reverse-engineered Stuxnet's payload)
- **Wired**: "The Real Story of Stuxnet" by Kim Zetter
- **MIT/CSAIL**: Technical analysis of the PLC rootkit
- **C:S:": Documentaries: Zero Days (2016), Countdown to Zero Day (2014)
