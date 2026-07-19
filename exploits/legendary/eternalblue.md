# EternalBlue (MS17-010)

## Classification
- **CVE**: CVE-2017-0143 / CVE-2017-0144 / CVE-2017-0145 / CVE-2017-0146 / CVE-2017-0147 / CVE-2017-0148
- **CVSS 3.0**: 8.1 (High) / 9.3 (original MS)
- **Type**: Windows SMBv1 Remote Code Execution (Buffer Overflow)
- **Origin**: NSA Equation Group → Shadow Brokers leak (April 2017)
- **Wild Impact**: WannaCry (May 2017), NotPetya (June 2017), Bad Rabbit, dozens of ransomware families

## Root Cause

A **DWORD→WORD truncation** in `SrvOs2FeaListSizeToNt()` when calculating buffer size for FEA (File Extended Attributes) list conversion. The calculation multiplies `NumberOfEntries` by `sizeof(FEA)` which can overflow a `WORD` (16-bit), resulting in an undersized buffer allocation. When `SrvOs2FeaToNt()` copies data into this undersized buffer, it overflows into adjacent kernel pool memory.

**The bug in simplified form:**
```c
// SrvOs2FeaListSizeToNt — returns WORD (16-bit)
WORD SrvOs2FeaListSizeToNt(DWORD count) {
    return (WORD)(count * sizeof(FEA));  // TRUNCATION!
}

// SrvOs2FeaToNt — copies FEA data
void SrvOs2FeaToNt(PVOID dst, DWORD count) {
    WORD size = SrvOs2FeaListSizeToNt(count);
    // dst is allocated for `size` bytes but count * sizeof(FEA) > size
    for (DWORD i = 0; i < count; i++) {
        memcpy(dst + ...);  // OVERFLOW!
    }
}
```

## Exploit Chain

### Stage 1: Pool Grooming
1. Send multiple SMB transactions to create predictable `srvnet.sys` pool allocations
2. Allocate ~6700 contiguous FEA entries in the non-paged pool
3. Create "holes" in the pool for the overflow target

### Stage 2: Buffer Overflow
1. Send crafted SMB_COM_TRANSACTION2 secondary request with oversized FEA list
2. Trigger the WORD truncation to allocate small buffer (e.g., 0x5100 instead of 0x20000)
3. Overflow writes past buffer into adjacent kernel objects in the non-paged pool
4. Overwrite the `PoolHeader` of an adjacent `srvnet.sys` NET buffer

### Stage 3: Info Leak
1. Modified pool header causes `srvnet!SrvNetWskConnReceive` to read from controlled address
2. Leak kernel base address (KASLR bypass) and HAL heap base

### Stage 4: Privileged Write Primitive
1. Use the overflow to overwrite `srvnet!SrvNetWskConnReceive` function pointer
2. Or overwrite pool pages containing an MDL (Memory Descriptor List) for arbitrary physical R/W

### Stage 5: Shellcode Execution
1. Set up a HAL heap allocation containing x64/x86 shellcode
2. Trigger function call via corrupted pointer
3. Shellcode runs at **SYSTEM level** → install DoublePulsar backdoor or deploy ransomware
4. WannaCry specifically: installs SMB worm component → self-propagates to new targets

## Critical Observations

### Why It Was So Effective
- **No authentication required**: Anonymous SMB access on default Windows config
- **Self-propagating**: WannaCry infected 200K+ systems in 4 days
- **Remained unpatched**: Many orgs still had SMBv1 enabled years after patch
- **Multiple CVEs**: The bug existed in multiple code paths (6 separate CVEs)

### The NSA Origin
- Developed by Equation Group (NSA)
- Stockpiled for years before leak
- Public leak forced Microsoft to patch → but too late for many

### Pattern Recognition: What to Watch For
1. **Type casts with loss of precision**: Any DWORD→WORD, size_t→int, 64→32 bit narrowing
2. **Inconsistent size calculations**: Where alloc size and copy size use different logic
3. **Protocol converters**: FEA→NT conversion is the classic example — generic protocol translation code
4. **Legacy protocol support**: SMBv1 should have been removed decades ago — always audit legacy features
5. **Anonymous access to complex parsers**: Any service accepting unauthenticated complex protocol parsing

## Related Variants
- **EternalSynergy**: Same SMB pool manipulation, different code path
- **EternalRomance**: SMBv1 transaction vulnerability (also used in WannaCry)
- **EternalChampion**: SMBv1 + SMBv2 negotiation bypass
- All patched by MS17-010

## Mitigation Status
- **Patch**: MS17-010 (March 2017) — still effective for supported Windows
- **Workaround**: Disable SMBv1 (`Set-SmbServerConfiguration -EnableSMB1Protocol $false`)
- **Legacy systems**: Windows XP/2003 remain permanently vulnerable
- **2026 status**: Shodan still shows tens of thousands of exposed SMBv1 hosts

## PoC & References
- **Original NSA tools** (leaked): github.com/iam-Senpai/Eternalblue-Doublepulsar-Metasploit
- **Metasploit module**: exploit/windows/smb/ms17_010_eternalblue
- **AutoBlue-MS17-010**: github.com/3ndG4me/AutoBlue-MS17-010 (standalone exploit kit)
- **DeepWiki analysis**: deepwiki.com/SecWiki/windows-kernel-exploits/MS17-010
- **FreeCodeCamp guide**: How to exploit EternalBlue step-by-step
- **Bomberbot analysis**: In-depth 2600-word analysis of the flaw
