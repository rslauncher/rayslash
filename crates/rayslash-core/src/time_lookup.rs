use std::{
    collections::BTreeMap,
    fmt,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;

const GEOCODING_API_BASE: &str = "https://geocoding-api.open-meteo.com";
const FORECAST_API_BASE: &str = "https://api.open-meteo.com";
const REQUEST_TIMEOUT: Duration = Duration::from_millis(1200);
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeLookupRequest {
    pub location: String,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeLookup {
    pub expression: String,
    pub location: String,
    pub result: String,
    pub date: String,
    pub timezone: String,
    pub offset: String,
    pub provider: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeLookupError {
    Request(String),
    Response(String),
}

#[derive(Debug, Clone)]
struct CachedPlace {
    place: PlaceTimeZone,
    fetched_at: Instant,
}

#[derive(Debug, Clone)]
struct PlaceTimeZone {
    display_location: String,
    timezone: String,
    offset_seconds: i32,
}

#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
    timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForecastTimeZoneResponse {
    timezone: Option<String>,
    utc_offset_seconds: i32,
}

static PLACE_CACHE: OnceLock<Mutex<BTreeMap<String, CachedPlace>>> = OnceLock::new();

impl fmt::Display for TimeLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(message) | Self::Response(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for TimeLookupError {}

pub fn lookup_query(query: &str) -> Option<Result<TimeLookup, TimeLookupError>> {
    let request = parse_query(query)?;
    Some(lookup_request(&request))
}

pub fn parse_query(query: &str) -> Option<TimeLookupRequest> {
    let query = query.trim();
    let location = strip_time_prefix(query)?.trim();

    if location.chars().count() < 2 {
        return None;
    }

    Some(TimeLookupRequest {
        location: location.to_owned(),
        expression: format!("time in {}", normalize_location_for_display(location)),
    })
}

pub fn lookup_request(request: &TimeLookupRequest) -> Result<TimeLookup, TimeLookupError> {
    let place = place_timezone_for(&request.location)?;
    let now = Utc::now() + ChronoDuration::seconds(i64::from(place.offset_seconds));
    let offset = format_offset(place.offset_seconds);

    Ok(TimeLookup {
        expression: request.expression.clone(),
        location: place.display_location,
        result: now.format("%H:%M").to_string(),
        date: now.format("%Y-%m-%d").to_string(),
        timezone: place.timezone,
        offset,
        provider: "Open-Meteo",
    })
}

fn strip_time_prefix(query: &str) -> Option<&str> {
    let lower = query.to_ascii_lowercase();
    let prefix = "time in";

    if !lower.starts_with(prefix) {
        return None;
    }

    let rest = &query[prefix.len()..];
    rest.chars()
        .next()
        .is_some_and(char::is_whitespace)
        .then_some(rest)
}

fn place_timezone_for(location: &str) -> Result<PlaceTimeZone, TimeLookupError> {
    let key = normalize_cache_key(location);
    let cache = PLACE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));

    if let Some(cached) = cache
        .lock()
        .map_err(|error| TimeLookupError::Response(error.to_string()))?
        .get(&key)
        .filter(|cached| cached.fetched_at.elapsed() < CACHE_TTL)
        .cloned()
    {
        return Ok(cached.place);
    }

    let place = fetch_place_timezone(location)?;
    cache
        .lock()
        .map_err(|error| TimeLookupError::Response(error.to_string()))?
        .insert(
            key,
            CachedPlace {
                place: place.clone(),
                fetched_at: Instant::now(),
            },
        );

    Ok(place)
}

fn fetch_place_timezone(location: &str) -> Result<PlaceTimeZone, TimeLookupError> {
    let location = fetch_location(location)?;
    let timezone = fetch_timezone(location.latitude, location.longitude)?;
    let display_location = display_location(&location);

    Ok(PlaceTimeZone {
        display_location,
        timezone: timezone
            .timezone
            .or(location.timezone)
            .unwrap_or_else(|| "Local time".to_owned()),
        offset_seconds: timezone.utc_offset_seconds,
    })
}

fn fetch_location(location: &str) -> Result<GeocodingResult, TimeLookupError> {
    let url = format!(
        "{GEOCODING_API_BASE}/v1/search?name={}&count=5&language=en&format=json",
        url_encode(location)
    );
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|source| TimeLookupError::Request(source.to_string()))?;
    let response: GeocodingResponse = response
        .body_mut()
        .read_json()
        .map_err(|source| TimeLookupError::Response(source.to_string()))?;

    response
        .results
        .and_then(|mut results| results.drain(..).next())
        .ok_or_else(|| TimeLookupError::Response(format!("No place found for {location}.")))
}

fn fetch_timezone(
    latitude: f64,
    longitude: f64,
) -> Result<ForecastTimeZoneResponse, TimeLookupError> {
    let url = format!(
        "{FORECAST_API_BASE}/v1/forecast?latitude={latitude}&longitude={longitude}&timezone=auto&forecast_days=1"
    );
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into();

    let mut response = agent
        .get(&url)
        .call()
        .map_err(|source| TimeLookupError::Request(source.to_string()))?;

    response
        .body_mut()
        .read_json()
        .map_err(|source| TimeLookupError::Response(source.to_string()))
}

fn display_location(location: &GeocodingResult) -> String {
    let country = location.country.as_deref();
    if country.is_some_and(|country| country.eq_ignore_ascii_case(&location.name)) {
        return location.name.clone();
    }

    let mut parts = vec![location.name.as_str()];
    if let Some(admin1) = location.admin1.as_deref()
        && admin1 != location.name
    {
        parts.push(admin1);
    }
    if let Some(country) = country
        && !parts.iter().any(|part| part.eq_ignore_ascii_case(country))
    {
        parts.push(country);
    }

    parts.join(", ")
}

fn normalize_location_for_display(location: &str) -> String {
    location.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_cache_key(location: &str) -> String {
    normalize_location_for_display(location).to_ascii_lowercase()
}

fn format_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let seconds = seconds.abs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;

    format!("UTC{sign}{hours:02}:{minutes:02}")
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
    fn parses_explicit_time_lookup_queries() {
        let request = parse_query("time in argentina").expect("time query");

        assert_eq!(request.location, "argentina");
        assert_eq!(request.expression, "time in argentina");
        assert!(parse_query("time argentina").is_none());
        assert!(parse_query("time in a").is_none());
    }

    #[test]
    fn formats_utc_offsets() {
        assert_eq!(format_offset(-10_800), "UTC-03:00");
        assert_eq!(format_offset(19_800), "UTC+05:30");
    }
}
