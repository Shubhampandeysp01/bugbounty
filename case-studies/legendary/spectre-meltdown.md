# Spectre & Meltdown

## Classification
- **CVEs**: Meltdown (CVE-2017-5754), Spectre v1 (CVE-2017-5753), Spectre v2 (CVE-2017-5715)
- **CVSS**: 5.6 (Medium per CVE) but **actual impact far exceeds score**
- **Type**: CPU Speculative Execution — Microarchitectural Side-Channel
- **Affected**: Nearly every CPU made since ~1995 (Intel, AMD, ARM, IBM)
- **Discovery**: Google Project Zero, Cyberus Technology, Graz University of Technology (Jan 2018)
- **Wild Impact**: Classified as speculative execution attacks — no known mass exploitation but demonstrated for every major OS/browser

## Root Cause

Modern CPUs optimize by **speculatively executing** instructions before knowing if they'll be needed. When speculation is wrong, the CPU **architecturally** rolls back changes — but **microarchitectural** state (caches, TLBs, branch predictors) is NOT rolled back. An attacker can leak data via a **covert channel** using cache timing.

### Meltdown (CVE-2017-5754)
The OS kernel and user-space share the same address space (with permission bits). Speculative execution skips the permission check:

```c
// User-space code that exploits Meltdown
char *kernel_ptr = (char*)0xFFFFF80000000000;  // kernel address
char probe[256 * 4096];

// Access is architecturally faulted (kernel-only memory)
// But CPU speculatively executes before fault
char kernel_byte = *kernel_ptr;  // speculatively loaded

// Use kernel_byte to index into probe array
// This caches ONE cache line in the probe
temp = probe[kernel_byte * 4096];  // speculative cache load

// Fault occurs here — execution is rolled back
// BUT: probe[0x42 * 4096] is now cached!
// Attacker measures access time to find which index is fast
```

### Spectre v1 (CVE-2017-5753)
Branch prediction can be poisoned to speculatively access attacker-controlled offsets:

```c
// Victim code with bounds check
if (x < array1_size) {  // array1_size is checked
    // BUT: if branch predictor is trained that x is "safe"
    // and then x is malicious, the CPU speculatively executes
    y = array2[array1[x] * 4096];  // SPECULATIVE OOB ACCESS
}
```

### Spectre v2 (CVE-2017-5715)
Branch Target Injection — poison the indirect branch predictor (BTB) to redirect speculation to attacker-chosen gadgets (like a speculative ROP).

## Exploit Chain (JIT-based browser variant)

1. **Train branch predictor**: Repeatedly call a function with valid index to train the predictor
2. **Flush + reload**: Time access to probe array to establish baseline
3. **Poison predictor**: Call function with out-of-bounds index
4. **Speculative execution**: CPU executes past the bounds check speculatively
5. **Bit-by-bit extraction**: Each iteration leaks one bit of memory via cache timing
6. **Reconstruct**: Combine bits to read entire memory regions (passwords, keys, etc.)

## Impact

### What Can Be Leaked
- **Kernel memory** (Meltdown): Physical memory contents — encryption keys, passwords, files
- **Other process memory** (Spectre): Any memory in the same address space
- **Cloud VMs** (Spectre): Read memory of co-tenant VMs on same physical host

### Bypassed Protections
- **Kernel ASLR**: Irrelevant — attacker reads kernel directly
- **Page table permissions**: Meltdown ignores them
- **SMEP/SMAP**: Irrelevant — no code execution needed
- **KASLR**: Irrelevant for Meltdown (reads memory directly)
- **Hardware isolation**: AES-NI, SGX, TrustZone all affected

## Pattern Recognition

1. **Anything that speculates can leak**: The pattern is "execute then roll back" — any speculative mechanism is vulnerable
2. **Timing side-channels**: The cache is the most common, but any measurable microarchitectural state works
3. **Bounds check ≠ safety**: The CPU may ignore the bounds check speculatively
4. **Isolation layers are meaningless**: Hardware isolation is not immune — Spectre crosses process boundaries, Meltdown crosses user/kernel
5. **The fix is "don't speculate dangerously"**: KPTI (Meltdown) separates page tables, retpoline (Spectre v2) replaces indirect branches, LFENCE serialization (Spectre v1)

## Extended Family

| Name | CVE | Mechanism | Year |
|------|-----|-----------|------|
| **Meltdown** | CVE-2017-5754 | Speculative permission bypass | 2018 |
| **Spectre v1** | CVE-2017-5753 | Bounds check bypass | 2018 |
| **Spectre v2** | CVE-2017-5715 | Branch target injection | 2018 |
| **MDS** (ZombieLoad) | CVE-2018-12126-30 | Microarchitectural buffer sampling | 2019 |
| **Foreshadow** (L1TF) | CVE-2018-3615 | L1 cache speculative access | 2018 |
| **Fallout** | CVE-2019-11135 | Store buffer sampling | 2019 |
| **RIDL** | CVE-2019-14821 | Intel CPU side-channel | 2019 |
| **SWAPGS** | CVE-2019-1125 | Speculative GS segment access | 2019 |
| **Native BHI** | CVE-2024-2201 | Native branch history injection on last-gen Intel | 2024 |

## Mitigation Status
- **Meltdown**: KPTI (Kernel Page Table Isolation) — 5-30% performance hit
- **Spectre v1**: Manual LFENCE barriers in kernel — compiler flags `-mindirect-branch=thunk-extern`
- **Spectre v2**: Retpoline (Google), IBRS, IBPB — CPU microcode + kernel changes
- **MDS**: VERW instruction to clear CPU buffers on context switch
- **Reality check**: Full mitigation is not possible without disabling speculative execution entirely — no vendor has done this

## PoC & References
- **Project Zero disclosure**: googleprojectzero.blogspot.com/2018/01/reading-privileged-memory-with-side.html
- **Graz University**: meltdownattack.com (original paper + FAQs)
- **Spectre paper**: spectreattack.com (original research)
- **InSpectre Gadget**: github.com/vusec/inspectre-gadget (VU Amsterdam tool finding new Spectre gadgets, 2024)
- **Native BHI exploit**: github.com/vusec/native-bhi (leaks kernel memory at 3.5KB/sec on last-gen Intel)
- **Kernel MDS doc**: kernel.org/doc/html/latest/arch/x86/mds.html (Linux kernel MDS mitigation docs)
- **Phoronix benchmarks**: phoronix.com — track Spectre/Meltdown performance impact over time
- **Still relevant 2026**: InSpectre Gadget found 1511 Spectre gadgets + 2105 dispatch gadgets in Linux kernel 6.6 — attack surface is non-trivial
