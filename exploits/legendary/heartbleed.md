# Heartbleed (CVE-2014-0160)

## Classification
- **CVE**: CVE-2014-0160
- **CVSS 3.0**: 7.5 (High)
- **Type**: TLS Heartbeat Extension — Missing Bounds Check (OOB Read)
- **Affected**: OpenSSL 1.0.1 → 1.0.1f (released March 2012 — April 2014)
- **Wild Impact**: ~500,000 SSL certificates compromised; catastrophic private key leakage

## Root Cause

The TLS Heartbeat extension (RFC 6520) allows peers to send a heartbeat request to verify the peer is alive. The request contains a payload and a **length field** (2 bytes). The OpenSSL implementation **trusted the length field without checking it against the actual buffer size**.

```c
// Simplified vulnerable code from OpenSSL ssl/d1_both.c
int dtls1_process_heartbeat(SSL *s) {
    unsigned char *p = &s->s3->rrec.data[0];
    unsigned short payload_length = *((unsigned short *)&p[1]);  // attacker-controlled
    unsigned char *payload = &p[3];  // actually only 16 bytes available
    
    // ALLOCATE response buffer using attacker's length
    unsigned char *response = OPENSSL_malloc(1 + 2 + payload_length + 16);
    
    // COPY attacker-controlled payload_length bytes even though
    // actual data may be much smaller
    memcpy(response, payload, payload_length);  // READS BEYOND BUFFER!
    
    // Send response back — contains server's private memory
    dtls1_send_heartbeat(s, response);
}
```

**The critical flaw**: `payload_length` comes from the attacker. The server has only ~16 bytes of heartbeat payload, but if the attacker sends `payload_length = 65535`, the `memcpy` will copy 64KB of server memory starting at the payload pointer. That memory may contain private keys, session data, passwords, etc.

## Exploit Chain

1. Attacker sends a crafted TLS Heartbeat Request with:
   - Actual payload: 1 byte ("x")
   - Claimed payload length: 0xFFFF (65535)
2. Server pads no extra data (it doesn't check the length mismatch)
3. Server allocates response buffer of 65535+1+2+16 bytes
4. Server copies 65535 bytes from the 1-byte payload location — reading 65534 bytes **past the buffer** into heap memory
5. Server sends the oversized response back containing memory contents
6. Attacker repeats thousands of times to harvest all reachable memory

## What Could Be Leaked Per Request
- **SSL private keys** (RSA/DSA/ECDSA) — most damaging
- **Session tickets / session IDs** — session hijacking
- **User credentials** in memory from recent authentication
- **HTTP request data** from other users on shared hosting
- **Database queries** — if server uses database from same process
- **File contents** — if files are cached in memory

## Known Breaches
- **MumsLife / UK Parenting Site**: 36M+ accounts leaked via Heartbleed
- **Canada Revenue Agency**: 900 SSNs stolen, site taken down
- **US Department of Veterans Affairs**: Patient data exposed
- **Dozens of major web properties**: Yahoo, Imgur, OKCupid, StackOverflow, etc.

## Pattern Recognition

1. **Length-mismatch bugs**: The single most common pattern in memory corruption. Any API where the user provides both a buffer and a claimed length.
2. **Protocol field trust**: Heartbeat was a simple feature that nobody audited — "boring" protocol code has fewer eyes but equal risk.
3. **Memory disclosure primitives**: OOB read is just as dangerous as OOB write — you can downgrade from a read to a full compromise via credential/kms theft.
4. **OpenSSL culture**: Complex, performance-critical C code with a small team = inevitable bugs.
5. **Missing sanitization bounds check**: Always verify user-supplied length against actual available data.

## Variants
- **Heartbleed for DTLS** (CVE-2014-0160 same) — same bug in datagram TLS
- **Other OpenSSL OOB**: CVE-2014-3470, CVE-2016-6309, etc.

## Mitigation Status
- **Patch**: OpenSSL 1.0.1g (released April 7, 2014)
- **Revoke/reissue**: All SSL certificates must be revoked and reissued
- **Change passwords**: All user passwords post-Heartbleed exposure period
- **2026 status**: Libraries patched, but the pattern lives on in countless applications

## PoC & References
- **Original PoC**: heartbleed.com (by Neel Mehta, Google Security)
- **Exploit-DB**: www.exploit-db.com/exploits/32745
- **ssltest.py**: github.com/titanous/heartbleeder (check if vulnerable)
- **Cloudflare challenge**: cloudflarechallenge.com/heartbleed (live demo of what leaks)
- **Cure53 analysis**: Analysis of the OpenSSL "heartbleed" vulnerability
- **XKCD 1354**: "Heartbleed" — iconic comic explaining the severity
