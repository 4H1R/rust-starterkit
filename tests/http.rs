use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rust_starterkit::{AppState, app, config::Config, db, migration::Migrator};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: &str,
    content_type: &str,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", content_type)
                .header("x-request-id", "untrusted-secret")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    uuid::Uuid::parse_str(headers["x-request-id"].to_str().unwrap()).unwrap();
    if status.is_client_error() || status.is_server_error() {
        assert_eq!(headers["content-type"], "application/problem+json");
        assert_eq!(value["status"], status.as_u16());
        assert_eq!(value["type"], "about:blank");
        assert_eq!(
            value["request_id"],
            headers["x-request-id"].to_str().unwrap()
        );
        assert!(!value.to_string().contains("untrusted-secret"));
    }
    (status, headers, value)
}

struct TestDb {
    admin: DatabaseConnection,
    db: DatabaseConnection,
    schema: String,
    config: Config,
}
impl TestDb {
    async fn new() -> Self {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is mandatory; tests never silently skip PostgreSQL");
        let config = Config::from_lookup(|key| match key {
            "DATABASE_URL" => Some(url.clone()),
            "ENABLE_EXAMPLE" => Some("true".into()),
            _ => None,
        })
        .unwrap();
        let admin = db::connect(&config).await.unwrap();
        let schema = format!("test_{}", uuid::Uuid::new_v4().simple());
        admin
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut options = ConnectOptions::new(url);
        options
            .set_schema_search_path(schema.clone())
            .sqlx_logging(false)
            .max_connections(3);
        let db = Database::connect(options).await.unwrap();
        Self {
            admin,
            db,
            schema,
            config,
        }
    }
    async fn cleanup(self) {
        self.db.close().await.unwrap();
        self.admin
            .execute_unprepared(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .await
            .unwrap();
        self.admin.close().await.unwrap();
    }
}

#[tokio::test]
async fn postgres_http_contract_and_migration_lifecycle() {
    let fixture = TestDb::new().await;
    let router = app(
        AppState {
            db: fixture.db.clone(),
        },
        &fixture.config,
    );
    assert_eq!(
        request(&router, "GET", "/readyz", "", "application/json")
            .await
            .0,
        503
    );
    Migrator::up(&fixture.db, None).await.unwrap();
    Migrator::up(&fixture.db, None).await.unwrap();
    assert_eq!(
        request(&router, "GET", "/healthz", "", "application/json")
            .await
            .0,
        200
    );
    assert_eq!(
        request(&router, "GET", "/readyz", "", "application/json")
            .await
            .0,
        200
    );
    let (status, _, note) = request(
        &router,
        "POST",
        "/example/notes",
        r#"{"title":"  hello  "}"#,
        "application/json",
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(note["title"], "hello");
    uuid::Uuid::parse_str(note["id"].as_str().unwrap()).unwrap();
    let path = format!("/example/notes/{}", note["id"].as_str().unwrap());
    assert_eq!(
        request(&router, "GET", &path, "", "application/json")
            .await
            .2,
        note
    );
    use sea_orm::EntityTrait;
    let persisted = rust_starterkit::notes::entity::Entity::find_by_id(
        uuid::Uuid::parse_str(note["id"].as_str().unwrap()).unwrap(),
    )
    .one(&fixture.db)
    .await
    .unwrap()
    .unwrap();
    assert_eq!(persisted.title, "hello");
    for (body, content_type, expected) in [
        (r#"{"title":" "}"#, "application/json", 422),
        ("{", "application/json", 400),
        (r#"{"title":12}"#, "application/json", 422),
        (r#"{"title":"hi","unknown":true}"#, "application/json", 422),
        (r#"{"title":"hi"}"#, "text/plain", 415),
    ] {
        assert_eq!(
            request(&router, "POST", "/example/notes", body, content_type)
                .await
                .0,
            expected
        );
    }
    assert_eq!(
        request(
            &router,
            "POST",
            "/example/notes",
            &json!({"title":"x".repeat(20000)}).to_string(),
            "application/json"
        )
        .await
        .0,
        413
    );
    assert_eq!(
        request(
            &router,
            "GET",
            "/example/notes/not-a-uuid",
            "",
            "application/json"
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &router,
            "GET",
            &format!("/example/notes/{}", uuid::Uuid::new_v4()),
            "",
            "application/json"
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &router,
            "GET",
            "/missing?secret=hidden",
            "",
            "application/json"
        )
        .await
        .0,
        404
    );
    let (status, headers, _) = request(&router, "DELETE", &path, "", "application/json").await;
    assert_eq!(status, 405);
    assert!(headers.contains_key("allow"));
    let mut disabled = fixture.config.clone();
    disabled.enable_example = false;
    let disabled = app(
        AppState {
            db: fixture.db.clone(),
        },
        &disabled,
    );
    assert_eq!(
        request(
            &disabled,
            "POST",
            "/example/notes",
            "{}",
            "application/json"
        )
        .await
        .0,
        404
    );
    let spec: Value = serde_json::from_str(include_str!("../docs/openapi.json")).unwrap();
    assert_eq!(
        spec["paths"]["/example/notes"]["post"]["responses"]["201"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/Note"
    );
    for field in spec["components"]["schemas"]["Note"]["required"]
        .as_array()
        .unwrap()
    {
        assert!(note.get(field.as_str().unwrap()).is_some());
    }
    Migrator::down(&fixture.db, Some(1)).await.unwrap();
    assert_eq!(
        request(&router, "GET", &path, "", "application/json")
            .await
            .0,
        503
    );
    Migrator::up(&fixture.db, None).await.unwrap();
    assert_eq!(
        request(&router, "GET", &path, "", "application/json")
            .await
            .0,
        404
    );
    fixture.cleanup().await;
}

#[tokio::test]
async fn deadline_and_unavailable_database() {
    let mut fixture = TestDb::new().await;
    Migrator::up(&fixture.db, None).await.unwrap();
    fixture.config.request_timeout = std::time::Duration::from_millis(50);
    use sea_orm::TransactionTrait;
    let lock = fixture.db.begin().await.unwrap();
    lock.execute_unprepared("LOCK TABLE notes IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let router = app(
        AppState {
            db: fixture.db.clone(),
        },
        &fixture.config,
    );
    assert_eq!(
        request(
            &router,
            "POST",
            "/example/notes",
            r#"{"title":"blocked"}"#,
            "application/json"
        )
        .await
        .0,
        408
    );
    lock.rollback().await.unwrap();
    fixture.db.clone().close().await.unwrap();
    assert_eq!(
        request(&router, "GET", "/healthz", "", "application/json")
            .await
            .0,
        200
    );
    assert_eq!(
        request(&router, "GET", "/readyz", "", "application/json")
            .await
            .0,
        503
    );
    fixture.cleanup().await;
}
