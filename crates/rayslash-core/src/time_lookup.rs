use std::{
    collections::BTreeMap,
    fmt, fs,
    str::FromStr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::{Duration as ChronoDuration, Offset, Utc};
use chrono_tz::Tz;
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
    places: Vec<PlaceTimeZone>,
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
    feature_code: Option<String>,
    country_code: Option<String>,
    population: Option<u64>,
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

pub fn lookup_query(query: &str) -> Option<Result<Vec<TimeLookup>, TimeLookupError>> {
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

pub fn lookup_request(request: &TimeLookupRequest) -> Result<Vec<TimeLookup>, TimeLookupError> {
    place_timezones_for(&request.location).map(|places| {
        places
            .into_iter()
            .map(|place| {
                let now = Utc::now() + ChronoDuration::seconds(i64::from(place.offset_seconds));
                TimeLookup {
                    expression: request.expression.clone(),
                    location: place.display_location,
                    result: now.format("%H:%M").to_string(),
                    date: now.format("%Y-%m-%d").to_string(),
                    timezone: place.timezone,
                    offset: format_offset(place.offset_seconds),
                    provider: "Open-Meteo / IANA tzdb",
                }
            })
            .collect()
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

fn place_timezones_for(location: &str) -> Result<Vec<PlaceTimeZone>, TimeLookupError> {
    let key = normalize_cache_key(location);
    let cache = PLACE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));

    if let Some(cached) = cache
        .lock()
        .map_err(|error| TimeLookupError::Response(error.to_string()))?
        .get(&key)
        .filter(|cached| cached.fetched_at.elapsed() < CACHE_TTL)
        .cloned()
    {
        return Ok(cached.places);
    }

    let places = fetch_place_timezones(location)?;
    cache
        .lock()
        .map_err(|error| TimeLookupError::Response(error.to_string()))?
        .insert(
            key,
            CachedPlace {
                places: places.clone(),
                fetched_at: Instant::now(),
            },
        );

    Ok(places)
}

fn fetch_place_timezones(location: &str) -> Result<Vec<PlaceTimeZone>, TimeLookupError> {
    if let Some((country_code, country_name)) = known_country(location)
        && let Some(places) = country_timezones(country_code, country_name)
    {
        return Ok(places);
    }
    let requested = canonical_location_query(location);
    let location = fetch_location(&requested)?;
    if location.feature_code.as_deref() == Some("PCLI")
        && let Some(country_code) = location.country_code.as_deref()
        && let Some(country_name) = location.country.as_deref().or(Some(location.name.as_str()))
        && let Some(places) = country_timezones(country_code, country_name)
    {
        return Ok(places);
    }
    let timezone = fetch_timezone(location.latitude, location.longitude)?;
    let display_location = display_location(&location);

    Ok(vec![PlaceTimeZone {
        display_location,
        timezone: timezone
            .timezone
            .or(location.timezone)
            .unwrap_or_else(|| "Local time".to_owned()),
        offset_seconds: timezone.utc_offset_seconds,
    }])
}

fn known_country(location: &str) -> Option<(&'static str, &'static str)> {
    match comparable_place_name(location).as_str() {
        "america" | "usa" | "us" | "unitedstates" | "unitedstatesofamerica" => {
            Some(("US", "United States"))
        }
        "argentina" => Some(("AR", "Argentina")),
        "australia" => Some(("AU", "Australia")),
        "brazil" | "brasil" => Some(("BR", "Brazil")),
        "canada" => Some(("CA", "Canada")),
        "chile" => Some(("CL", "Chile")),
        "china" => Some(("CN", "China")),
        "france" => Some(("FR", "France")),
        "germany" => Some(("DE", "Germany")),
        "india" => Some(("IN", "India")),
        "indonesia" => Some(("ID", "Indonesia")),
        "italy" => Some(("IT", "Italy")),
        "japan" => Some(("JP", "Japan")),
        "mexico" => Some(("MX", "Mexico")),
        "newzealand" => Some(("NZ", "New Zealand")),
        "portugal" => Some(("PT", "Portugal")),
        "russia" => Some(("RU", "Russia")),
        "southafrica" => Some(("ZA", "South Africa")),
        "southkorea" => Some(("KR", "South Korea")),
        "spain" => Some(("ES", "Spain")),
        "turkey" | "turkiye" => Some(("TR", "Türkiye")),
        "uk" | "greatbritain" | "unitedkingdom" => Some(("GB", "United Kingdom")),
        "ukraine" => Some(("UA", "Ukraine")),
        _ => None,
    }
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
        .and_then(|results| best_location_result(results, location, None))
        .ok_or_else(|| TimeLookupError::Response(format!("No place found for {location}.")))
}

fn best_location_result(
    mut results: Vec<GeocodingResult>,
    requested: &str,
    country_code: Option<&str>,
) -> Option<GeocodingResult> {
    let requested = comparable_place_name(requested);
    results.sort_by_key(|result| {
        let country_mismatch = country_code.is_some_and(|code| {
            !result
                .country_code
                .as_deref()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(code))
        });
        let name_mismatch = comparable_place_name(&result.name) != requested;
        let not_capital = result.feature_code.as_deref() != Some("PPLC");
        (
            country_mismatch,
            name_mismatch,
            not_capital,
            std::cmp::Reverse(result.population.unwrap_or(0)),
        )
    });
    results.into_iter().next()
}

fn comparable_place_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn canonical_location_query(location: &str) -> String {
    match comparable_place_name(location).as_str() {
        "america" | "usa" | "us" | "unitedstatesofamerica" => "United States".to_owned(),
        "washingtondc" | "washingtondistrict ofcolumbia" => "Washington D.C.".to_owned(),
        "uk" | "greatbritain" => "United Kingdom".to_owned(),
        "uae" => "United Arab Emirates".to_owned(),
        _ => normalize_location_for_display(location),
    }
}

fn country_timezones(country_code: &str, country_name: &str) -> Option<Vec<PlaceTimeZone>> {
    let contents = [
        "/usr/share/zoneinfo/zone.tab",
        "/usr/share/zoneinfo/zone1970.tab",
    ]
    .into_iter()
    .find_map(|path| fs::read_to_string(path).ok())?;
    let now = Utc::now();
    let mut by_offset: BTreeMap<i32, (String, Vec<String>)> = BTreeMap::new();

    for line in contents.lines().filter(|line| !line.starts_with('#')) {
        let mut fields = line.split('\t');
        let Some(countries) = fields.next() else {
            continue;
        };
        let _coordinates = fields.next();
        let Some(zone_name) = fields.next() else {
            continue;
        };
        let comment = fields.next().unwrap_or_default().trim();
        if !countries
            .split(',')
            .any(|code| code.eq_ignore_ascii_case(country_code))
        {
            continue;
        }
        let Ok(timezone) = Tz::from_str(zone_name) else {
            continue;
        };
        let offset = now
            .with_timezone(&timezone)
            .offset()
            .fix()
            .local_minus_utc();
        let description = if comment.is_empty() {
            zone_name.replace('_', " ")
        } else {
            comment.to_owned()
        };
        let entry = by_offset
            .entry(offset)
            .or_insert_with(|| (zone_name.to_owned(), Vec::new()));
        if !entry.1.contains(&description) {
            entry.1.push(description);
        }
    }

    (!by_offset.is_empty()).then(|| {
        by_offset
            .into_iter()
            .map(|(offset_seconds, (timezone, descriptions))| PlaceTimeZone {
                display_location: format!("{country_name} — {}", descriptions.join(", ")),
                timezone,
                offset_seconds,
            })
            .collect()
    })
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
    fn canonicalizes_country_and_punctuation_free_capital_queries() {
        assert_eq!(canonical_location_query("america"), "United States");
        assert_eq!(canonical_location_query("washington dc"), "Washington D.C.");
        assert_eq!(comparable_place_name("Washington D.C."), "washingtondc");
    }

    #[test]
    fn country_timezones_group_regions_by_current_offset() {
        let brazil = country_timezones("BR", "Brazil").expect("Brazil zones");
        assert!(brazil.len() >= 4);
        assert!(
            brazil
                .iter()
                .all(|zone| zone.display_location.starts_with("Brazil — "))
        );
    }

    #[test]
    fn america_query_resolves_locally_to_united_states_timezones() {
        let request = parse_query("time in america").expect("request");
        let results = lookup_request(&request).expect("lookup");
        assert!(results.len() > 1);
        assert!(
            results
                .iter()
                .all(|result| result.location.starts_with("United States — "))
        );
    }

    #[test]
    fn formats_utc_offsets() {
        assert_eq!(format_offset(-10_800), "UTC-03:00");
        assert_eq!(format_offset(19_800), "UTC+05:30");
    }
}
