use std::path::PathBuf;

use crate::config::WebSearchConfig;

pub const DEFAULT_SEARCH_KEYWORD: &str = "search";

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
    if let Some(path) = cached_favicon_path(template) {
        return Some(path);
    }
    let host = host_from_url(&template.url)?;
    let scheme = if template.url.trim().starts_with("http://") {
        "http"
    } else {
        "https"
    };
    let mut response = ureq::get(&format!("{scheme}://{host}/favicon.ico"))
        .header("User-Agent", "rayslash/0.1")
        .call()
        .ok()?;
    let bytes = response.body_mut().read_to_vec().ok()?;
    let favicon = image::load_from_memory(&bytes).ok()?;
    let cache_dir = dirs::cache_dir()?.join("rayslash/search-favicons");
    std::fs::create_dir_all(&cache_dir).ok()?;
    let filename = host
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let path = cache_dir.join(format!("{filename}.png"));
    favicon
        .save_with_format(&path, image::ImageFormat::Png)
        .ok()?;
    Some(path)
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
}
