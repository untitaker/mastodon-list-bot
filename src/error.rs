use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use reqwest::header::InvalidHeaderValue;
use tokio::task::JoinError;

#[derive(Debug, thiserror::Error)]
pub enum ResponseError {
    #[error("internal db error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("internal runtime error: {0}")]
    Io(#[from] JoinError),
    #[error("failed to send HTTP request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("failed to construct auth header: {0}")]
    InvalidHeader(#[from] InvalidHeaderValue),
    #[error("invalid JSON input: {0}")]
    Json(#[from] serde_json::Error),
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        tracing::error!("error while serving request: {}", self);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{}\n", self)).into_response()
    }
}
