use std::{
    collections::BTreeMap,
    fmt,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use serde::Deserialize;

const FRANKFURTER_API_BASE: &str = "https://api.frankfurter.dev";
const REQUEST_TIMEOUT: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyConversionRequest {
    pub amount: f64,
    pub base: String,
    pub quote: String,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyConversion {
    pub expression: String,
    pub result: String,
    pub date: Option<String>,
    pub provider: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrencyConversionError {
    Request(String),
    Response(String),
}

#[derive(Debug, Clone)]
struct CachedRate {
    rate: f64,
    date: String,
}

#[derive(Debug, Deserialize)]
struct FrankfurterRateResponse {
    date: String,
    rate: f64,
}

static RATE_CACHE: OnceLock<Mutex<BTreeMap<(String, String), CachedRate>>> = OnceLock::new();

impl fmt::Display for CurrencyConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(message) | Self::Response(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CurrencyConversionError {}

pub fn convert_query(query: &str) -> Option<Result<CurrencyConversion, CurrencyConversionError>> {
    let request = parse_query(query)?;
    Some(convert_request(&request))
}

pub fn parse_query(query: &str) -> Option<CurrencyConversionRequest> {
    let query = query.trim();
    let (left, quote) = split_conversion_query(query)?;
    let (amount, base) = parse_amount_and_currency(left)?;
    let base = normalize_currency_code(base)?;
    let quote = normalize_currency_code(quote)?;

    Some(CurrencyConversionRequest {
        amount,
        base,
        quote,
        expression: query.to_owned(),
    })
}

pub fn convert_request(
    request: &CurrencyConversionRequest,
) -> Result<CurrencyConversion, CurrencyConversionError> {
    let rate = if request.base == request.quote {
        CachedRate {
            rate: 1.0,
            date: String::new(),
        }
    } else {
        rate_for(&request.base, &request.quote)?
    };
    let converted = request.amount * rate.rate;

    Ok(CurrencyConversion {
        expression: request.expression.clone(),
        result: format!("{} {}", format_number(converted), request.quote),
        date: (!rate.date.is_empty()).then_some(rate.date),
        provider: "Frankfurter",
    })
}

fn split_conversion_query(query: &str) -> Option<(&str, &str)> {
    split_once_word(query, "to").or_else(|| split_once_word(query, "in"))
}

fn split_once_word<'a>(query: &'a str, word: &str) -> Option<(&'a str, &'a str)> {
    let lower = query.to_ascii_lowercase();
    let needle = format!(" {word} ");
    let index = lower.find(&needle)?;
    let left = query[..index].trim();
    let right = query[index + needle.len()..].trim();

    (!left.is_empty() && !right.is_empty()).then_some((left, right))
}

fn parse_amount_and_currency(text: &str) -> Option<(f64, &str)> {
    let text = text.trim();
    let mut end = 0;
    let mut seen_digit = false;

    for (index, ch) in text.char_indices() {
        let valid = ch.is_ascii_digit()
            || ch == '.'
            || ((ch == '+' || ch == '-') && index == 0)
            || ((ch == 'e' || ch == 'E') && seen_digit);
        if !valid {
            break;
        }
        if ch.is_ascii_digit() {
            seen_digit = true;
        }
        end = index + ch.len_utf8();
    }

    if !seen_digit {
        return None;
    }

    let amount = text[..end].parse::<f64>().ok()?;
    let currency = text[end..].trim();
    (!currency.is_empty()).then_some((amount, currency))
}

fn normalize_currency_code(code: &str) -> Option<String> {
    let code = code.trim().to_ascii_uppercase();
    (code.len() == 3 && code.chars().all(|ch| ch.is_ascii_alphabetic())).then_some(code)
}

fn rate_for(base: &str, quote: &str) -> Result<CachedRate, CurrencyConversionError> {
    let key = (base.to_owned(), quote.to_owned());
    let cache = RATE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));

    if let Some(rate) = cache
        .lock()
        .map_err(|error| CurrencyConversionError::Response(error.to_string()))?
        .get(&key)
        .cloned()
    {
        return Ok(rate);
    }

    let rate = fetch_rate(base, quote)?;
    cache
        .lock()
        .map_err(|error| CurrencyConversionError::Response(error.to_string()))?
        .insert(key, rate.clone());

    Ok(rate)
}

fn fetch_rate(base: &str, quote: &str) -> Result<CachedRate, CurrencyConversionError> {
    let url = format!("{FRANKFURTER_API_BASE}/v2/rate/{base}/{quote}");
    let agent = ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build();
    let response: FrankfurterRateResponse = agent
        .get(&url)
        .call()
        .map_err(|source| CurrencyConversionError::Request(source.to_string()))?
        .into_json()
        .map_err(|source| CurrencyConversionError::Response(source.to_string()))?;

    Ok(CachedRate {
        rate: response.rate,
        date: response.date,
    })
}

fn format_number(value: f64) -> String {
    if (value - value.round()).abs() < 0.000000001 {
        return format!("{:.0}", value.round());
    }

    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_currency_conversion_queries() {
        let request = parse_query("12.5 usd to brl").expect("currency query");

        assert_eq!(request.amount, 12.5);
        assert_eq!(request.base, "USD");
        assert_eq!(request.quote, "BRL");
        assert!(parse_query("12 usd").is_none());
        assert!(parse_query("12 usd to widgets").is_none());
    }

    #[test]
    fn same_currency_conversion_does_not_require_network() {
        let conversion = convert_query("10 usd to usd")
            .expect("currency query")
            .expect("conversion");

        assert_eq!(conversion.result, "10 USD");
        assert_eq!(conversion.provider, "Frankfurter");
    }
}
