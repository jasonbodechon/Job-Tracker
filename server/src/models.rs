use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User { pub id: Uuid, pub email: String, pub display_name: String, pub role:String }

#[derive(Debug, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct Credentials { pub email: String, pub password: String, pub display_name: Option<String> }

fn title_weight()->i32{20} fn required_weight()->i32{30} fn preferred_weight()->i32{10} fn resume_weight()->i32{20} fn experience_weight()->i32{15} fn education_weight()->i32{5} fn proximity_weight()->i32{10}
#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct Weights {
 #[serde(default="title_weight")] pub title: i32,
 #[serde(default="required_weight")] pub required_skills: i32,
 #[serde(default="preferred_weight")] pub preferred_skills: i32,
 #[serde(default="resume_weight",alias="remote")] pub resume_evidence: i32,
 #[serde(default="experience_weight",alias="freshness")] pub experience: i32,
 #[serde(default="education_weight")] pub education: i32,
 #[serde(default="proximity_weight")] pub proximity: i32,
}
impl Default for Weights { fn default() -> Self { Self { title:title_weight(),required_skills:required_weight(),preferred_skills:preferred_weight(),resume_evidence:resume_weight(),experience:experience_weight(),education:education_weight(),proximity:proximity_weight() } } }

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)] #[serde(rename_all = "camelCase")]
pub struct ProfileRow { pub id: String, pub name: String, pub titles: Value, pub required_skills: Value, pub preferred_skills: Value, pub remote_only: bool, pub countries: Value, pub regions: Value, pub cities: Value, pub min_score: i32, pub enabled: bool, pub schedule: String, pub resume_text: String, pub weights: Value, pub distance_enabled: bool, pub active_location: String, pub active_latitude: Option<f64>, pub active_longitude: Option<f64>, pub maximum_distance_km: f64, pub include_remote: bool, pub include_hybrid: bool, pub include_onsite: bool, pub broad_location_policy: String }

#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct Profile { pub id: String, pub name: String, pub titles: Vec<String>, pub required_skills: Vec<String>, pub preferred_skills: Vec<String>, pub remote_only: bool, pub countries: Vec<String>, pub regions: Vec<String>, pub cities: Vec<String>, pub min_score: i32, pub enabled: bool, pub schedule: String, pub resume_text: String, #[serde(default)] pub weights: Weights, #[serde(default)] pub distance_enabled: bool, #[serde(default)] pub active_location: String, #[serde(default)] pub active_latitude: Option<f64>, #[serde(default)] pub active_longitude: Option<f64>, #[serde(default="default_distance")] pub maximum_distance_km: f64, #[serde(default="default_true")] pub include_remote: bool, #[serde(default="default_true")] pub include_hybrid: bool, #[serde(default="default_true")] pub include_onsite: bool, #[serde(default="default_broad_policy")] pub broad_location_policy: String }

fn default_distance()->f64{75.0} fn default_true()->bool{true} fn default_broad_policy()->String{"review".into()}

impl TryFrom<ProfileRow> for Profile { type Error = serde_json::Error; fn try_from(r: ProfileRow) -> Result<Self, Self::Error> { Ok(Self { id:r.id,name:r.name,titles:serde_json::from_value(r.titles)?,required_skills:serde_json::from_value(r.required_skills)?,preferred_skills:serde_json::from_value(r.preferred_skills)?,remote_only:r.remote_only,countries:serde_json::from_value(r.countries)?,regions:serde_json::from_value(r.regions)?,cities:serde_json::from_value(r.cities)?,min_score:r.min_score,enabled:r.enabled,schedule:r.schedule,resume_text:r.resume_text,weights:serde_json::from_value(r.weights)?,distance_enabled:r.distance_enabled,active_location:r.active_location,active_latitude:r.active_latitude,active_longitude:r.active_longitude,maximum_distance_km:r.maximum_distance_km,include_remote:r.include_remote,include_hybrid:r.include_hybrid,include_onsite:r.include_onsite,broad_location_policy:r.broad_location_policy }) } }

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)] #[serde(rename_all = "camelCase")]
pub struct Source { pub id: String, pub kind: String, pub company: String, pub board_token: String, pub location: Option<String>, pub enabled: bool, pub last_refreshed_at: Option<DateTime<Utc>>, pub last_error: Option<String>, #[serde(default)] pub request_delay_ms:i32, #[serde(default)] pub max_retries:i32, #[serde(default)] pub max_results:i32, #[serde(default)] pub page_size:i32, #[serde(default)] pub max_calls:i32, #[serde(default)] pub hydration_limit:i32 }

#[derive(Debug, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct SourceInput { #[serde(flatten)] pub source: Source, pub usa_jobs_email: Option<String>, pub usa_jobs_api_key: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)] #[serde(rename_all = "camelCase")]
pub struct CompanyDirectoryEntry {
 pub id:String,
 pub enabled:bool,
 pub company_name:String,
 pub countries:Value,
 pub ats_provider:String,
 pub connector_class:String,
 pub board_identifier:String,
 pub careers_url:String,
 pub current_job_count:i32,
 pub last_verified_at:Option<DateTime<Utc>>,
 pub verification_status:String,
 pub verification_error:Option<String>,
 pub identifier_origin:String,
 pub detected_provider:Option<String>,
 pub redirect_url:Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)] #[serde(rename_all = "camelCase")]
pub struct Job { pub id: String, pub source: String, pub external_id: String, pub title: String, pub company: String, pub location: String, pub remote_type: String, pub posted_at: String, pub apply_url: String, pub description: String, pub skills: Value, pub match_score: i32, pub match_reason: String, pub profile_matches: Value, pub is_new: bool, pub status: String, #[serde(default)] pub distance_km: Option<f64>, #[serde(default)] pub location_status: String }

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)] #[serde(rename_all = "camelCase")]
pub struct Tracker { pub job_id: String, pub status: String, pub notes: String, pub contact_name: String, pub contact_email: String, pub application_date: Option<String>, pub interview_date: Option<String>, pub follow_up_at: Option<String> }

#[derive(Debug, Serialize, FromRow)] #[serde(rename_all = "camelCase")]
pub struct SearchRun { pub id: String, pub started_at: String, pub finished_at: Option<String>, pub trigger_type: String, pub sources_checked: i32, pub jobs_imported: i32, pub errors: Value, pub status: String, pub progress: String, pub result: Option<Value> }

#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ImportResult {
 pub imported: usize,
 pub inactive: usize,
 pub fetched: usize,
 pub available: usize,
 pub outside_location: usize,
 pub below_minimum_score: usize,
 pub source_errors: Vec<String>,
 pub truncated_sources: Vec<String>,
}

#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Diagnostics { pub database_path: String, pub database_size_bytes: i64, pub active_jobs: i64, pub inactive_jobs: i64, pub profiles: i64, pub enabled_sources: i64, pub search_runs: i64, pub source_errors: i64, pub due_follow_ups: i64 }

#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ScoreComponent { pub key:String,pub label:String,pub earned:f64,pub possible:i32,pub details:String }

#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ProfileMatch {
 pub profile_id: String, pub profile_name: String, pub match_score: i32, pub match_reason: String,
 #[serde(default)] pub score_breakdown:Vec<ScoreComponent>,#[serde(default)] pub matched_terms:Vec<String>,#[serde(default)] pub missing_terms:Vec<String>,
 #[serde(default)] pub resume_years:Option<f64>,#[serde(default)] pub required_years:Option<f64>,#[serde(default)] pub algorithm_version:String,
 #[serde(default)] pub distance_km:Option<f64>,#[serde(default)] pub location_status:String,#[serde(default)] pub location_details:String,
}

#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ResolvedLocation { pub label:String,pub latitude:f64,pub longitude:f64,pub confidence:String }

#[derive(Debug, Clone)]
pub struct Posting { pub external_id:String, pub title:String, pub company:String, pub location:String, pub remote_type:String, pub posted_at:String, pub apply_url:String, pub description:String }

#[derive(Debug, Clone, FromRow)]
pub struct StoredPosting { pub id:String,pub external_id:String,pub title:String,pub company:String,pub location:String,pub remote_type:String,pub posted_at:String,pub apply_url:String,pub description:String,pub location_candidates:Value }
