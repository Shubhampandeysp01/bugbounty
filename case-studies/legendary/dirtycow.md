# Dirty COW (CVE-2016-5195)

## Classification
- **CVE**: CVE-2016-5195
- **CVSS 3.0**: 7.8 (High)
- **Type**: Linux Kernel Race Condition — Local Privilege Escalation
- **Affected**: Linux kernel 2.6.22+ (2007) through 4.8.3 (2016) — **~12 years of kernels**
- **Wild Impact**: All Android devices (4.4+), all Linux servers, all Docker containers, all Chromebooks

## Root Cause

A **race condition between `madvise(MADV_DONTNEED)` and the page fault handler's COW (Copy-On-Write) path**. The kernel's get_user_pages() function used by `ptrace` / `write()` on `proc/self/mem` has a TOCTOU (Time-of-Check-Time-of-Use) vulnerability.

**High-level explanation:**
1. Attacker opens `/proc/self/mem` and maps read-only system file (e.g., `/etc/passwd`)
2. Kernel uses get_user_pages() to resolve the writable page — but the page is read-only
3. Kernel starts COW (Copy-On-Write) to create a private writable copy
4. **BEFORE the COW completes**, attacker calls `madvise(MADV_DONTNEED)` on the page
5. Kernel discards the **private** page (since MADV_DONTNEED tells it to)
6. **Kernel now writes the new data to the ORIGINAL read-only page** — the mapped file

```c
// Simplified race:
// Thread 1: Write to /proc/self/mem (targeting a read-only mmap'd file)
// Thread 2: madvise() on the same memory region

// Thread 1                                          // Thread 2
// ----                                               ----
page = get_user_pages(addr, FOLL_WRITE);
// Page is COW — private copy created               
// Writes to private copy                            
                                                      madvise(addr, len, MADV_DONTNEED);
                                                      // Private copy is discarded!
// get_user_pages resolves page again                 
// BUT: now resolves to ORIGINAL read-only page!     
// Writes directly to original page →                 
// /etc/passwd is now modified! <-- PRIVILEGE ESCALATION
```

## Exploit Chain

1. **Open read-only file**: `open("/etc/passwd")` — attacker creates a read-only mapping
2. **Spawn race threads**:
   - Thread A: continuously calls `write()` to the mapped page (triggers COW fault)
   - Thread B: continuously calls `madvise(MADV_DONTNEED)` on the same page
3. **Win the race**: After millions of iterations, the race condition triggers — write lands on the original read-only page
4. **Overwrite `root:x:0:0:...`** with `root::0:0:...` (remove password hash)
5. `su root` — no password needed → full root shell

## Why It's Legendary

- **12 years of vulnerability** — from 2007 through late 2016
- **All kernels affected** — desktops, servers, Android phones, Docker containers, embedded devices
- **Extremely stable exploit** — easily won the race with thread priority tricks
- **Bypassed all Android security** — chain to root every Android device pre-2017
- **Perfect race**: The window was small but reliably exploitable with enough loop iterations

## Pattern Recognition

1. **COW with concurrent memory management**: Whenever the page cache interacts with user-space memory management syscalls (madvise, munmap, mprotect)
2. **get_user_pages + file-backed VMA**: The core of the bug — getting write access to MAP_SHARED read-only page
3. **TOCTOU with kernel memory operations**: The pattern of "check permission, then use" with a gap in between
4. **Thread races on memory management**: madvise/mprotect/mlockall during active page operations

## Variants
- **CVE-2017-7537** — another race in `__dquot_initialize()`
- **CVE-2022-0847** (Dirty Pipe) — similar flavor, different mechanism (pipe buffer vs page cache)

## Mitigation Status
- **Patch**: Linux 4.8.3, 4.4.26 LTS, Android patches (Oct 2016)
- **Cleanup**: All distributions pulled updates within days
- **2026 status**: Fully patched but the technique is taught as "the definitive kernel race" in exploitation training

## PoC & References
- **Original exploit**: github.com/dirtycow/dirtycow.github.io
- **dirtycow.github.io**: Official site with PoC code, FAQ, writeups
- **Android exploit**: Dirty COW used in all major Android root tools pre-2017
- **Container escape**: Dirty COW works IN Docker containers to escape to host (no container boundaries for kernel bugs)
