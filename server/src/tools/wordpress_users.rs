//! WordPress → User enumeration — DELETE file + route + registry to remove.
//! Techniques: REST /wp/v2/users, author archives (?author=N), oEmbed author.

use axum::{extract::Query, http::StatusCode, response::Json};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use super::common::{http_client, normalize_url};

#[derive(Debug, Clone, Serialize)]
pub struct WpUser {
    pub id: Option<u64>,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub link: Option<String>,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct WpUsersResponse {
    pub url: String,
    pub users: Vec<WpUser>,
    pub rest_users_enabled: bool,
    pub author_enum_works: bool,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

pub async fn wordpress_users(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<WpUsersResponse>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()))?;
    let base = normalize_url(url).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let max_author: u64 = params
        .get("max_id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
        .clamp(1, 25);

    let client = http_client(12).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut users: Vec<WpUser> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut notes = Vec::new();
    let mut rest_users_enabled = false;
    let mut author_enum_works = false;
    let mut error = None;

    // 1) REST API users endpoint (often blocked on hardened sites)
    let rest_url = format!("{base}/wp-json/wp/v2/users?per_page=100");
    match client.get(&rest_url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if status == 200 {
                if let Ok(body) = resp.text().await {
                    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&body) {
                        rest_users_enabled = true;
                        for u in arr {
                            let id = u.get("id").and_then(|v| v.as_u64());
                            let slug = u
                                .get("slug")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let name = u
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let link = u
                                .get("link")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let key = format!(
                                "{}:{}",
                                id.map(|i| i.to_string()).unwrap_or_default(),
                                slug.clone().unwrap_or_default()
                            );
                            if seen.insert(key) {
                                users.push(WpUser {
                                    id,
                                    slug,
                                    name,
                                    link,
                                    source: "rest_api".into(),
                                });
                            }
                        }
                        notes.push(format!("REST /wp/v2/users returned {} user(s)", users.len()));
                    }
                }
            } else if status == 401 || status == 403 {
                notes.push(format!(
                    "REST /wp/v2/users blocked (HTTP {status}) — good hardening"
                ));
            } else {
                notes.push(format!("REST /wp/v2/users → HTTP {status}"));
            }
        }
        Err(e) => {
            error = Some(format!("REST request failed: {e}"));
        }
    }

    // 2) Author archive enum (?author=1..N) — classic technique
    for id in 1..=max_author {
        let author_url = format!("{base}/?author={id}");
        if let Ok(resp) = client.get(&author_url).send().await {
            let final_url = resp.url().to_string();
            let status = resp.status().as_u16();
            if status == 200 {
                // Redirect to /author/slug/ or body contains author name
                if let Some(slug) = extract_author_slug(&final_url) {
                    author_enum_works = true;
                    let key = format!("{id}:{slug}");
                    if seen.insert(key) {
                        users.push(WpUser {
                            id: Some(id),
                            slug: Some(slug.clone()),
                            name: None,
                            link: Some(final_url.clone()),
                            source: "author_archive".into(),
                        });
                    }
                } else if let Ok(body) = resp.text().await {
                    if body.contains("/author/") {
                        if let Some(slug) = extract_author_from_body(&body) {
                            author_enum_works = true;
                            let key = format!("{id}:{slug}");
                            if seen.insert(key) {
                                users.push(WpUser {
                                    id: Some(id),
                                    slug: Some(slug),
                                    name: None,
                                    link: Some(author_url),
                                    source: "author_archive_body".into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    if author_enum_works {
        notes.push(format!(
            "Author archive enum works (probed IDs 1–{max_author})"
        ));
    } else {
        notes.push(format!(
            "Author archive enum: no hits for IDs 1–{max_author}"
        ));
    }

    // 3) oEmbed can leak author on some posts via REST search
    let posts_url = format!("{base}/wp-json/wp/v2/posts?per_page=5");
    if let Ok(resp) = client.get(&posts_url).send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.text().await {
                if let Ok(posts) = serde_json::from_str::<Vec<serde_json::Value>>(&body) {
                    for p in posts {
                        if let Some(author) = p.get("author").and_then(|v| v.as_u64()) {
                            let key = format!("{author}:");
                            // only add if we don't have this id yet
                            if !users.iter().any(|u| u.id == Some(author)) && seen.insert(key) {
                                users.push(WpUser {
                                    id: Some(author),
                                    slug: None,
                                    name: None,
                                    link: None,
                                    source: "posts_author_id".into(),
                                });
                            }
                        }
                        // yoast / author_name in embedded sometimes
                        if let Some(name) = p
                            .pointer("/_embedded/author/0/name")
                            .and_then(|v| v.as_str())
                        {
                            let slug = p
                                .pointer("/_embedded/author/0/slug")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let id = p
                                .pointer("/_embedded/author/0/id")
                                .and_then(|v| v.as_u64());
                            let key = format!(
                                "{}:{}",
                                id.map(|i| i.to_string()).unwrap_or_default(),
                                slug.clone().unwrap_or_else(|| name.to_string())
                            );
                            if seen.insert(key) {
                                users.push(WpUser {
                                    id,
                                    slug,
                                    name: Some(name.to_string()),
                                    link: None,
                                    source: "embedded_author".into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    users.sort_by_key(|a| a.id);

    Ok(Json(WpUsersResponse {
        url: base,
        users,
        rest_users_enabled,
        author_enum_works,
        notes,
        error,
    }))
}

fn extract_author_slug(url: &str) -> Option<String> {
    // .../author/slug/ or .../author/slug
    let marker = "/author/";
    let idx = url.find(marker)?;
    let rest = &url[idx + marker.len()..];
    let slug: String = rest
        .split(['/', '?', '#'])
        .next()?
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    if slug.is_empty() || slug == "page" {
        None
    } else {
        Some(slug)
    }
}

fn extract_author_from_body(body: &str) -> Option<String> {
    let marker = "/author/";
    let idx = body.find(marker)?;
    let rest = &body[idx + marker.len()..];
    let slug: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}
