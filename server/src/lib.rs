pub mod api;
pub mod admin;
pub mod auth;
pub mod config;
pub mod connectors;
pub mod crypto;
pub mod directory;
pub mod error;
pub mod importer;
pub mod geocoding;
pub mod matching;
pub mod models;
pub mod notifications;
pub mod service;

use sqlx::PgPool;
use std::{net::IpAddr, time::Duration};

/// Limits for every outbound request, so one unresponsive site cannot hang a search forever.
pub const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: config::Config,
    pub client: reqwest::Client,
}

impl AppState {
    pub async fn connect(config: config::Config) -> anyhow::Result<Self> {
        let db = PgPool::connect(&config.database_url).await?;
        repair_company_directory_migration_checksum(&db).await?;
        MIGRATOR.run(&db).await?;
        let redirect = reqwest::redirect::Policy::custom(|attempt| {
            let host = attempt.url().host_str().unwrap_or_default().to_ascii_lowercase();
            if host == "localhost" || host.ends_with(".local") || host.parse::<IpAddr>().is_ok() { attempt.stop() } else { attempt.follow() }
        });
        let client = reqwest::Client::builder().redirect(redirect).connect_timeout(HTTP_CONNECT_TIMEOUT).timeout(HTTP_REQUEST_TIMEOUT).user_agent(format!("JobTracker/{}", env!("CARGO_PKG_VERSION"))).build().expect("HTTP client");
        Ok(Self { db, config, client })
    }
}

/// Version 0.14.0 accidentally republished migration 6 with a changed seed URL.
/// Existing databases correctly rejected the changed checksum before migration 7
/// could apply the URL update. Repair only this known migration after confirming
/// that SQLx recorded it successfully and its complete schema is present.
async fn repair_company_directory_migration_checksum(db: &PgPool) -> anyhow::Result<()> {
    let migrations_table_exists: bool = sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
        .fetch_one(db)
        .await?;
    if !migrations_table_exists {
        return Ok(());
    }

    let applied: Option<(Vec<u8>, bool, String)> = sqlx::query_as(
        "SELECT checksum, success, description FROM _sqlx_migrations WHERE version = 6",
    )
    .fetch_optional(db)
    .await?;
    let Some((stored_checksum, success, description)) = applied else {
        return Ok(());
    };
    let migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == 6)
        .ok_or_else(|| anyhow::anyhow!("embedded company-directory migration is missing"))?;
    let expected_checksum: &[u8] = migration.checksum.as_ref();
    if stored_checksum.as_slice() == expected_checksum {
        return Ok(());
    }

    let schema_is_complete: bool = sqlx::query_scalar(
        "SELECT to_regclass('public.company_directory') IS NOT NULL AND NOT EXISTS (
           SELECT required.column_name
           FROM (VALUES ('id'),('company_name'),('countries'),('ats_provider'),('connector_class'),
                        ('board_identifier'),('careers_url'),('current_job_count'),('last_verified_at'),
                        ('next_verification_at'),('verification_status'),('verification_error'),
                        ('identifier_origin'),('detected_provider'),('redirect_url'),('updated_at')) AS required(column_name)
           WHERE NOT EXISTS (
             SELECT 1 FROM information_schema.columns AS existing
             WHERE table_schema = 'public' AND table_name = 'company_directory'
               AND existing.column_name = required.column_name
           )
         )",
    )
    .fetch_one(db)
    .await?;
    if !safe_to_repair_company_directory_migration(success, &description, migration.description.as_ref(), schema_is_complete) {
        anyhow::bail!("migration 6 checksum differs, but its recorded identity or schema is not safe to repair");
    }

    sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = 6")
        .bind(expected_checksum)
        .execute(db)
        .await?;
    tracing::warn!("repaired the known migration 6 checksum mismatch from Job-Tracker v0.14.0");
    Ok(())
}

fn safe_to_repair_company_directory_migration(success: bool, stored_description: &str, expected_description: &str, schema_is_complete: bool) -> bool {
    success && schema_is_complete && stored_description == expected_description
}

#[cfg(test)]
mod migration_recovery_tests {
    use super::safe_to_repair_company_directory_migration;

    #[test]
    fn accepts_only_the_successful_matching_complete_migration() {
        assert!(safe_to_repair_company_directory_migration(true, "company directory", "company directory", true));
        assert!(!safe_to_repair_company_directory_migration(false, "company directory", "company directory", true));
        assert!(!safe_to_repair_company_directory_migration(true, "other migration", "company directory", true));
        assert!(!safe_to_repair_company_directory_migration(true, "company directory", "company directory", false));
    }
}
