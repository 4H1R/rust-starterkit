use axum::{
    Extension, Json,
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

#[derive(Clone, Copy)]
pub struct AppError(pub StatusCode, pub &'static str);
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
        // request_context renders the problem once its correlation ID is available.
        (self.0, Extension(self)).into_response()
    }
}

pub(crate) fn normalize(mut response: Response, request_id: &str) -> Response {
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    // Only AppError supplies safe public detail; never inspect arbitrary response bodies.
    let detail = response
        .extensions_mut()
        .remove::<AppError>()
        .map_or("Request failed", |error| error.1);
    let problem = Problem {
        kind: "about:blank".into(),
        title: status.canonical_reason().unwrap_or("Error").into(),
        status: status.as_u16(),
        detail: detail.into(),
        request_id: request_id.into(),
    };
    *response.body_mut() = Json(problem).into_response().into_body();
    let headers = response.headers_mut();
    for name in [
        "content-length",
        "content-encoding",
        "content-range",
        "etag",
        "last-modified",
    ] {
        headers.remove(name);
    }
    headers.insert(
        "content-type",
        axum::http::HeaderValue::from_static("application/problem+json"),
    );
    response
}
