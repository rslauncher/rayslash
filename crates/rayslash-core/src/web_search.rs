use std::path::PathBuf;

use crate::config::WebSearchConfig;

pub const DEFAULT_SEARCH_KEYWORD: &str = "search";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearch {
    pub name: String,
    pub keyword: String,
    pub query: String,
    pub url: String,
    pub icon_label: String,
    pub icon_path: Option<PathBuf>,
    pub host: String,
}

pub fn matching_web_searches(templates: &[WebSearchConfig], input: &str) -> Vec<WebSearch> {
    templates
        .iter()
        .filter(|template| template.enabled && is_valid_template(template))
        .filter_map(|template| web_search_for_template(template, input))
        .collect()
}

pub fn web_search_for_template(template: &WebSearchConfig, input: &str) -> Option<WebSearch> {
    let search_terms = search_terms_for_trigger(input, &template.keyword)?;
    let encoded_query = url_encode(search_terms);
    let url = template.url.replace("%s", &encoded_query);

    Some(WebSearch {
        name: template.name.clone(),
        keyword: template.keyword.clone(),
        query: search_terms.to_owned(),
        url,
        icon_label: icon_label_for_template(template),
        icon_path: cached_favicon_path(template),
        host: host_from_url(&template.url).unwrap_or_default(),
    })
}

pub fn is_valid_template(template: &WebSearchConfig) -> bool {
    !template.name.trim().is_empty()
        && !template.keyword.trim().is_empty()
        && template.url.contains("%s")
        && host_from_url(&template.url).is_some()
}

pub fn search_terms_for_trigger<'a>(input: &'a str, trigger: &str) -> Option<&'a str> {
    let input = input.trim();
    let trigger = trigger.trim();

    if input.len() < trigger.len() || trigger.is_empty() {
        return None;
    }

    let (head, rest) = input.split_at(trigger.len());
    if !head.eq_ignore_ascii_case(trigger) {
        return None;
    }

    if !rest.is_empty() && !rest.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }

    let search_terms = rest.trim();
    (!search_terms.is_empty()).then_some(search_terms)
}

pub fn default_search_terms(input: &str) -> Option<&str> {
    search_terms_for_trigger(input, DEFAULT_SEARCH_KEYWORD)
}

pub fn is_default_search_trigger(input: &str) -> bool {
    input.trim().eq_ignore_ascii_case(DEFAULT_SEARCH_KEYWORD)
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

pub fn icon_label_for_template(template: &WebSearchConfig) -> String {
    icon_label_from_name(&template.keyword)
        .or_else(|| icon_label_from_name(&template.name))
        .or_else(|| icon_label_from_host(&template.url))
        .unwrap_or_else(|| "W".to_owned())
}

fn icon_label_from_name(name: &str) -> Option<String> {
    let words = name
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();

    if words.len() > 1 {
        let label = words
            .iter()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect::<String>()
            .to_ascii_uppercase();
        return (!label.is_empty()).then_some(label);
    }

    let label = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase();
    (!label.is_empty()).then_some(label)
}

fn icon_label_from_host(url: &str) -> Option<String> {
    let host = host_from_url(url)?;
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let significant = host.split('.').find(|part| !part.is_empty())?;
    icon_label_from_name(significant)
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
    if !is_valid_template(template) {
        return None;
    }
    if template
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
    let url = format!("{scheme}://{host}/favicon.ico");
    let mut response = ureq::get(&url)
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

fn url_encode(text: &str) -> String {
    let mut encoded = String::new();

    for byte in text.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            byte => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_search_requires_trigger_and_terms() {
        let template = WebSearchConfig {
            name: "DuckDuckGo".to_owned(),
            keyword: "ddg".to_owned(),
            url: "https://duckduckgo.com/?q=%s".to_owned(),
            enabled: true,
        };

        assert_eq!(web_search_for_template(&template, "ddg").map(|_| ()), None);
        assert_eq!(
            web_search_for_template(&template, "ddgrust").map(|_| ()),
            None
        );

        let result = web_search_for_template(&template, "DDG rust slint").expect("web search");

        assert_eq!(result.name, "DuckDuckGo");
        assert_eq!(result.keyword, "ddg");
        assert_eq!(result.query, "rust slint");
        assert_eq!(result.url, "https://duckduckgo.com/?q=rust%20slint");
        assert_eq!(result.host, "duckduckgo.com");
        assert_eq!(result.icon_label, "DD");
    }

    #[test]
    fn url_encoding_uses_utf8_percent_encoding() {
        assert_eq!(url_encode("a+b & café"), "a%2Bb%20%26%20caf%C3%A9");
    }

    #[test]
    fn disabled_templates_do_not_match() {
        let template = WebSearchConfig {
            name: "YouTube".to_owned(),
            keyword: "yt".to_owned(),
            url: "https://www.youtube.com/results?search_query=%s".to_owned(),
            enabled: false,
        };

        assert!(matching_web_searches(&[template], "yt rust").is_empty());
    }

    #[test]
    fn incomplete_templates_are_drafts_and_never_match() {
        let template = WebSearchConfig {
            name: "YouTube".to_owned(),
            keyword: String::new(),
            url: "https://www.youtube.com/results?search_query=%s".to_owned(),
            enabled: true,
        };

        assert!(!is_valid_template(&template));
        assert!(matching_web_searches(&[template], "yt rust").is_empty());
    }

    #[test]
    fn trigger_detection_matches_enabled_keyword_only() {
        let templates = vec![
            WebSearchConfig {
                name: "YouTube".to_owned(),
                keyword: "yt".to_owned(),
                url: "https://www.youtube.com/results?search_query=%s".to_owned(),
                enabled: true,
            },
            WebSearchConfig {
                name: "Disabled".to_owned(),
                keyword: "off".to_owned(),
                url: "https://example.com/?q=%s".to_owned(),
                enabled: false,
            },
        ];

        assert_eq!(
            trigger_from_input(&templates, "YT").map(|template| template.name.as_str()),
            Some("YouTube")
        );
        assert!(trigger_from_input(&templates, "off").is_none());
        assert!(trigger_from_input(&templates, "yt rust").is_none());
    }

    #[test]
    fn default_search_requires_builtin_trigger_and_terms() {
        assert_eq!(default_search_terms("search manhattan"), Some("manhattan"));
        assert_eq!(
            default_search_terms("SEARCH rust slint"),
            Some("rust slint")
        );
        assert_eq!(default_search_terms("manhattan"), None);
        assert_eq!(default_search_terms("search"), None);
        assert!(is_default_search_trigger("search"));
    }
}
