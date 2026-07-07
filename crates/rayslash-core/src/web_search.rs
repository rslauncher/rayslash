use crate::config::WebSearchConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearch {
    pub name: String,
    pub query: String,
    pub url: String,
}

pub fn matching_web_searches(templates: &[WebSearchConfig], input: &str) -> Vec<WebSearch> {
    templates
        .iter()
        .filter_map(|template| web_search_for_template(template, input))
        .collect()
}

pub fn web_search_for_template(template: &WebSearchConfig, input: &str) -> Option<WebSearch> {
    let search_terms = search_terms_for_trigger(input, &template.query)?;
    let encoded_query = url_encode(search_terms);
    let url = template.url_template.replace("{query}", &encoded_query);

    Some(WebSearch {
        name: template.name.clone(),
        query: search_terms.to_owned(),
        url,
    })
}

fn search_terms_for_trigger<'a>(input: &'a str, trigger: &str) -> Option<&'a str> {
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
            query: "ddg".to_owned(),
            url_template: "https://duckduckgo.com/?q={query}".to_owned(),
        };

        assert_eq!(web_search_for_template(&template, "ddg").map(|_| ()), None);
        assert_eq!(
            web_search_for_template(&template, "ddgrust").map(|_| ()),
            None
        );

        let result = web_search_for_template(&template, "DDG rust slint").expect("web search");

        assert_eq!(result.name, "DuckDuckGo");
        assert_eq!(result.query, "rust slint");
        assert_eq!(result.url, "https://duckduckgo.com/?q=rust%20slint");
    }

    #[test]
    fn url_encoding_uses_utf8_percent_encoding() {
        assert_eq!(url_encode("a+b & café"), "a%2Bb%20%26%20caf%C3%A9");
    }
}
