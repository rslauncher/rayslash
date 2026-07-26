use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use crate::config::WebSearchConfig;

pub const DEFAULT_SEARCH_KEYWORD: &str = "search";
const MIN_SHARP_FAVICON_SIZE: u32 = 48;
const FAVICON_FAILURE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_FAVICON_CACHE_FILES: usize = 64;
const MAX_FAVICON_BYTES: u64 = 4 * 1024 * 1024;
const MAX_HTML_BYTES: u64 = 1024 * 1024;

pub fn is_valid_template(template: &WebSearchConfig) -> bool {
    !template.name.trim().is_empty()
        && !template.keyword.trim().is_empty()
        && template.url.contains("%s")
        && host_from_url(&template.url).is_some()
}

pub fn trigger_from_input<'a>(
    templates: &'a [WebSearchConfig],
    input: &str,
) -> Option<&'a WebSearchConfig> {
    let input = input.trim();
    templates
        .iter()
        .filter(|template| template.enabled && is_valid_template(template))
        .find(|template| input.eq_ignore_ascii_case(template.keyword.trim()))
}

pub fn host_from_url(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim();
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

pub fn cached_favicon_path(template: &WebSearchConfig) -> Option<PathBuf> {
    let host = host_from_url(&template.url)?;
    let filename = host
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let path = dirs::cache_dir()?
        .join("rayslash/search-favicons")
        .join(format!("{filename}.png"));
    path.is_file().then_some(path)
}

pub fn fetch_and_cache_favicon(template: &WebSearchConfig) -> Option<PathBuf> {
    if !is_valid_template(template)
        || template
            .keyword
            .eq_ignore_ascii_case(DEFAULT_SEARCH_KEYWORD)
    {
        return None;
    }
    if let Some(path) = cached_favicon_path(template)
        && favicon_is_sharp(&path)
    {
        return Some(path);
    }
    let host = host_from_url(&template.url)?;
    let scheme = if template.url.trim().starts_with("http://") {
        "http"
    } else {
        "https"
    };
    let origin = format!("{scheme}://{host}");
    let cache_dir = dirs::cache_dir()?.join("rayslash/search-favicons");
    let failure_path = cache_dir.join(format!("{}.failed", cache_stem(&host)));
    if failure_cache_is_fresh(&failure_path) {
        return None;
    }
    let mut candidates = favicon_candidates(&origin);
    candidates.extend([
        format!("{origin}/apple-touch-icon.png"),
        format!("{origin}/favicon-192x192.png"),
        format!("{origin}/favicon-128x128.png"),
        format!("{origin}/favicon-96x96.png"),
    ]);
    candidates.push(format!("{origin}/favicon.ico"));

    let Some(favicon) = candidates.into_iter().find_map(|url| fetch_favicon(&url)) else {
        fs::create_dir_all(&cache_dir).ok()?;
        let _ =
            crate::atomic_write::write_bytes(&failure_path, unix_seconds().to_string().as_bytes());
        return None;
    };
    fs::create_dir_all(&cache_dir).ok()?;
    let path = cache_dir.join(format!("{}.png", cache_stem(&host)));
    favicon
        .save_with_format(&path, image::ImageFormat::Png)
        .ok()?;
    let _ = fs::remove_file(failure_path);
    prune_favicon_cache(&cache_dir, &path);
    Some(path)
}

fn cache_stem(host: &str) -> String {
    host.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn failure_cache_is_fresh(path: &Path) -> bool {
    let Some(failed_at) = fs::read_to_string(path)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    else {
        return false;
    };
    unix_seconds().saturating_sub(failed_at) <= FAVICON_FAILURE_TTL.as_secs()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn prune_favicon_cache(cache_dir: &Path, keep: &Path) {
    let mut files = fs::read_dir(cache_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            metadata
                .is_file()
                .then(|| (path, metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
        })
        .collect::<Vec<_>>();
    if files.len() <= MAX_FAVICON_CACHE_FILES {
        return;
    }
    files.sort_by_key(|(_, modified)| *modified);
    let remove_count = files.len() - MAX_FAVICON_CACHE_FILES;
    for (path, _) in files
        .into_iter()
        .filter(|(path, _)| path != keep)
        .take(remove_count)
    {
        let _ = fs::remove_file(path);
    }
}

fn favicon_is_sharp(path: &std::path::Path) -> bool {
    image::image_dimensions(path)
        .ok()
        .is_some_and(|(width, height)| {
            width >= MIN_SHARP_FAVICON_SIZE && height >= MIN_SHARP_FAVICON_SIZE
        })
}

fn favicon_candidates(origin: &str) -> Vec<String> {
    let Ok(request) = ureq::http::Request::get(format!("{origin}/"))
        .header("User-Agent", "rayslash/0.1")
        .body(())
    else {
        return Vec::new();
    };
    let Ok(mut response) = http_agent().run(request) else {
        return Vec::new();
    };
    let Ok(html) = response
        .body_mut()
        .with_config()
        .limit(MAX_HTML_BYTES)
        .read_to_string()
    else {
        return Vec::new();
    };

    favicon_candidates_from_html(origin, &html)
}

fn favicon_candidates_from_html(origin: &str, html: &str) -> Vec<String> {
    let lowercase = html.to_ascii_lowercase();
    let mut cursor = 0;
    let mut candidates = Vec::new();

    while let Some(relative_start) = lowercase[cursor..].find("<link") {
        let start = cursor + relative_start;
        let Some(relative_end) = lowercase[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        let tag = &html[start..end];
        let Some(rel) = html_attribute(tag, "rel") else {
            cursor = end;
            continue;
        };
        if !rel.to_ascii_lowercase().contains("icon") {
            cursor = end;
            continue;
        }
        let Some(href) = html_attribute(tag, "href") else {
            cursor = end;
            continue;
        };
        let size = html_attribute(tag, "sizes")
            .as_deref()
            .and_then(icon_size)
            .unwrap_or_else(|| {
                if rel.to_ascii_lowercase().contains("apple-touch") {
                    180
                } else {
                    0
                }
            });
        if let Some(url) = resolve_icon_url(origin, &href) {
            candidates.push((size, url));
        }
        cursor = end;
    }

    candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .map(|(_, url)| url)
        .filter(|url| seen.insert(url.clone()))
        .collect()
}

fn html_attribute(tag: &str, name: &str) -> Option<String> {
    let lowercase = tag.to_ascii_lowercase();
    let bytes = lowercase.as_bytes();
    let mut cursor = 0;

    while let Some(relative_start) = lowercase[cursor..].find(name) {
        let start = cursor + relative_start;
        let before_is_boundary = start == 0 || bytes[start - 1].is_ascii_whitespace();
        let mut value_start = start + name.len();
        while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
            value_start += 1;
        }
        if !before_is_boundary || bytes.get(value_start) != Some(&b'=') {
            cursor = start + name.len();
            continue;
        }
        value_start += 1;
        while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
            value_start += 1;
        }
        let quote = bytes.get(value_start).copied();
        let (value_start, value_end) = if matches!(quote, Some(b'\'' | b'"')) {
            let value_start = value_start + 1;
            let quote = quote?;
            let value_end = bytes[value_start..]
                .iter()
                .position(|byte| *byte == quote)
                .map(|offset| value_start + offset)?;
            (value_start, value_end)
        } else {
            let value_end = bytes[value_start..]
                .iter()
                .position(|byte| byte.is_ascii_whitespace() || *byte == b'>')
                .map(|offset| value_start + offset)
                .unwrap_or(bytes.len());
            (value_start, value_end)
        };
        return Some(tag[value_start..value_end].replace("&amp;", "&"));
    }

    None
}

fn icon_size(value: &str) -> Option<u32> {
    value
        .split_ascii_whitespace()
        .filter_map(|size| size.split_once('x'))
        .filter_map(|(width, height)| Some(width.parse::<u32>().ok()?.min(height.parse().ok()?)))
        .max()
}

fn resolve_icon_url(origin: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("https://") || href.starts_with("http://") {
        Some(href.to_owned())
    } else if href.starts_with("//") {
        Some(format!("{}:{href}", origin.split_once("://")?.0))
    } else if href.starts_with('/') {
        Some(format!("{origin}{href}"))
    } else if !href.is_empty() && !href.starts_with("data:") {
        Some(format!("{origin}/{href}"))
    } else {
        None
    }
}

fn fetch_favicon(url: &str) -> Option<image::DynamicImage> {
    let request = ureq::http::Request::get(url)
        .header("User-Agent", "rayslash/0.1")
        .body(())
        .ok()?;
    let mut response = http_agent().run(request).ok()?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_FAVICON_BYTES)
        .read_to_vec()
        .ok()?;
    image::load_from_memory(&bytes).ok()
}

fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_finds_enabled_settings_rows() {
        let template = WebSearchConfig {
            name: "Docs".into(),
            keyword: "docs".into(),
            url: "https://example.com/?q=%s".into(),
            enabled: true,
        };
        assert!(is_valid_template(&template));
        assert_eq!(
            trigger_from_input(&[template], "DOCS").unwrap().name,
            "Docs"
        );
    }

    #[test]
    fn declared_favicons_are_ranked_largest_first() {
        let html = r#"
            <link rel="shortcut icon" href="/favicon.ico">
            <link sizes="32x32" href="/favicon-32.png" rel="icon">
            <link rel="icon" href="https://cdn.example/icon-96.png" sizes="96x96">
        "#;
        assert_eq!(
            favicon_candidates_from_html("https://example.com", html),
            vec![
                "https://cdn.example/icon-96.png",
                "https://example.com/favicon-32.png",
                "https://example.com/favicon.ico",
            ]
        );
    }

    #[test]
    fn icon_urls_support_protocol_relative_and_relative_paths() {
        assert_eq!(
            resolve_icon_url("https://example.com", "//cdn.example/icon.png").as_deref(),
            Some("https://cdn.example/icon.png")
        );
        assert_eq!(
            resolve_icon_url("https://example.com", "assets/icon.png").as_deref(),
            Some("https://example.com/assets/icon.png")
        );
        assert_eq!(
            resolve_icon_url("https://example.com", "data:image/png,x"),
            None
        );
    }
}
