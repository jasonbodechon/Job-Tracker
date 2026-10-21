use std::{env, net::SocketAddr};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind: SocketAddr,
    pub app_origin: String,
    pub cookie_secure: bool,
    pub session_days: i64,
    pub encryption_key: String,
    pub vapid_public_key: Option<String>,
    pub vapid_private_key: Option<String>,
    pub vapid_subject: String,
    pub static_dir: String,
    pub worker_poll_seconds: u64,
    pub geocoding_url: String,
    pub geocoding_delay_ms: u64,
    pub adzuna_app_id: Option<String>,
    pub adzuna_app_key: Option<String>,
    pub connector_max_results: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let get = |key: &str| env::var(key).map_err(|_| format!("{key} is required"));
        Ok(Self {
            database_url: get("DATABASE_URL")?,
            bind: env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".into()).parse().map_err(|e| format!("Invalid BIND_ADDRESS: {e}"))?,
            app_origin: env::var("APP_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into()),
            cookie_secure: env::var("COOKIE_SECURE").map(|v| v != "false").unwrap_or(true),
            session_days: env::var("SESSION_TTL_DAYS").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            encryption_key: get("ENCRYPTION_KEY")?,
            vapid_public_key: env::var("VAPID_PUBLIC_KEY").ok().filter(|v| !v.is_empty()),
            vapid_private_key: env::var("VAPID_PRIVATE_KEY").ok().filter(|v| !v.is_empty()),
            vapid_subject: env::var("VAPID_SUBJECT").unwrap_or_else(|_| "mailto:admin@example.com".into()),
            static_dir: env::var("STATIC_DIR").unwrap_or_else(|_| "dist".into()),
            worker_poll_seconds: env::var("WORKER_POLL_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            geocoding_url: env::var("GEOCODING_URL").unwrap_or_else(|_| "https://nominatim.openstreetmap.org".into()).trim_end_matches('/').to_string(),
            geocoding_delay_ms: env::var("GEOCODING_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000),
            adzuna_app_id: env::var("ADZUNA_APP_ID").ok().filter(|value| !value.is_empty()),
            adzuna_app_key: env::var("ADZUNA_APP_KEY").ok().filter(|value| !value.is_empty()),
            connector_max_results: env::var("CONNECTOR_MAX_RESULTS").ok().and_then(|value| value.parse().ok()).unwrap_or(5000).clamp(100, 20000),
        })
    }
}
