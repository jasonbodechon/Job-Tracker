use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::{error::ApiError, models::{Posting, ResolvedLocation}, AppState};

/// Upper limit for one location lookup, so a slow geocoder cannot stall a search.
const GEOCODING_TIMEOUT_SECONDS: u64 = 15;

#[derive(Debug, Deserialize)]
struct GeocoderResult {
    display_name: String,
    lat: String,
    lon: String,
    #[serde(default)]
    addresstype: String,
    #[serde(default, rename = "type")]
    result_type: String,
}

pub fn normalize_query(value: &str) -> String {
    value.to_lowercase().chars().map(|character| if character.is_alphanumeric() { character } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn is_broad_location(value: &str) -> bool {
    let normalized = normalize_query(value);
    let parts = normalized.split_whitespace().collect::<Vec<_>>();
    if normalized.is_empty() { return false; }
    const BROAD: &[&str] = &["canada","united states","united states of america","usa","ontario","quebec","alberta","british columbia","manitoba","saskatchewan","nova scotia","new brunswick","newfoundland and labrador","prince edward island","northwest territories","nunavut","yukon","california","texas","new york","florida","washington","massachusetts","north america"];
    if BROAD.contains(&normalized.as_str()) { return true; }
    let broad_tokens = ["canada","ontario","quebec","alberta","manitoba","saskatchewan","usa"];
    parts.len() <= 3 && parts.iter().all(|part| broad_tokens.contains(part) || part.len() <= 2)
}

fn built_in(value: &str) -> Option<ResolvedLocation> {
    let normalized = normalize_query(value);
    let city = normalized.split_whitespace().next()?;
    let (label, latitude, longitude) = match city {
        "kitchener" => ("Kitchener, Ontario, Canada", 43.4516, -80.4925),
        "waterloo" => ("Waterloo, Ontario, Canada", 43.4643, -80.5204),
        "cambridge" => ("Cambridge, Ontario, Canada", 43.3616, -80.3144),
        "guelph" => ("Guelph, Ontario, Canada", 43.5448, -80.2482),
        "toronto" => ("Toronto, Ontario, Canada", 43.6532, -79.3832),
        "mississauga" => ("Mississauga, Ontario, Canada", 43.5890, -79.6441),
        "hamilton" => ("Hamilton, Ontario, Canada", 43.2557, -79.8711),
        "burlington" => ("Burlington, Ontario, Canada", 43.3255, -79.7990),
        "oakville" => ("Oakville, Ontario, Canada", 43.4675, -79.6877),
        "ottawa" => ("Ottawa, Ontario, Canada", 45.4215, -75.6972),
        "vancouver" => ("Vancouver, British Columbia, Canada", 49.2827, -123.1207),
        "montreal" => ("Montreal, Quebec, Canada", 45.5019, -73.5674),
        "calgary" => ("Calgary, Alberta, Canada", 51.0447, -114.0719),
        "edmonton" => ("Edmonton, Alberta, Canada", 53.5461, -113.4938),
        "boston" => ("Boston, Massachusetts, United States", 42.3601, -71.0589),
        "austin" => ("Austin, Texas, United States", 30.2672, -97.7431),
        _ => return None,
    };
    Some(ResolvedLocation { label: label.into(), latitude, longitude, confidence: "city".into() })
}

pub fn candidate_queries(posting: &Posting) -> Vec<String> {
    let splitter = Regex::new(r"(?i)\s*(?:;|\||/|·|\bor\b)\s*").unwrap();
    let description_locations = Regex::new(r"\b([A-Z][A-Za-z.' -]{2,40},\s*(?:[A-Z]{2}|Ontario|Quebec|Alberta|British Columbia|Canada))\b").unwrap();
    let mut values = splitter.split(&posting.location).map(str::trim).filter(|value| !value.is_empty()).filter(|value| !matches!(normalize_query(value).as_str(), "remote"|"hybrid"|"on site"|"onsite")).map(str::to_string).collect::<Vec<_>>();
    values.extend(description_locations.captures_iter(&posting.description).filter_map(|capture| capture.get(1).map(|value| value.as_str().trim().to_string())));
    values.sort_by_key(|value| normalize_query(value));
    values.dedup_by(|left, right| normalize_query(left) == normalize_query(right));
    values.truncate(6);
    values
}

pub async fn resolve_query(state: &AppState, query: &str) -> Result<Vec<ResolvedLocation>, ApiError> {
    if query.trim().is_empty() || is_broad_location(query) { return Ok(Vec::new()); }
    if let Some(value) = built_in(query) { return Ok(vec![value]); }
    let normalized = normalize_query(query);
    if let Some(cached) = sqlx::query_scalar::<_, serde_json::Value>("SELECT results FROM geocoding_cache WHERE normalized_query=$1").bind(&normalized).fetch_optional(&state.db).await? {
        return Ok(serde_json::from_value(cached).unwrap_or_default());
    }
    if state.config.geocoding_delay_ms > 0 { tokio::time::sleep(Duration::from_millis(state.config.geocoding_delay_ms)).await; }
    let response = state.client.get(format!("{}/search", state.config.geocoding_url)).query(&[("q", query), ("format", "jsonv2"), ("limit", "3"), ("addressdetails", "1")]).timeout(Duration::from_secs(GEOCODING_TIMEOUT_SECONDS)).send().await;
    let results = match response {
        // Failed lookups are not cached, so a timeout or rate limit is retried on the next search
        // instead of permanently treating the place as unknown.
        Ok(response) if response.status().is_success() => {
            let values = match response.json::<Vec<GeocoderResult>>().await {
                Ok(values) => values,
                Err(error) => { tracing::warn!(%error, query, "geocoding response was unreadable; not caching"); return Ok(Vec::new()); }
            };
            values.into_iter().filter(|value| !matches!(value.addresstype.as_str(), "country"|"state"|"region") && !matches!(value.result_type.as_str(), "country"|"state"|"region")).filter_map(|value| Some(ResolvedLocation { label:value.display_name,latitude:value.lat.parse().ok()?,longitude:value.lon.parse().ok()?,confidence:if matches!(value.addresstype.as_str(),"house"|"building"|"road"){"address".into()}else{"city".into()} })).collect::<Vec<_>>()
        }
        Ok(response) => { tracing::warn!(status = %response.status(), query, "geocoding lookup failed; not caching"); return Ok(Vec::new()); }
        Err(error) => { tracing::warn!(%error, query, "geocoding lookup failed; not caching"); return Ok(Vec::new()); }
    };
    sqlx::query("INSERT INTO geocoding_cache(normalized_query,query,results,resolved_at) VALUES($1,$2,$3,now()) ON CONFLICT(normalized_query) DO UPDATE SET query=excluded.query,results=excluded.results,resolved_at=excluded.resolved_at").bind(normalized).bind(query).bind(json!(results)).execute(&state.db).await?;
    Ok(results)
}

pub async fn resolve_posting(state: &AppState, posting: &Posting) -> Result<Vec<ResolvedLocation>, ApiError> {
    if posting.remote_type == "Remote" { return Ok(Vec::new()); }
    let mut results = Vec::new();
    for query in candidate_queries(posting) {
        results.extend(resolve_query(state, &query).await?);
    }
    results.sort_by(|left, right| left.label.cmp(&right.label));
    results.dedup_by(|left, right| (left.latitude-right.latitude).abs()<0.0001 && (left.longitude-right.longitude).abs()<0.0001);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_broad_locations_without_using_a_centroid() {
        assert!(is_broad_location("Ontario, Canada"));
        assert!(is_broad_location("Canada"));
        assert!(!is_broad_location("Toronto, Ontario"));
    }

    #[test]
    fn extracts_multiple_office_candidates() {
        let posting=Posting{external_id:"1".into(),title:"Engineer".into(),company:"A".into(),location:"Toronto, ON or Waterloo, ON · Hybrid".into(),remote_type:"Hybrid".into(),posted_at:"".into(),apply_url:"".into(),description:"".into()};
        assert_eq!(candidate_queries(&posting),vec!["Toronto, ON","Waterloo, ON"]);
    }
}
