use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self { Self::Unauthorized => StatusCode::UNAUTHORIZED, Self::Forbidden => StatusCode::FORBIDDEN, Self::BadRequest(_) => StatusCode::BAD_REQUEST, Self::NotFound => StatusCode::NOT_FOUND, Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError { fn from(value: sqlx::Error) -> Self { tracing::error!(?value, "database error"); Self::Internal("database operation failed".into()) } }
impl From<reqwest::Error> for ApiError { fn from(value: reqwest::Error) -> Self { Self::BadRequest(format!("source request failed: {value}")) } }
impl From<serde_json::Error> for ApiError { fn from(value: serde_json::Error) -> Self { Self::BadRequest(format!("invalid data: {value}")) } }

