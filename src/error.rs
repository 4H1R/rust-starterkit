use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    /// Server-generated correlation ID, also returned in X-Request-ID.
    pub request_id: String,
}

pub struct AppError(pub StatusCode, pub &'static str);
impl AppError {
    pub fn response(self, request_id: &str) -> Response {
        let problem = Problem {
            kind: "about:blank".into(),
            title: self.0.canonical_reason().unwrap_or("Error").into(),
            status: self.0.as_u16(),
            detail: self.1.into(),
            request_id: request_id.into(),
        };
        (
            self.0,
            [("content-type", "application/problem+json")],
            Json(problem),
        )
            .into_response()
    }
}
impl From<sea_orm::DbErr> for AppError {
    fn from(_: sea_orm::DbErr) -> Self {
        // SQL errors may include bound input or credentials. Never log/display their text.
        tracing::error!(error_kind = "database", "database operation failed");
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database operation unavailable",
        )
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.response("")
    }
}
