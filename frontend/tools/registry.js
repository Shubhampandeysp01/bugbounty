/**
 * Vault tools catalog — single source of truth for sidebar + panes.
 *
 * Categories (keep lean):
 *   wordpress → CMS tools
 *   recon     → asset discovery / surface mapping
 *   web       → website probe / scan / fuzz
 *   local     → scan files on disk
 *
 * To DELETE a tool: remove its object here + server module (see tools/DELETE.md).
 */
window.VAULT_TOOLS = {
  categories: [
    {
      id: 'wordpress',
      label: 'WordPress',
      eyebrow: 'CMS',
      tools: [
        {
          id: 'wordpress-check',
          label: 'Version Check',
          desc: 'Version, REST API, XML-RPC, readme',
          badge: 'Recon',
          title: 'WordPress Version Check',
          blurb: 'Fingerprint a site by probing public WordPress endpoints.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-check',
          render: 'wordpress',
        },
        {
          id: 'wordpress-users',
          label: 'User Enum',
          desc: 'REST + author archive users',
          badge: 'Enum',
          title: 'WordPress User Enumeration',
          blurb: 'Find users via /wp-json/wp/v2/users, ?author=N archives, and post author IDs.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          extras: [
            {
              type: 'text',
              name: 'max_id',
              placeholder: '10',
              label: 'Max author ID',
            },
          ],
          endpoint: '/api/tools/wordpress-users',
          render: 'wp-users',
        },
        {
          id: 'wordpress-plugins',
          label: 'Plugin Enum',
          desc: 'Popular plugins + versions',
          badge: 'Enum',
          title: 'WordPress Plugin Enumeration',
          blurb: 'Probe popular plugin paths and readme.txt for versions; also parse HTML references.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-plugins',
          render: 'wp-list',
        },
        {
          id: 'wordpress-themes',
          label: 'Theme Enum',
          desc: 'Active + installed themes',
          badge: 'Enum',
          title: 'WordPress Theme Enumeration',
          blurb: 'Detect themes from HTML + style.css (name/version). Guess active theme from page assets.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-themes',
          render: 'wp-themes',
        },
        {
          id: 'wordpress-xmlrpc',
          label: 'XML-RPC Probe',
          desc: 'Methods, multicall, pingback',
          badge: 'Surface',
          title: 'WordPress XML-RPC Probe',
          blurb: 'Deep-check xmlrpc.php: system.listMethods, multicall, pingback, and interesting methods.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-xmlrpc',
          render: 'wp-xmlrpc',
        },
        {
          id: 'wordpress-paths',
          label: 'Sensitive Paths',
          desc: 'Backups, debug.log, config leaks',
          badge: 'Discovery',
          title: 'WordPress Sensitive Paths',
          blurb: 'Hunt debug logs, config backups, migration dumps, .git, install leftovers, and more.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-paths',
          render: 'wp-paths',
        },
        {
          id: 'wordpress-rest',
          label: 'REST Surface',
          desc: 'Namespaces & risky routes',
          badge: 'Map',
          title: 'WordPress REST API Surface',
          blurb: 'Map /wp-json namespaces and flag interesting routes (users, auth, WooCommerce, forms…).',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-rest',
          render: 'wp-rest',
        },
        {
          id: 'wordpress-nuclei',
          label: 'WP Nuclei Scan',
          desc: 'Nuclei WordPress templates',
          badge: 'Scan',
          title: 'WordPress Nuclei Scan',
          blurb: 'Run nuclei with WordPress tags against the target. Requires nuclei + updated templates.',
          binary: 'nuclei',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          extras: [
            {
              type: 'text',
              name: 'severity',
              placeholder: 'low,medium,high,critical',
              label: 'Severity',
            },
            {
              type: 'text',
              name: 'tags',
              placeholder: 'wordpress',
              label: 'Tags',
            },
          ],
          endpoint: '/api/tools/wordpress-nuclei',
          render: 'nuclei',
        },
        {
          id: 'wordpress-vuln-scan',
          label: 'WF Vuln Scanner',
          desc: 'Wordfence DB · CVE + CVSS + fixes',
          badge: 'Intel',
          title: 'Wordfence Vulnerability Scanner',
          blurb:
            'Detect WordPress core, plugins, and themes — then match versions against the local Wordfence Intelligence database (CVE, CVSS, remediation). Use Refresh DB to pull the latest feed.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/wordpress-vuln-scan',
          render: 'wp-vuln-scan',
          dbRefresh: true,
        },
      ],
    },
    {
      id: 'recon',
      label: 'Recon',
      eyebrow: 'Discovery',
      tools: [
        {
          id: 'subfinder-enum',
          label: 'Subdomain Enum',
          desc: 'Subfinder passive enumeration',
          badge: 'Enum',
          title: 'Subdomain Enumeration (subfinder)',
          blurb: 'Passive subdomain discovery from public sources. -all pulls every source (needs provider API keys for best results).',
          binary: 'subfinder',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'example.com',
            label: 'Target domain',
          },
          extras: [
            {
              type: 'text',
              name: 'all',
              placeholder: '1',
              label: 'All sources (1)',
            },
          ],
          endpoint: '/api/tools/subfinder',
          render: 'subdomains',
        },
        {
          id: 'waybackurls-mine',
          label: 'Archive URLs',
          desc: 'Historical URLs from the Wayback Machine',
          badge: 'Mine',
          title: 'Archive URL Mining (waybackurls)',
          blurb: 'Pull every archived URL for a domain from the Internet Archive — great for finding old endpoints, params, and exposed files.',
          binary: 'waybackurls',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'example.com',
            label: 'Target domain',
          },
          endpoint: '/api/tools/waybackurls',
          render: 'url-list',
        },
        {
          id: 'katana-crawl',
          label: 'Crawler',
          desc: 'Katana crawl with JS execution',
          badge: 'Crawl',
          title: 'Crawler (katana)',
          blurb: 'Crawl the site and enumerate endpoints, forms, and JS-discovered links. JS crawling is enabled; cap depth to stay fast.',
          binary: 'katana',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          extras: [
            {
              type: 'text',
              name: 'depth',
              placeholder: '2',
              label: 'Crawl depth',
            },
          ],
          endpoint: '/api/tools/katana',
          render: 'url-list',
        },
        {
          id: 'js-analysis',
          label: 'JS Analysis',
          desc: 'Endpoints + secrets from JS',
          badge: 'Intel',
          title: 'JavaScript Analysis',
          blurb: 'Fetch a page (or a raw .js URL) and mine endpoints, API routes, and secret-looking strings from all loaded scripts.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com  or  https://…/app.js',
            label: 'Target URL',
          },
          endpoint: '/api/tools/js-analysis',
          render: 'js-analysis',
        },
      ],
    },
    {
      id: 'web',
      label: 'Websites',
      eyebrow: 'Targets',
      tools: [
        {
          id: 'httpx-probe',
          label: 'Live Probe',
          desc: 'Status, title, tech, server',
          badge: 'Probe',
          title: 'Live Probe (httpx)',
          blurb: 'Quick HTTP fingerprint — status code, title, tech stack, IP, web server.',
          binary: 'httpx',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/httpx',
          render: 'httpx',
        },
        {
          id: 'nuclei-scan',
          label: 'Vuln Scan',
          desc: 'Template-based vulnerability scan',
          badge: 'Scan',
          title: 'Vuln Scan (nuclei)',
          blurb: 'Run nuclei templates against a URL (medium+ by default, no Interactsh). First run: nuclei -update-templates.',
          binary: 'nuclei',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          extras: [
            {
              type: 'text',
              name: 'severity',
              placeholder: 'medium,high,critical',
              label: 'Severity',
            },
            {
              type: 'text',
              name: 'tags',
              placeholder: 'cve,misconfig (optional)',
              label: 'Tags',
            },
          ],
          endpoint: '/api/tools/nuclei',
          render: 'nuclei',
        },
        {
          id: 'ffuf-fuzz',
          label: 'Path Fuzz',
          desc: 'Discover hidden paths',
          badge: 'Fuzz',
          title: 'Path Fuzz (ffuf)',
          blurb: 'Directory/path fuzzing with the bundled common-paths wordlist. Appends /FUZZ if missing.',
          binary: 'ffuf',
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/ffuf',
          render: 'ffuf',
        },
        {
          id: 'cors-check',
          label: 'CORS Check',
          desc: 'Misconfiguration probe',
          badge: 'Config',
          title: 'CORS Misconfiguration Check',
          blurb:
            'Send requests with attacker-controlled Origin headers and flag reflected / credentialed `Access-Control-Allow-Origin` responses.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://api.example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/cors-check',
          render: 'cors-check',
        },
        {
          id: 'open-redirect',
          label: 'Open Redirect',
          desc: 'Probe redirect params',
          badge: 'Redirect',
          title: 'Open Redirect Check',
          blurb:
            'Append common redirect parameters (url, redirect, next, …) with an off-site payload and flag any 3xx that redirects away.',
          binary: null,
          input: {
            type: 'url',
            name: 'url',
            placeholder: 'https://example.com',
            label: 'Target URL',
          },
          endpoint: '/api/tools/open-redirect',
          render: 'open-redirect',
        },
      ],
    },
    {
      id: 'local',
      label: 'Local Files',
      eyebrow: 'Disk',
      tools: [
        {
          id: 'gitleaks-scan',
          label: 'Secrets Scan',
          desc: 'Find leaked keys in a folder',
          badge: 'Secrets',
          title: 'Secrets Scan (gitleaks)',
          blurb: 'Scan a local directory or git repo for secrets. Default path is this Vault repo (.).',
          binary: 'gitleaks',
          input: {
            type: 'text',
            name: 'path',
            placeholder: '.  or  /path/to/repo',
            label: 'Local path',
            defaultValue: '.',
          },
          endpoint: '/api/tools/gitleaks',
          render: 'gitleaks',
        },
        {
          id: 'trivy-scan',
          label: 'FS Vuln Scan',
          desc: 'Vulns, secrets, misconfig on disk',
          badge: 'Supply chain',
          title: 'Filesystem Scan (trivy)',
          blurb: 'Trivy fs scan for HIGH/CRITICAL vulns, secrets, and misconfigurations. Needs network for DB updates.',
          binary: 'trivy',
          input: {
            type: 'text',
            name: 'path',
            placeholder: '.  or  /path/to/project',
            label: 'Local path',
            defaultValue: '.',
          },
          endpoint: '/api/tools/trivy',
          render: 'trivy',
        },
      ],
    },
  ],
};

window.VAULT_TOOLS.allTools = function allTools() {
  return window.VAULT_TOOLS.categories.flatMap((c) =>
    c.tools.map((t) => ({ ...t, categoryId: c.id, categoryLabel: c.label }))
  );
};

window.VAULT_TOOLS.getTool = function getTool(id) {
  return window.VAULT_TOOLS.allTools().find((t) => t.id === id) || null;
};

window.VAULT_TOOLS.count = function count() {
  return window.VAULT_TOOLS.allTools().length;
};
