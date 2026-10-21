use rusqlite::{backup, params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, sync::Mutex, time::Duration};
use tauri::{Manager, State};

struct Database(Mutex<Connection>, PathBuf);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobRecord {
    id: String, source: String, external_id: String, title: String, company: String,
    location: String, remote_type: String, posted_at: String, apply_url: String,
    description: String, skills: Vec<String>, match_score: i64, match_reason: String,
    profile_matches: Vec<serde_json::Value>, is_new: bool, status: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileRecord {
    id: String, name: String, titles: Vec<String>, required_skills: Vec<String>,
    preferred_skills: Vec<String>, resume_text: String, remote_only: bool,
    #[serde(default)] countries: Vec<String>,
    #[serde(default)] regions: Vec<String>,
    #[serde(default)] cities: Vec<String>,
    min_score: i64, enabled: bool, schedule: String, weights: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceRecord {
    id: String, kind: String, company: String, board_token: String, enabled: bool,
    location: Option<String>, last_refreshed_at: Option<String>, last_error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportedJob {
    id: String, source: String, external_id: String, title: String, company: String,
    location: String, remote_type: String, posted_at: String, apply_url: String,
    description: String, skills: Vec<String>, match_score: i64, match_reason: String,
    #[serde(default)] profile_matches: Vec<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResult { imported: usize, inactive: usize }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchRunRecord { id: String, started_at: String, finished_at: Option<String>, trigger_type: String, sources_checked: i64, jobs_imported: i64, errors: Vec<String> }

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackerRecord { job_id: String, status: String, notes: String, contact_name: String, contact_email: String, application_date: Option<String>, interview_date: Option<String>, follow_up_at: Option<String> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsRecord { database_path: String, database_size_bytes: u64, active_jobs: i64, inactive_jobs: i64, profiles: i64, enabled_sources: i64, search_runs: i64, source_errors: i64, due_follow_ups: i64 }

fn open_database(path: PathBuf) -> Result<Connection, String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|error| error.to_string())?; }
    let database = Connection::open(path).map_err(|error| error.to_string())?;
    database.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, source TEXT NOT NULL, source_id TEXT, external_id TEXT NOT NULL, title TEXT NOT NULL, company TEXT NOT NULL, location TEXT, remote_type TEXT, posted_at TEXT, apply_url TEXT NOT NULL, description TEXT, skills_json TEXT NOT NULL DEFAULT '[]', match_score INTEGER NOT NULL DEFAULT 0, match_reason TEXT NOT NULL DEFAULT '', profile_matches_json TEXT NOT NULL DEFAULT '[]', is_new INTEGER NOT NULL DEFAULT 0, active INTEGER NOT NULL DEFAULT 1, imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE(source, external_id));
         CREATE INDEX IF NOT EXISTS jobs_active_posted_idx ON jobs(active, posted_at DESC);
         CREATE INDEX IF NOT EXISTS jobs_company_idx ON jobs(company);
         CREATE TABLE IF NOT EXISTS profiles (id TEXT PRIMARY KEY, name TEXT NOT NULL, titles_json TEXT NOT NULL DEFAULT '[]', required_skills_json TEXT NOT NULL DEFAULT '[]', preferred_skills_json TEXT NOT NULL DEFAULT '[]', resume_text TEXT NOT NULL DEFAULT '', remote_only INTEGER NOT NULL DEFAULT 0, countries_json TEXT NOT NULL DEFAULT '[]', regions_json TEXT NOT NULL DEFAULT '[]', cities_json TEXT NOT NULL DEFAULT '[]', min_score INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1, schedule TEXT NOT NULL DEFAULT 'Manual', weights_json TEXT, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
         CREATE TABLE IF NOT EXISTS sources (id TEXT PRIMARY KEY, kind TEXT NOT NULL, company TEXT NOT NULL, board_token TEXT NOT NULL, location TEXT, enabled INTEGER NOT NULL DEFAULT 1, last_refreshed_at TEXT, last_error TEXT, UNIQUE(kind, board_token));
         CREATE TABLE IF NOT EXISTS search_runs (id TEXT PRIMARY KEY, started_at TEXT NOT NULL, finished_at TEXT, trigger_type TEXT NOT NULL, sources_checked INTEGER NOT NULL DEFAULT 0, jobs_imported INTEGER NOT NULL DEFAULT 0, errors_json TEXT NOT NULL DEFAULT '[]');
         CREATE TABLE IF NOT EXISTS schedules (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id), cadence TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, next_run_at TEXT, last_run_at TEXT);
         CREATE TABLE IF NOT EXISTS tracker_entries (job_id TEXT PRIMARY KEY REFERENCES jobs(id), status TEXT NOT NULL DEFAULT 'New', notes TEXT NOT NULL DEFAULT '', contact_name TEXT NOT NULL DEFAULT '', contact_email TEXT NOT NULL DEFAULT '', application_date TEXT, interview_date TEXT, follow_up_at TEXT);
         CREATE TABLE IF NOT EXISTS backups (id TEXT PRIMARY KEY, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, file_path TEXT NOT NULL);"
    ).map_err(|error| error.to_string())?;
    add_job_column_if_missing(&database, "match_score", "INTEGER NOT NULL DEFAULT 0")?;
    add_job_column_if_missing(&database, "match_reason", "TEXT NOT NULL DEFAULT ''")?;
    add_job_column_if_missing(&database, "is_new", "INTEGER NOT NULL DEFAULT 0")?;
    add_job_column_if_missing(&database, "source_id", "TEXT")?;
    add_job_column_if_missing(&database, "profile_matches_json", "TEXT NOT NULL DEFAULT '[]'")?;
    add_tracker_column_if_missing(&database, "contact_name", "TEXT NOT NULL DEFAULT ''")?;
    add_tracker_column_if_missing(&database, "contact_email", "TEXT NOT NULL DEFAULT ''")?;
    add_tracker_column_if_missing(&database, "application_date", "TEXT")?;
    add_tracker_column_if_missing(&database, "interview_date", "TEXT")?;
    add_table_column_if_missing(&database, "profiles", "enabled", "INTEGER NOT NULL DEFAULT 1")?;
    add_table_column_if_missing(&database, "profiles", "schedule", "TEXT NOT NULL DEFAULT 'Manual'")?;
    add_table_column_if_missing(&database, "profiles", "weights_json", "TEXT")?;
    add_table_column_if_missing(&database, "profiles", "countries_json", "TEXT NOT NULL DEFAULT '[]'")?;
    add_table_column_if_missing(&database, "profiles", "regions_json", "TEXT NOT NULL DEFAULT '[]'")?;
    add_table_column_if_missing(&database, "profiles", "cities_json", "TEXT NOT NULL DEFAULT '[]'")?;
    add_table_column_if_missing(&database, "sources", "location", "TEXT")?;
    Ok(database)
}

fn add_tracker_column_if_missing(database: &Connection, column: &str, definition: &str) -> Result<(), String> {
    let mut statement = database.prepare("PRAGMA table_info(tracker_entries)").map_err(|error| error.to_string())?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1)).map_err(|error| error.to_string())?;
    if columns.filter_map(Result::ok).any(|name| name == column) { return Ok(()); }
    database.execute_batch(&format!("ALTER TABLE tracker_entries ADD COLUMN {column} {definition}")).map_err(|error| error.to_string())
}

fn add_table_column_if_missing(database: &Connection, table: &str, column: &str, definition: &str) -> Result<(), String> {
    let mut statement = database.prepare(&format!("PRAGMA table_info({table})")).map_err(|error| error.to_string())?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1)).map_err(|error| error.to_string())?;
    if columns.filter_map(Result::ok).any(|name| name == column) { return Ok(()); }
    database.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}")).map_err(|error| error.to_string())
}

fn add_job_column_if_missing(database: &Connection, column: &str, definition: &str) -> Result<(), String> {
    let mut statement = database.prepare("PRAGMA table_info(jobs)").map_err(|error| error.to_string())?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1)).map_err(|error| error.to_string())?;
    if columns.filter_map(Result::ok).any(|name| name == column) { return Ok(()); }
    database.execute_batch(&format!("ALTER TABLE jobs ADD COLUMN {column} {definition}"))
        .map_err(|error| error.to_string())
}

fn seed_jobs_if_empty(database: &Connection) -> Result<(), String> {
    let count: i64 = database.query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if count > 0 { return Ok(()); }
    let examples = [
        ("job-1", "Greenhouse", "gh-4289", "Senior Engineering Manager", "Northstar Labs", "Canada · Remote", "Remote", "Today", "https://boards.greenhouse.io", "Lead a product engineering team building TypeScript services and React experiences.", r#"["TypeScript","React","Leadership","PostgreSQL"]"#, 92, "Strong title match, remote in Canada, and all 3 required skills found.", 1, "New"),
        ("job-2", "Lever", "lever-906", "Engineering Manager, Platform", "Waypoint", "Toronto, ON · Hybrid", "Hybrid", "Yesterday", "https://jobs.lever.co", "Manage platform developers and scale developer tools.", r#"["TypeScript","Leadership","Kubernetes"]"#, 79, "Excellent leadership and skills alignment; hybrid location is a partial preference match.", 1, "Saved"),
        ("job-3", "Ashby", "ashby-128", "Director of Engineering", "Signal & Pine", "Remote · Canada", "Remote", "2 days ago", "https://jobs.ashbyhq.com", "Build and mentor a high-performing full-stack product organization.", r#"["React","Leadership","Strategy"]"#, 76, "Matches title, leadership, React, and location. TypeScript is not explicitly listed.", 1, "Applied"),
        ("job-4", "USAJOBS", "usa-19", "IT Program Manager", "Government of Canada", "Ottawa, ON", "On-site", "3 days ago", "https://www.usajobs.gov", "Manage technical programs across teams.", r#"["Leadership","Program Management"]"#, 54, "Leadership matches, but job is on-site and lacks core technical keywords.", 0, "New"),
    ];
    let transaction = database.unchecked_transaction().map_err(|error| error.to_string())?;
    for job in examples {
        transaction.execute("INSERT INTO jobs(id, source, external_id, title, company, location, remote_type, posted_at, apply_url, description, skills_json, match_score, match_reason, is_new) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)", params![job.0, job.1, job.2, job.3, job.4, job.5, job.6, job.7, job.8, job.9, job.10, job.11, job.12, job.13]).map_err(|error| error.to_string())?;
        transaction.execute("INSERT INTO tracker_entries(job_id, status) VALUES (?1, ?2)", params![job.0, job.14]).map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
fn database_health(database: State<Database>) -> Result<String, String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.query_row("SELECT 'ready'", [], |row| row.get(0)).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_jobs(database: State<Database>) -> Result<Vec<JobRecord>, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let mut statement = connection.prepare("SELECT jobs.id, jobs.source, jobs.external_id, jobs.title, jobs.company, COALESCE(jobs.location, ''), COALESCE(jobs.remote_type, 'Unknown'), COALESCE(jobs.posted_at, ''), jobs.apply_url, COALESCE(jobs.description, ''), jobs.skills_json, jobs.match_score, jobs.match_reason, jobs.profile_matches_json, jobs.is_new, COALESCE(tracker_entries.status, 'New') FROM jobs LEFT JOIN tracker_entries ON tracker_entries.job_id = jobs.id WHERE jobs.active = 1 ORDER BY jobs.match_score DESC, jobs.imported_at DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| {
        let skills_json: String = row.get(10)?;
        Ok(JobRecord { id: row.get(0)?, source: row.get(1)?, external_id: row.get(2)?, title: row.get(3)?, company: row.get(4)?, location: row.get(5)?, remote_type: row.get(6)?, posted_at: row.get(7)?, apply_url: row.get(8)?, description: row.get(9)?, skills: serde_json::from_str(&skills_json).unwrap_or_default(), match_score: row.get(11)?, match_reason: row.get(12)?, profile_matches: serde_json::from_str(&row.get::<_, String>(13)?).unwrap_or_default(), is_new: row.get::<_, i64>(14)? != 0, status: row.get(15)? })
    }).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_tracker_status(database: State<Database>, job_id: String, status: String) -> Result<(), String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.execute("INSERT INTO tracker_entries(job_id, status) VALUES (?1, ?2) ON CONFLICT(job_id) DO UPDATE SET status = excluded.status", params![job_id, status]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_profiles(database: State<Database>) -> Result<Vec<ProfileRecord>, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let mut statement = connection.prepare("SELECT id, name, titles_json, required_skills_json, preferred_skills_json, resume_text, remote_only, countries_json, regions_json, cities_json, min_score, enabled, schedule, weights_json FROM profiles ORDER BY updated_at DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| { let weights: Option<String> = row.get(13)?; Ok(ProfileRecord { id: row.get(0)?, name: row.get(1)?, titles: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(), required_skills: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(), preferred_skills: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(), resume_text: row.get(5)?, remote_only: row.get::<_, i64>(6)? != 0, countries: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(), regions: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(), cities: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(), min_score: row.get(10)?, enabled: row.get::<_, i64>(11)? != 0, schedule: row.get(12)?, weights: weights.and_then(|value| serde_json::from_str(&value).ok()) }) }).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_profile(database: State<Database>, profile: ProfileRecord) -> Result<(), String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.execute("INSERT INTO profiles(id, name, titles_json, required_skills_json, preferred_skills_json, resume_text, remote_only, countries_json, regions_json, cities_json, min_score, enabled, schedule, weights_json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, CURRENT_TIMESTAMP) ON CONFLICT(id) DO UPDATE SET name=excluded.name, titles_json=excluded.titles_json, required_skills_json=excluded.required_skills_json, preferred_skills_json=excluded.preferred_skills_json, resume_text=excluded.resume_text, remote_only=excluded.remote_only, countries_json=excluded.countries_json, regions_json=excluded.regions_json, cities_json=excluded.cities_json, min_score=excluded.min_score, enabled=excluded.enabled, schedule=excluded.schedule, weights_json=excluded.weights_json, updated_at=CURRENT_TIMESTAMP", params![profile.id, profile.name, serde_json::to_string(&profile.titles).map_err(|error| error.to_string())?, serde_json::to_string(&profile.required_skills).map_err(|error| error.to_string())?, serde_json::to_string(&profile.preferred_skills).map_err(|error| error.to_string())?, profile.resume_text, profile.remote_only as i64, serde_json::to_string(&profile.countries).map_err(|error| error.to_string())?, serde_json::to_string(&profile.regions).map_err(|error| error.to_string())?, serde_json::to_string(&profile.cities).map_err(|error| error.to_string())?, profile.min_score, profile.enabled as i64, profile.schedule, profile.weights.map(|value| value.to_string())]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_profile(database: State<Database>, profile_id: String) -> Result<(), String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.execute("DELETE FROM profiles WHERE id=?1", params![profile_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_sources(database: State<Database>) -> Result<Vec<SourceRecord>, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let mut statement = connection.prepare("SELECT id, kind, company, board_token, location, enabled, last_refreshed_at, last_error FROM sources ORDER BY company").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok(SourceRecord { id: row.get(0)?, kind: row.get(1)?, company: row.get(2)?, board_token: row.get(3)?, location: row.get(4)?, enabled: row.get::<_, i64>(5)? != 0, last_refreshed_at: row.get(6)?, last_error: row.get(7)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_source(database: State<Database>, source: SourceRecord) -> Result<(), String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.execute("INSERT INTO sources(id, kind, company, board_token, location, enabled, last_refreshed_at, last_error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, company=excluded.company, board_token=excluded.board_token, location=excluded.location, enabled=excluded.enabled, last_refreshed_at=COALESCE(excluded.last_refreshed_at, sources.last_refreshed_at), last_error=excluded.last_error", params![source.id, source.kind, source.company, source.board_token, source.location, source.enabled as i64, source.last_refreshed_at, source.last_error]).map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_board_token(token: &str) -> Result<&str, String> {
    let value = token.trim();
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_')) {
        return Err("The company board identifier contains unsupported characters.".into());
    }
    Ok(value)
}

#[tauri::command]
async fn fetch_source_payload(source: SourceRecord, usa_jobs_email: String, usa_jobs_api_key: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().map_err(|error| error.to_string())?;
    let request = match source.kind.as_str() {
        "Greenhouse" => client.get(format!("https://boards-api.greenhouse.io/v1/boards/{}/jobs", validate_board_token(&source.board_token)?)).query(&[("content", "true")]),
        "Lever" => client.get(format!("https://api.lever.co/v0/postings/{}", validate_board_token(&source.board_token)?)).query(&[("mode", "json")]),
        "Ashby" => client.get(format!("https://api.ashbyhq.com/posting-api/job-board/{}", validate_board_token(&source.board_token)?)).query(&[("includeCompensation", "true")]),
        "USAJOBS" => {
            if usa_jobs_email.trim().is_empty() || usa_jobs_api_key.trim().is_empty() { return Err("USAJOBS requires an email address and API key for this session.".into()); }
            let mut request = client.get("https://data.usajobs.gov/api/search")
                .header(reqwest::header::USER_AGENT, usa_jobs_email.trim())
                .header("Authorization-Key", usa_jobs_api_key.trim())
                .query(&[("Keyword", source.board_token.trim()), ("ResultsPerPage", "100")]);
            if let Some(location) = source.location.as_deref().filter(|value| !value.trim().is_empty()) { request = request.query(&[("LocationName", location.trim())]); }
            request
        },
        _ => return Err(format!("Unsupported source type: {}", source.kind)),
    };
    let response = request.send().await.map_err(|error| format!("{} request failed: {error}", source.kind))?;
    let status = response.status();
    if !status.is_success() { return Err(format!("{} returned HTTP {status}. Check the source settings and credentials.", source.kind)); }
    response.json::<serde_json::Value>().await.map_err(|error| format!("{} returned invalid JSON: {error}", source.kind))
}

#[tauri::command]
fn delete_source(database: State<Database>, source_id: String) -> Result<(), String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.execute("DELETE FROM sources WHERE id=?1", params![source_id]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn persist_source_jobs(database: State<Database>, source_id: String, jobs: Vec<ImportedJob>, error: Option<String>, trigger_type: Option<String>) -> Result<ImportResult, String> {
    let mut connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    transaction.query_row("SELECT id FROM sources WHERE id=?1", params![source_id], |row| row.get::<_, String>(0)).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE jobs SET active=0 WHERE source_id=?1", params![source_id]).map_err(|error| error.to_string())?;
    for job in &jobs {
        transaction.execute("INSERT INTO jobs(id, source, source_id, external_id, title, company, location, remote_type, posted_at, apply_url, description, skills_json, match_score, match_reason, profile_matches_json, is_new, active) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,1,1) ON CONFLICT(source, external_id) DO UPDATE SET source_id=excluded.source_id, title=excluded.title, company=excluded.company, location=excluded.location, remote_type=excluded.remote_type, posted_at=excluded.posted_at, apply_url=excluded.apply_url, description=excluded.description, skills_json=excluded.skills_json, match_score=excluded.match_score, match_reason=excluded.match_reason, profile_matches_json=excluded.profile_matches_json, active=1", params![job.id, job.source, source_id, job.external_id, job.title, job.company, job.location, job.remote_type, job.posted_at, job.apply_url, job.description, serde_json::to_string(&job.skills).map_err(|error| error.to_string())?, job.match_score, job.match_reason, serde_json::to_string(&job.profile_matches).map_err(|error| error.to_string())?]).map_err(|error| error.to_string())?;
    }
    let inactive: i64 = transaction.query_row("SELECT COUNT(*) FROM jobs WHERE source_id=?1 AND active=0", params![source_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    transaction.execute("UPDATE sources SET last_refreshed_at=CURRENT_TIMESTAMP, last_error=?2 WHERE id=?1", params![source_id, error]).map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO search_runs(id, started_at, finished_at, trigger_type, sources_checked, jobs_imported, errors_json) VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?2, 1, ?3, ?4)", params![format!("run-{}", chrono::Utc::now().timestamp_millis()), trigger_type.unwrap_or_else(|| "manual".into()), jobs.len() as i64, if error.is_some() { serde_json::to_string(&vec![error]).unwrap_or_else(|_| "[]".into()) } else { "[]".into() }]).map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(ImportResult { imported: jobs.len(), inactive: inactive as usize })
}

#[tauri::command]
fn record_source_error(database: State<Database>, source_id: String, message: String, trigger_type: Option<String>) -> Result<(), String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    connection.execute("UPDATE sources SET last_error=?2 WHERE id=?1", params![source_id, message]).map_err(|error| error.to_string())?;
    connection.execute("INSERT INTO search_runs(id, started_at, finished_at, trigger_type, sources_checked, jobs_imported, errors_json) VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?2, 1, 0, ?3)", params![format!("run-{}", chrono::Utc::now().timestamp_millis()), trigger_type.unwrap_or_else(|| "manual".into()), serde_json::to_string(&vec![message]).unwrap_or_else(|_| "[]".into())]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_search_runs(database: State<Database>) -> Result<Vec<SearchRunRecord>, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let mut statement = connection.prepare("SELECT id, started_at, finished_at, trigger_type, sources_checked, jobs_imported, errors_json FROM search_runs ORDER BY started_at DESC LIMIT 25").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| { let errors_json: String = row.get(6)?; Ok(SearchRunRecord { id: row.get(0)?, started_at: row.get(1)?, finished_at: row.get(2)?, trigger_type: row.get(3)?, sources_checked: row.get(4)?, jobs_imported: row.get(5)?, errors: serde_json::from_str(&errors_json).unwrap_or_default() }) }).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
fn load_tracker(database: State<Database>, job_id: String) -> Result<TrackerRecord, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    connection.query_row("SELECT job_id, status, notes, contact_name, contact_email, application_date, interview_date, follow_up_at FROM tracker_entries WHERE job_id=?1", params![job_id], |row| Ok(TrackerRecord { job_id: row.get(0)?, status: row.get(1)?, notes: row.get(2)?, contact_name: row.get(3)?, contact_email: row.get(4)?, application_date: row.get(5)?, interview_date: row.get(6)?, follow_up_at: row.get(7)? })).optional().map_err(|error| error.to_string())?.ok_or_else(|| "Tracker entry was not found.".into())
}

#[tauri::command]
fn save_tracker(database: State<Database>, tracker: TrackerRecord) -> Result<(), String> {
    database.0.lock().map_err(|_| "Database lock unavailable".to_string())?.execute("INSERT INTO tracker_entries(job_id, status, notes, contact_name, contact_email, application_date, interview_date, follow_up_at) VALUES (?1,?2,?3,?4,?5,NULLIF(?6,''),NULLIF(?7,''),NULLIF(?8,'')) ON CONFLICT(job_id) DO UPDATE SET status=excluded.status, notes=excluded.notes, contact_name=excluded.contact_name, contact_email=excluded.contact_email, application_date=excluded.application_date, interview_date=excluded.interview_date, follow_up_at=excluded.follow_up_at", params![tracker.job_id, tracker.status, tracker.notes, tracker.contact_name, tracker.contact_email, tracker.application_date, tracker.interview_date, tracker.follow_up_at]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_diagnostics(database: State<Database>) -> Result<DiagnosticsRecord, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let count = |sql: &str| connection.query_row(sql, [], |row| row.get::<_, i64>(0)).map_err(|error| error.to_string());
    Ok(DiagnosticsRecord {
        database_path: database.1.display().to_string(),
        database_size_bytes: fs::metadata(&database.1).map(|value| value.len()).unwrap_or(0),
        active_jobs: count("SELECT COUNT(*) FROM jobs WHERE active=1")?,
        inactive_jobs: count("SELECT COUNT(*) FROM jobs WHERE active=0")?,
        profiles: count("SELECT COUNT(*) FROM profiles")?,
        enabled_sources: count("SELECT COUNT(*) FROM sources WHERE enabled=1")?,
        search_runs: count("SELECT COUNT(*) FROM search_runs")?,
        source_errors: count("SELECT COUNT(*) FROM sources WHERE last_error IS NOT NULL AND last_error != ''")?,
        due_follow_ups: count("SELECT COUNT(*) FROM tracker_entries WHERE follow_up_at IS NOT NULL AND datetime(follow_up_at) <= datetime('now') AND status NOT IN ('Rejected','Archived')")?,
    })
}

#[tauri::command]
fn apply_retention(database: State<Database>, retention_days: i64) -> Result<usize, String> {
    let days = retention_days.clamp(7, 3650);
    let modifier = format!("-{days} days");
    let mut connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let jobs = transaction.execute("DELETE FROM jobs WHERE active=0 AND datetime(imported_at) < datetime('now', ?1)", params![modifier]).map_err(|error| error.to_string())?;
    let runs = transaction.execute("DELETE FROM search_runs WHERE datetime(started_at) < datetime('now', ?1)", params![modifier]).map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(jobs + runs)
}

#[tauri::command]
fn backup_database(database: State<Database>, path: String) -> Result<(), String> {
    let source = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let mut destination = Connection::open(path).map_err(|error| error.to_string())?;
    let backup = backup::Backup::new(&source, &mut destination).map_err(|error| error.to_string())?;
    backup.run_to_completion(64, Duration::from_millis(10), None).map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_database(database: State<Database>, path: String) -> Result<(), String> {
    let source = Connection::open(path).map_err(|error| format!("Unable to open backup: {error}"))?;
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if integrity != "ok" { return Err(format!("Backup integrity check failed: {integrity}")); }
    let mut destination = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let backup = backup::Backup::new(&source, &mut destination).map_err(|error| error.to_string())?;
    backup.run_to_completion(64, Duration::from_millis(10), None).map_err(|error| error.to_string())?;
    Ok(())
}

fn csv_cell(value: &str) -> String { format!("\"{}\"", value.replace('"', "\"\"")) }

#[tauri::command]
fn export_jobs_csv(database: State<Database>, path: String) -> Result<usize, String> {
    let connection = database.0.lock().map_err(|_| "Database lock unavailable".to_string())?;
    let mut statement = connection.prepare("SELECT jobs.title, jobs.company, jobs.source, jobs.location, jobs.posted_at, jobs.apply_url, jobs.match_score, COALESCE(tracker_entries.status,'New'), COALESCE(tracker_entries.notes,''), COALESCE(tracker_entries.contact_name,''), COALESCE(tracker_entries.contact_email,''), COALESCE(tracker_entries.application_date,''), COALESCE(tracker_entries.follow_up_at,'') FROM jobs LEFT JOIN tracker_entries ON jobs.id=tracker_entries.job_id WHERE jobs.active=1 ORDER BY jobs.match_score DESC").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok((0..13).map(|index| row.get::<_, String>(index).unwrap_or_else(|_| row.get::<_, i64>(index).map(|value| value.to_string()).unwrap_or_default())).collect::<Vec<_>>())).map_err(|error| error.to_string())?;
    let mut file = fs::File::create(path).map_err(|error| error.to_string())?;
    writeln!(file, "title,company,source,location,posted_at,apply_url,match_score,status,notes,contact_name,contact_email,application_date,follow_up_at").map_err(|error| error.to_string())?;
    let mut count = 0;
    for row in rows { writeln!(file, "{}", row.map_err(|error| error.to_string())?.iter().map(|value| csv_cell(value)).collect::<Vec<_>>().join(",")).map_err(|error| error.to_string())?; count += 1; }
    Ok(count)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let path = app.path().app_data_dir().expect("unable to resolve application-data directory").join("job-tracker.sqlite3");
            let database = open_database(path.clone()).expect("unable to initialize local database");
            seed_jobs_if_empty(&database).expect("unable to seed local jobs");
            app.manage(Database(Mutex::new(database), path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![database_health, list_jobs, save_tracker_status, load_tracker, save_tracker, list_profiles, save_profile, delete_profile, list_sources, save_source, delete_source, fetch_source_payload, persist_source_jobs, record_source_error, list_search_runs, get_diagnostics, apply_retention, backup_database, restore_database, export_jobs_csv])
        .run(tauri::generate_context!())
        .expect("error while running Job Tracker");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("job-tracker-{name}-{}-{}.sqlite3", std::process::id(), chrono::Utc::now().timestamp_millis()))
    }

    #[test]
    fn creates_complete_current_schema() {
        let path = test_path("schema");
        let database = open_database(path.clone()).expect("database should open");
        for table in ["profiles", "sources", "jobs", "tracker_entries", "search_runs", "schedules", "backups"] {
            let found: i64 = database.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1", params![table], |row| row.get(0)).unwrap();
            assert_eq!(found, 1, "missing table {table}");
        }
        let source_location: i64 = database.query_row("SELECT COUNT(*) FROM pragma_table_info('sources') WHERE name='location'", [], |row| row.get(0)).unwrap();
        let job_source_id: i64 = database.query_row("SELECT COUNT(*) FROM pragma_table_info('jobs') WHERE name='source_id'", [], |row| row.get(0)).unwrap();
        let job_profile_matches: i64 = database.query_row("SELECT COUNT(*) FROM pragma_table_info('jobs') WHERE name='profile_matches_json'", [], |row| row.get(0)).unwrap();
        let profile_weights: i64 = database.query_row("SELECT COUNT(*) FROM pragma_table_info('profiles') WHERE name='weights_json'", [], |row| row.get(0)).unwrap();
        let profile_location_columns: i64 = database.query_row("SELECT COUNT(*) FROM pragma_table_info('profiles') WHERE name IN ('countries_json','regions_json','cities_json')", [], |row| row.get(0)).unwrap();
        assert_eq!(source_location, 1, "missing sources.location migration");
        assert_eq!(job_source_id, 1, "missing jobs.source_id migration");
        assert_eq!(job_profile_matches, 1, "missing jobs.profile_matches_json migration");
        assert_eq!(profile_weights, 1, "missing profiles.weights_json migration");
        assert_eq!(profile_location_columns, 3, "missing profile location migrations");
        drop(database);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn seed_is_idempotent_and_preserves_tracker_rows() {
        let path = test_path("seed");
        let database = open_database(path.clone()).unwrap();
        seed_jobs_if_empty(&database).unwrap();
        seed_jobs_if_empty(&database).unwrap();
        let jobs: i64 = database.query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0)).unwrap();
        let tracked: i64 = database.query_row("SELECT COUNT(*) FROM tracker_entries", [], |row| row.get(0)).unwrap();
        assert_eq!(jobs, 4);
        assert_eq!(tracked, 4);
        drop(database);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn csv_escaping_preserves_commas_quotes_and_newlines() {
        assert_eq!(csv_cell("Acme, \"Inc\"\nRemote"), "\"Acme, \"\"Inc\"\"\nRemote\"");
    }
}
