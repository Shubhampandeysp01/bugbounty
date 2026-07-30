# WordPress Recon & Enumeration

## Version Detection

### 1. Generator Meta Tag
```html
<meta name="generator" content="WordPress 7.0.2" />
```

### 2. CSS/JS Version Parameters
```
/wp-includes/css/dashicons.min.css?ver=7.0.2
/wp-includes/js/jquery/jquery.min.js?ver=3.7.1
```

### 3. readme.html
```
GET /readme.html
```
If accessible, reveals version info. Often removed in hardened installs.

### 4. REST API Index
```
GET /wp-json/
```
Returns `site_version` in some configurations. Also reveals registered namespaces (plugins).

### 5. Feed URLs
```
GET /?feed=rss2
GET /feed/
```
The generator tag in RSS feeds often includes the version.

### 6. oEmbed
```
GET /wp-json/oembed/1.0/embed?url=<site-url>
```

## User Enumeration

### REST API (unauthenticated)
```
GET /wp-json/wp/v2/users
GET /wp-json/wp/v2/users?per_page=100
```
Returns `id`, `name`, `slug`, `avatar_urls`. **Most common info leak.**

### Author Archives
```
GET /?author=1   → 301 redirect to /author/adminname/
GET /?author=2   → 301 redirect to /author/username2/
```

### Author Sitemap (Yoast/Rank Math)
```
GET /author-sitemap.xml
```

### wp-json/users with custom post types
```
GET /wp-json/wp/v2/users?whove=authors
GET /wp-json/wp/v2/users?context=edit  (requires auth, but worth checking)
```

## Plugin Fingerprinting

### 1. Static Asset Detection
Check for plugin-specific CSS/JS files:
```
/wp-content/plugins/wordpress-seo/css/main-sitemap.xsl
/wp-content/plugins/akismet/_inc/akismet.css
/wp-content/plugins/elementor/assets/css/frontend.min.css
```

### 2. REST API Namespaces
```
GET /wp-json/
```
Each registered namespace reveals a plugin:
- `yoast/v1` → Yoast SEO
- `akismet/v1` → Akismet
- `elementor/v1` → Elementor
- `rankmath/v1` → Rank Math SEO
- `wpforms/v1` → WPForms
- `litespeed/v1` → LiteSpeed Cache
- `metaslider/v1` → MetaSlider
- `google-site-kit/v1` → Site Kit by Google

### 3. Readme Files (if accessible)
```
GET /wp-content/plugins/<plugin-name>/readme.txt
GET /wp-content/plugins/<plugin-name>/README.txt
```

### 4. Error Messages
Trigger 404s or errors to reveal plugin paths in stack traces.

### 5. WPScan / Wapiti
Automated fingerprinting tools (see `05-tools-resources.md`).

## Theme Fingerprinting

### Style.css
```
GET /wp-content/themes/<theme-name>/style.css
```
Reveals theme name, version, author, and "Tested up to" WordPress version.

### Screenshot
```
GET /wp-content/themes/<theme-name>/screenshot.png
```

## Sensitive File Discovery

| Path | What it reveals |
|------|----------------|
| `/readme.html` | WordPress version requirements |
| `/license.txt` | WordPress license (confirms WP) |
| `/wp-config.php` | Database credentials (should 404) |
| `/wp-config.php.bak` | Backup of config |
| `/wp-config.php.old` | Old config |
| `/.wp-config.php.swp` | Vim swap file |
| `/.env` | Environment variables |
| `/.git/config` | Git repo exposure |
| `/wp-content/debug.log` | PHP error log |
| `/wp-content/uploads/` | Directory listing (if enabled) |
| `/wp-json/wp/v2/settings` | Site settings (requires auth) |
| `/wp-json/wp/v2/plugins` | Plugin list (requires auth) |

## Server Header Analysis
```
curl -sI https://target.com/
```
- `Server: Apache/2.4.57` → Apache version
- `X-Powered-By: PHP/8.1.22` → PHP version
- `Set-Cookie` → Session handling details

## WAF Detection
```
curl -sI https://target.com/ -H "User-Agent: malicious"
```
Compare responses to detect WAF presence (Cloudflare, Sucuri, Wordfence, etc.).
