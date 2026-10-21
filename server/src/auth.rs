use argon2::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString}, Argon2};
use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use crate::{error::ApiError, models::User, AppState};

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    if password.len() < 10 { return Err(ApiError::BadRequest("Password must contain at least 10 characters.".into())); }
    Argon2::default().hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng)).map(|v| v.to_string()).map_err(|_| ApiError::Internal("password hashing failed".into()))
}
pub fn verify_password(password: &str, encoded: &str) -> bool { PasswordHash::new(encoded).ok().and_then(|hash| Argon2::default().verify_password(password.as_bytes(), &hash).ok()).is_some() }
fn token_hash(token: &str) -> String { hex::encode(Sha256::digest(token.as_bytes())) }
pub fn new_token() -> String { format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()) }

pub async fn create_session(state: &AppState, user_id: Uuid) -> Result<String, ApiError> {
    let token = new_token(); let expires = Utc::now() + Duration::days(state.config.session_days);
    sqlx::query("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,$3)").bind(token_hash(&token)).bind(user_id).bind(expires).execute(&state.db).await?;
    Ok(format!("jt_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={};{}", state.config.session_days * 86400, if state.config.cookie_secure { " Secure;" } else { "" }))
}
pub fn clear_cookie(state: &AppState) -> String { format!("jt_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0;{}", if state.config.cookie_secure { " Secure;" } else { "" }) }
fn cookie(headers: &HeaderMap, name: &str) -> Option<String> { headers.get("cookie")?.to_str().ok()?.split(';').filter_map(|part| part.trim().split_once('=')).find(|(key,_)| *key == name).map(|(_,v)| v.to_string()) }
pub async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    let token = cookie(headers,"jt_session").ok_or(ApiError::Unauthorized)?;
    sqlx::query_as::<_,User>("SELECT u.id,u.email,u.display_name,u.role FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.token_hash=$1 AND s.expires_at>now()")
        .bind(token_hash(&token)).fetch_optional(&state.db).await?.ok_or(ApiError::Unauthorized)
}
pub async fn destroy_session(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> { if let Some(token)=cookie(headers,"jt_session") { sqlx::query("DELETE FROM sessions WHERE token_hash=$1").bind(token_hash(&token)).execute(&state.db).await?; } Ok(()) }
