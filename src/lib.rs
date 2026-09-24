pub mod config;
pub mod db;
pub mod error;
pub mod migration;
pub mod notes;

use axum::{
    Json, Router,
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
        let response = match tokio::time::timeout(timeout, next.run(request)).await {
            Ok(response) => response,
            Err(_) => {
                AppError(StatusCode::REQUEST_TIMEOUT, "Request deadline exceeded").into_response()
            }
        };
        let mut response = error::normalize(response, &request_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn errors_preserve_headers_and_only_expose_explicit_public_details() {
        let router = Router::new()
            .route(
                "/known",
                get(|| async {
                    (
                        [
                            ("retry-after", "30"),
                            ("www-authenticate", "Bearer"),
                            ("access-control-allow-origin", "https://client.example"),
                            ("set-cookie", "session=; Max-Age=0"),
                        ],
                        AppError(StatusCode::TOO_MANY_REQUESTS, "Try later"),
                    )
                }),
            )
            .route(
                "/unknown",
                get(|| async {
                    (
                        StatusCode::BAD_GATEWAY,
                        [
                            ("content-length", "999"),
                            ("content-encoding", "gzip"),
                            ("etag", "old-body"),
                        ],
                        Json(json!({"detail": "provider-secret"})),
                    )
                }),
            )
            .route("/success", get(|| async { "unchanged" }))
            .layer(middleware::from_fn_with_state(
                Duration::from_secs(1),
                request_context,
            ));

        for (path, expected_status, expected_detail) in [
            ("/known", StatusCode::TOO_MANY_REQUESTS, "Try later"),
            ("/unknown", StatusCode::BAD_GATEWAY, "Request failed"),
        ] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status);
            let headers = response.headers().clone();
            assert_eq!(headers["content-type"], "application/problem+json");
            assert!(!headers.contains_key("content-encoding"));
            assert!(!headers.contains_key("etag"));
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            if let Some(length) = headers.get("content-length") {
                assert_eq!(
                    length.to_str().unwrap().parse::<usize>().unwrap(),
                    bytes.len()
                );
            }
            let problem: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(problem["detail"], expected_detail);
            assert_eq!(
                problem["request_id"],
                headers["x-request-id"].to_str().unwrap()
            );
            assert!(!problem.to_string().contains("provider-secret"));
            if path == "/known" {
                assert_eq!(headers["retry-after"], "30");
                assert_eq!(headers["www-authenticate"], "Bearer");
                assert_eq!(
                    headers["access-control-allow-origin"],
                    "https://client.example"
                );
                assert_eq!(headers["set-cookie"], "session=; Max-Age=0");
            }
        }
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/success")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            "unchanged"
        );
    }
}
