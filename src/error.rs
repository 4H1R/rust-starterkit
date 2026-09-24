use axum::{
    Extension, Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::collections::BTreeMap;
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
    /// Validation messages keyed by field; `_root` identifies request-wide errors.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub errors: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Default)]
pub struct ValidationErrors(BTreeMap<String, Vec<String>>);

impl ValidationErrors {
    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.entry(field.into()).or_default().push(message.into());
    }

    pub fn merge(&mut self, other: Self) {
        for (field, messages) in other.0 {
            self.0.entry(field).or_default().extend(messages);
        }
    }

    pub fn finish(self) -> Result<(), AppError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(self.into())
        }
    }
}

#[derive(Clone)]
pub struct AppError {
    status: StatusCode,
    detail: &'static str,
    errors: BTreeMap<String, Vec<String>>,
}

impl AppError {
    pub fn new(status: StatusCode, detail: &'static str) -> Self {
        Self {
            status,
            detail,
            errors: BTreeMap::new(),
        }
    }
}

impl From<ValidationErrors> for AppError {
    fn from(errors: ValidationErrors) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            detail: "The given data was invalid.",
            errors: errors.0,
        }
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(_: sea_orm::DbErr) -> Self {
        // SQL errors may include bound input or credentials. Never log/display their text.
        tracing::error!(error_kind = "database", "database operation failed");
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database operation unavailable",
        )
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // request_context renders the problem once its correlation ID is available.
        (self.status, Extension(self)).into_response()
    }
}

pub(crate) fn normalize(mut response: Response, request_id: &str) -> Response {
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    // Only AppError supplies safe public detail; never inspect arbitrary response bodies.
    let error = response
        .extensions_mut()
        .remove::<AppError>()
        .unwrap_or_else(|| AppError::new(status, "Request failed"));
    let problem = Problem {
        kind: "about:blank".into(),
        title: status.canonical_reason().unwrap_or("Error").into(),
        status: status.as_u16(),
        detail: error.detail.into(),
        request_id: request_id.into(),
        errors: error.errors,
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
