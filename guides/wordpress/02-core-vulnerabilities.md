# WordPress Core Vulnerabilities

## Recent Critical CVEs (2024–2026)

### CVE-2024-XXXX — REST API Batch Vulnerability (SQLi → RCE)
**Affected**: WordPress 6.90–6.94, 7.0–7.01
**Fixed in**: 7.0.2
**Type**: SQL Injection → Privilege Escalation → RCE

**The Exploit Chain**:
1. Call unauthenticated `/wp-json/batch/v1` endpoint
2. Send a POST to `/wp/v2/posts` with a nested request body
3. The nested request bypasses the method allowlist → allows GET requests
4. GET request to a non-existent post triggers a desync in `WP_REST_Server::dispatch()`
5. The desync causes `author_exclude` parameter to be interpolated into SQL as a raw string
6. SQL injection creates a new admin user
7. Authenticated as admin → upload malicious plugin (web shell)
8. Full site compromise

**Key files**: `wp-includes/rest-api/endpoints/class-wp-rest-posts-controller.php`, `wp-includes/rest-api/class-wp-rest-server.php`

**Detection**:
```bash
# Check if batch endpoint is accessible
curl -s -X POST https://target.com/wp-json/batch/v1 \
  -H "Content-Type: application/json" \
  -d '{"requests":[{"method":"POST","path":"/wp/v2/posts","body":{"title":"test"}}]}'
# 401 = patched (auth required), 200 = vulnerable
```

### CVE-2024-XXXX — Stored XSS via Comment System
**Affected**: WordPress < 6.8
**Type**: Stored XSS
Comments with crafted payloads bypassing `wp_kses()` filtering.

### CVE-2024-XXXX — Privilege Escalation via Application Passwords
**Affected**: WordPress < 6.7
**Type**: Auth Bypass
Application passwords API allowed creating passwords with higher privileges than the user.

### CVE-2023-XXXX — Shortcode Execution in Comments
**Affected**: WordPress < 6.3
**Type**: Stored XSS
Comments containing shortcodes like `[audio src="xss"]` could execute JavaScript.

## Classic WordPress Vulnerabilities

### SQL Injection
- **Author query params**: `author`, `author_name`, `author__in`, `author__not_in`
- **Meta queries**: `meta_key`, `meta_value`, `meta_compare`
- **Order/Orderby**: Custom `orderby` parameters in WP_Query
- **Legacy**: `GET /?p=-1' UNION SELECT...`

### Cross-Site Scripting (XSS)
- **Stored**: Comments, user display names, post content
- **Reflected**: Search results, 404 templates, login error messages
- **DOM-based**: Media library, customizer preview

### Cross-Site Request Forgery (CSRF)
- **Admin actions**: Plugin activate/deactivate, user creation, settings changes
- **Nonce bypass**: Weak nonce generation in older versions

### Server-Side Request Forgery (SSRF)
- **Pingback**: `/xmlrpc.php` pingback.ping can be used to scan internal networks
- **Media import**: `media/new` endpoint fetches URLs
- **Link preview**: oEmbed proxy fetches external URLs

### Path Traversal / LFI
- **Template inclusion**: `?page_id=../../../etc/passwd`
- **Download handler**: `?wpdm-file=../../../wp-config.php`
- **Theme editor**: Template file selection

### Deserialization
- **PHP Object Injection**: `unserialize()` in `maybe_unserialize()` with crafted cookies
- **Phar deserialization**: `phar://` wrapper in file operations

## XML-RPC Attack Surface

### Methods Available
| Method | Risk |
|--------|------|
| `system.multicall` | Brute-force passwords in bulk |
| `pingback.ping` | SSRF, DDoS amplification |
| `wp.getUsersBlogs` | User enumeration |
| `wp.getOptions` | Version disclosure |
| `wp.getComments` | Comment data extraction |

### Disable XML-RPC
```apache
# .htaccess
<Files xmlrpc.php>
  Require all denied
</Files>
```

## REST API Attack Surface

### Unauthenticated Endpoints
| Endpoint | Risk |
|----------|------|
| `GET /wp/v2/users` | User enumeration |
| `GET /wp/v2/posts` | Post data (including drafts in some configs) |
| `GET /wp/v2/comments` | Comment data, email addresses |
| `POST /batch/v1` | Batch request processing |
| `GET /oembed/1.0/proxy` | SSRF via oEmbed |

### Authenticated Endpoints (low-privilege)
| Endpoint | Risk |
|----------|------|
| `POST /wp/v2/posts` | Create posts (author+) |
| `POST /wp/v2/media` | Upload files (author+) |
| `POST /wp/v2/users/me/application-passwords` | Create app passwords |
| `PUT /wp/v2/users/me` | Update profile (XSS via display_name) |

## SSRF via oEmbed Proxy
```bash
# Test oEmbed SSRF
curl -s "https://target.com/wp-json/oembed/1.0/proxy?url=http://169.254.169.254/latest/meta-data/"
```

## SSRF via Pingback
```bash
# Test pingback SSRF
curl -s -X POST https://target.com/xmlrpc.php \
  -H "Content-Type: text/xml" \
  -d '<?xml version="1.0"?>
  <methodCall>
    <methodName>pingback.ping</methodName>
    <params>
      <param><value><string>http://internal-server:8080/</string></value></param>
      <param><value><string>https://target.com/some-post/</string></value></param>
    </params>
  </methodCall>'
```
