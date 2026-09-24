pub mod config;
pub mod db;
pub mod error;
pub mod migration;
pub mod notes;

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, MatchedPath, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use error::AppError;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use std::time::{Duration, Instant};
use tracing::Instrument;
use utoipa::{OpenApi, ToSchema};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
}
#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: String,
}

#[utoipa::path(get, path = "/healthz", responses((status = 200, description = "Process alive; no dependencies checked", body = Health)))]
async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".into(),
    })
}

#[utoipa::path(get, path = "/readyz", responses((status = 200, description = "Database query succeeds", body = Health), (status = 408, description = "Configured request deadline exceeded", body = error::Problem, content_type = "application/problem+json"), (status = 503, description = "Database unavailable", body = error::Problem, content_type = "application/problem+json")))]
async fn ready(State(state): State<AppState>) -> Result<Json<Health>, AppError> {
    use sea_orm::ConnectionTrait;
    tokio::time::timeout(
        Duration::from_secs(2),
        state.db.execute_unprepared("SELECT 1 FROM notes LIMIT 0"),
    )
    .await
    .map_err(|_| {
        AppError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Database readiness deadline exceeded",
        )
    })??;
    Ok(Json(Health {
        status: "ready".into(),
    }))
}

#[derive(OpenApi)]
#[openapi(
    paths(health, ready, notes::create, notes::get),
    components(schemas(Health, notes::Note, notes::CreateNote, error::Problem)),
    info(
        title = "Rust starter example",
        version = "0.1.0",
        description = "Unauthenticated teaching API. Example routes require ENABLE_EXAMPLE=true. See docs/http.md."
    )
)]
pub struct ApiDoc;

pub fn app(state: AppState, config: &config::Config) -> Router {
    let mut router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready));
    if config.enable_example {
        router = router
            .route("/example/notes", post(notes::create))
            .route("/example/notes/{id}", get(notes::get));
    }
    router
        .fallback(|| async { AppError(StatusCode::NOT_FOUND, "Route not found") })
        .method_not_allowed_fallback(|| async {
            AppError(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed")
        })
        .layer(DefaultBodyLimit::max(config.body_limit))
        .layer(middleware::from_fn_with_state(
            config.request_timeout,
            request_context,
        ))
        .with_state(state)
}

async fn request_context(
    State(timeout): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("unmatched")
        .to_owned();
    // Never record raw URI/query, incoming request ID, headers, or request/response bodies.
    let span = tracing::info_span!("http_request", request_id = %request_id, route = %route);
    async {
        let started = Instant::now();
        let mut response = match tokio::time::timeout(timeout, next.run(request)).await {
            Ok(response) => response,
            Err(_) => AppError(StatusCode::REQUEST_TIMEOUT, "Request deadline exceeded")
                .response(&request_id),
        };
        // Stamp every application error consistently, including framework fallbacks/rejections.
        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status();
            let headers = response.headers().clone();
            let body = std::mem::replace(response.body_mut(), Body::empty());
            let detail = match axum::body::to_bytes(body, 4096).await {
                Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .and_then(|v| v["detail"].as_str().map(str::to_owned)),
                Err(_) => None,
            };
            let problem = error::Problem {
                kind: "about:blank".into(),
                title: status.canonical_reason().unwrap_or("Error").into(),
                status: status.as_u16(),
                detail: detail.unwrap_or_else(|| "Request failed".into()),
                request_id: request_id.clone(),
            };
            response = (status, Json(problem)).into_response();
            for name in ["allow", "retry-after", "www-authenticate"] {
                if let Some(value) = headers.get(name) {
                    response.headers_mut().insert(name, value.clone());
                }
            }
            response.headers_mut().insert(
                "content-type",
                HeaderValue::from_static("application/problem+json"),
            );
        }
        response.headers_mut().insert(
            "x-request-id",
            HeaderValue::from_str(&request_id).expect("UUID is a valid header"),
        );
        tracing::info!(
            status = response.status().as_u16(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "request completed"
        );
        response
    }
    .instrument(span)
    .await
}
