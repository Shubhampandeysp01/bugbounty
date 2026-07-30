# Plugin & Theme Vulnerability Research

## Methodology

### 1. Identify All Plugins
- REST API namespaces (`/wp-json/`)
- Static assets (`/wp-content/plugins/<name>/`)
- Readme files (`/wp-content/plugins/<name>/readme.txt`)
- Sitemap XSL references
- HTML source comments

### 2. Version Detection
- CSS/JS `?ver=` parameters
- Readme.txt "Stable tag" field
- Changelog entries
- GitHub tags/releases

### 3. CVE Research
Check each plugin version against:
- [WPScan Vulnerability Database](https://wpscan.com/)
- [CVE Mitre](https://cve.mitre.org/)
- [NVD NIST](https://nvd.nist.gov/)
- [Exploit-DB](https://www.exploit-db.com/)
- Plugin's own changelog (look for "security fix" entries)

### 4. Source Code Review
For open-source plugins, download and audit:
```bash
# Download plugin from WordPress.org
wget https://downloads.wordpress.org/plugin/<plugin-name>.<version>.zip
unzip <plugin-name>.<version>.zip

# Search for dangerous functions
grep -rn "unserialize\|eval\|assert\|preg_replace.*\/e\|system\|exec\|shell_exec\|passthru\|call_user_func\|extract\|parse_str\|include(\$\|require(\$\|file_get_contents.*\$\|fopen.*\$\|move_uploaded_file\|wp_remote_get.*\$\|add_query_arg.*\$\|wp_nonce_field.*\$_" .
```

## Common Plugin Vulnerability Patterns

### 1. SQL Injection
```php
// Vulnerable: direct query with unsanitized input
$wpdb->query("SELECT * FROM {$wpdb->prefix}posts WHERE ID = " . $_GET['id']);

// Safe: prepared statement
$wpdb->query($wpdb->prepare("SELECT * FROM {$wpdb->prefix}posts WHERE ID = %d", $_GET['id']));
```

### 2. Stored XSS
```php
// Vulnerable: no sanitization on save
update_post_meta($post_id, 'custom_field', $_POST['custom_field']);

// Vulnerable: no escaping on output
echo get_post_meta($post_id, 'custom_field', true);
```

### 3. File Upload / RCE
```php
// Vulnerable: no file type validation
move_uploaded_file($_FILES['file']['tmp_name'], WP_CONTENT_DIR . '/uploads/' . $_FILES['file']['name']);

// Vulnerable: dangerous file type allowed
$allowed_types = ['jpg', 'png', 'gif', 'php'];  // PHP should NOT be allowed
```

### 4. CSRF (Missing Nonce)
```php
// Vulnerable: no nonce check
if (isset($_POST['save_settings'])) {
    update_option('my_plugin_option', $_POST['option_value']);
}

// Safe: nonce check
if (isset($_POST['save_settings']) && wp_verify_nonce($_POST['_wpnonce'], 'my_plugin_action')) {
    update_option('my_plugin_option', $_POST['option_value']);
}
```

### 5. SSRF
```php
// Vulnerable: no URL validation
$response = wp_remote_get($_GET['url']);

// Safe: domain allowlist
$allowed = ['api.example.com'];
$parsed = parse_url($_GET['url']);
if (in_array($parsed['host'], $allowed)) {
    $response = wp_remote_get($_GET['url']);
}
```

### 6. Privilege Escalation
```php
// Vulnerable: no capability check
add_action('wp_ajax_my_action', 'my_action_callback');
function my_action_callback() {
    update_user_meta(get_current_user_id(), 'role', 'administrator');
}

// Safe: capability check
add_action('wp_ajax_my_action', 'my_action_callback');
function my_action_callback() {
    if (!current_user_can('manage_options')) return;
    // ...
}
```

### 7. PHP Object Injection
```php
// Vulnerable: unserializing user input
$data = unserialize($_POST['data']);

// Safe: JSON instead
$data = json_decode($_POST['data'], true);
```

## High-Value Plugin Targets

### Page Builders
- **Elementor** — Large attack surface, many AJAX actions
- **WPBakery** — Legacy code, file operations
- **Divi / Visual Builder** — Shortcode parsing, file uploads
- **Beaver Builder** — Template imports

### SEO Plugins
- **Yoast SEO** — XML sitemap generation, import/export
- **Rank Math SEO** — Rich snippet schema, AI features
- **All in One SEO** — Legacy codebase

### Caching / Performance
- **W3 Total Cache** — Database caching, file operations
- **WP Super Cache** — File generation
- **LiteSpeed Cache** — Server-level integration
- **WP Rocket** — Premium, file optimization

### Security Plugins (ironically)
- **Wordfence** — Firewall bypass, rate limiting bypass
- **Sucuri** — Malware scanner bypass
- **iThemes Security** — Feature conflicts

### E-commerce
- **WooCommerce** — Payment processing, order management
- **Easy Digital Downloads** — File downloads, payment handling

### Forms
- **Contact Form 7** — File uploads, mail handling
- **WPForms** — File uploads, payment integrations
- **Gravity Forms** — Premium, file operations
- **Formidable Forms** — Database queries

## Theme Vulnerability Patterns

### 1. XSS in Theme Options
Themes with customizer options often store unsanitized data.

### 2. LFI in Template Includes
```php
// Vulnerable theme pattern
include(get_template_directory() . '/templates/' . $_GET['template'] . '.php');
```

### 3. CSRF in Theme Settings
Missing nonce checks in theme settings pages.

### 4. Arbitrary File Upload
Themes with demo import functionality may allow arbitrary file uploads.

## Research Workflow

```bash
# 1. Fingerprint target
wpscan --url https://target.com --api-token YOUR_TOKEN

# 2. Check specific plugin
wpscan --url https://target.com --plugins-detection aggressive

# 3. Manual version check
curl -s https://target.com/wp-content/plugins/<plugin>/readme.txt | grep "Stable tag"

# 4. Search for known vulnerabilities
# Visit: https://wpscan.com/search?w=<plugin-name>

# 5. Download and audit
wget https://downloads.wordpress.org/plugin/<plugin>.<version>.zip
unzip -d /tmp/audit <plugin>.<version>.zip
cd /tmp/audit && grep -rn "_\$_\|_\$_POST\|_\$_GET\|_\$_REQUEST\|_\$_FILES\|_\$_SERVER" --include="*.php" | grep -v "nonce\|_wpnonce\|wp_verify_nonce" | head -50
```
