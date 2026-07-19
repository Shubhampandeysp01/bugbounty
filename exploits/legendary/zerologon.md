# ZeroLogon (CVE-2020-1472)

## Classification
- **CVE**: CVE-2020-1472
- **CVSS 3.1**: 10.0 (Critical)
- **Type**: Netlogon Cryptographic Bypass — Privilege Escalation to Domain Admin
- **Affected**: All Windows Server versions (2008 → 2019), all Domain Controllers
- **Wild Impact**: Complete Active Directory domain takeover for any organization with unpatched DCs

## Root Cause

The Netlogon Remote Protocol uses AES-CFB8 for authentication. **The CFB8 initialization vector is all zeros**. If an attacker sends a ciphertext of all zeros, the output is all zeros, and the authentication check passes trivially.

```c
// Netlogon uses AES-128-CFB8 with an IV of all zeros
// CFB8 encryption: C[i] = AES_encrypt(IV || C[0..i-1]) XOR P[i]
//
// If attacker sets both IV and all ciphertext bytes to zero:
// C[0] = AES_encrypt(zero_iv) XOR P[0]
//
// BUT: If the attacker sends 8 zero bytes as the ciphertext
// and claims it's for the CLIENT_CREDENTIALS (8 zero bytes):
// The server computes: AES_encrypt(zero_iv) → some value X
// Server expects: X XOR P[0] = C[0]
// Server has C[0] = 0 (from attacker), P[0] = 0 (expected credential byte)
// Server checks: does AES_encrypt(zero_iv) XOR 0 == 0?
// → Does AES_encrypt(zero_iv) == 0?? 
// With AES, NO this doesn't normally work...
```

**Wait — the actual vulnerability is more subtle:**

The bug is in the **AES-CFB8 implementation in Microsoft's netlogon**. The CFB8 mode has a specific property: if the ciphertext is **all zeros** and the **output feedback is also zero**, the validation can be bypassed. The key insight: the attacker sends **8 zero bytes** as the "encrypted" client credential. Due to a combination of:
1. CFB8's chaining mode
2. The zero IV
3. An implementation-specific behavior in how Netlogon handles the zero ciphertext

→ The server incorrectly computes the expected credential as 8 zero bytes, matching the attacker's input.

**Simplified**: By sending 8 zero bytes as the "encrypted password," and making 256 authentication attempts (one for each possible first byte of AES output), there is a 1-in-256 chance per attempt that `AES_encrypt(zero_IV)[0:1] = 0x00`, which makes the entire authentication succeed. After ~256 attempts, one is guaranteed to succeed.

## Exploit Chain

1. **ZeroAuth**: Connect to DC's Netlogon RPC interface (port 445, via SMB named pipe `\pipe\netlogon`)
2. **256 attempts**: Send 256 Netlogon `NetServerAuthenticate2` requests with zero session keys and zero credentials
3. **Success**: One attempt will statistically pass (1:256 odds per attempt) — attacker is "authenticated" as Domain Controller machine account
4. **Change DC password**: Use authenticated Netlogon session to call `NetrServerPasswordSet2` — change the DC's computer account password to empty string
5. **Dump Domain Secrets**: Using the empty password, DCSync:
   ```
   impacket-secretsdump -just-dc -no-pass <DC_NAME>\$@<DC_IP>
   ```
   → Obtains NTLM hashes of ALL domain users including KRBTGT, Domain Admin
6. **Domain Admin**: Use DA hash for Pass-the-Hash to any resource in the domain

## Why It's 10.0 (Critical)
- **Unauthenticated**: Any attacker with network access to a Domain Controller
- **Zero interaction**: No phishing, no user action, no creds needed
- **Complete domain takeover**: From zero to Domain Admin in < 60 seconds
- **Stealth**: Leaves minimal forensic evidence (can be detected via event log 5827/5828 or Netlogon failures)

## Pattern Recognition

1. **Crypto implementation errors**: Not using proven library implementations, custom crypto logic
2. **Zero IV / zero key problems**: Cryptographic primitives that don't account for edge cases
3. **Protocol downgrade**: Attacker forces weak crypto mode in protocol negotiation
4. **Brute-forcible entropy**: 1:256 odds per attempt → 256 attempts = guaranteed success
5. **Ability to change machine password**: Once authenticated, being able to change ANY account's password including DC accounts

## Variants
- **CVE-2020-1472**: Original — full ZeroLogon
- **CVE-2020-1473**: Related Netlogon DoS
- **Microsoft partial fix**: Nov 2020 — required DC enforcement mode Feb 2021

## Mitigation Status
- **Patch**: August 2020 security update
- **Mitigation**: Domain Controller enforcement mode (requires all domain-connected devices to use secure RPC) — enabled by default Feb 2021
- **Detection**: Windows Event ID 5827 (Denied Netlogon), 5828 (Allowed Netlogon with vulnerable connection)
- **2026 status**: Fully patched in mainstream but ZeroLogon is still attempted against legacy/slow-to-patch orgs

## PoC & References
- **Original**: Secura BV — zerologon.com (original disclosure + whitepaper)
- **Impacket zeroLogon**: impacket/examples/zerologon.py (Tom Tervoort's PoC)
- **Metasploit**: auxiliary/admin/dcerpc/cve_2020_1472_zerologon
- **Exploit-DB**: www.exploit-db.com/exploits/48864
- **CISA Advisory**: cisa.gov — Alert AA20-259A (ZeroLogon mass exploitation)
- **Detection**: Event 5827/5828, increase in Netlogon RPC attempts
