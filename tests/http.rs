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
    let request_id = uuid::Uuid::parse_str(headers["x-request-id"].to_str().unwrap()).unwrap();
    assert_eq!(request_id.get_version_num(), 7);
    if status.is_client_error() || status.is_server_error() {
        assert_eq!(headers["content-type"], "application/problem+json");
        assert_eq!(value["status"], status.as_u16());
        assert_eq!(value["type"], "about:blank");
        assert_eq!(
            value["request_id"],
            headers["x-request-id"].to_str().unwrap()
        );
        assert!(!value.to_string().contains("untrusted-secret"));
        if status != StatusCode::UNPROCESSABLE_ENTITY {
            assert!(value.get("issues").is_none());
        }
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
        let schema = format!("test_{}", uuid::Uuid::now_v7().simple());
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
    let note_id = uuid::Uuid::parse_str(note["id"].as_str().unwrap()).unwrap();
    assert_eq!(note_id.get_version_num(), 7);
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
    let legacy_id = uuid::Uuid::parse_str("95a73fe1-616e-4de6-b21e-74f8d9dfe638").unwrap();
    use sea_orm::{ActiveModelTrait, Set};
    rust_starterkit::notes::entity::ActiveModel {
        id: Set(legacy_id),
        title: Set("existing v4 note".into()),
    }
    .insert(&fixture.db)
    .await
    .unwrap();
    let (status, _, legacy_note) = request(
        &router,
        "GET",
        &format!("/example/notes/{legacy_id}"),
        "",
        "application/json",
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(legacy_note["id"], legacy_id.to_string());
    assert_eq!(legacy_note["title"], "existing v4 note");
    for (body, expected_issues) in [
        (
            "{}".to_owned(),
            json!([{"code": "invalid_type", "path": ["title"], "message": "The title field is required."}]),
        ),
        (
            r#"{"title":null}"#.to_owned(),
            json!([{"code": "invalid_type", "path": ["title"], "message": "The title field is required."}]),
        ),
        (
            r#"{"title":" \n "}"#.to_owned(),
            json!([{"code": "too_small", "path": ["title"], "message": "The title field is required."}]),
        ),
        (
            r#"{"title":12}"#.to_owned(),
            json!([{"code": "invalid_type", "path": ["title"], "message": "The title must be a string."}]),
        ),
        (
            r#"{"title":false}"#.to_owned(),
            json!([{"code": "invalid_type", "path": ["title"], "message": "The title must be a string."}]),
        ),
        (
            r#"{"title":[]}"#.to_owned(),
            json!([{"code": "invalid_type", "path": ["title"], "message": "The title must be a string."}]),
        ),
        (
            r#"{"title":{"secret":"sensitive-input"}}"#.to_owned(),
            json!([{"code": "invalid_type", "path": ["title"], "message": "The title must be a string."}]),
        ),
        (
            json!({"title": "é".repeat(201)}).to_string(),
            json!([{"code": "too_big", "path": ["title"], "message": "The title must not be greater than 200 characters."}]),
        ),
        (
            r#"{"title":"", "sensitive-input":"secret"}"#.to_owned(),
            json!([{"code": "unrecognized_keys", "path": [], "message": "Unknown fields are not allowed."}, {"code": "too_small", "path": ["title"], "message": "The title field is required."}]),
        ),
        (
            r#"{"title":"ok", "sensitive-input":"secret"}"#.to_owned(),
            json!([{"code": "unrecognized_keys", "path": [], "message": "Unknown fields are not allowed."}]),
        ),
        (
            r#"{"title":"first","title":"second"}"#.to_owned(),
            json!([{"code": "custom", "path": [], "message": "Expected an object with no duplicate fields."}]),
        ),
        (
            "[]".to_owned(),
            json!([{"code": "custom", "path": [], "message": "Expected an object with no duplicate fields."}]),
        ),
        (
            "null".to_owned(),
            json!([{"code": "custom", "path": [], "message": "Expected an object with no duplicate fields."}]),
        ),
    ] {
        let (status, _, problem) =
            request(&router, "POST", "/example/notes", &body, "application/json").await;
        assert_eq!(status, 422, "body: {body}");
        assert_eq!(problem["detail"], "The given data was invalid.");
        assert_eq!(problem["issues"], expected_issues, "body: {body}");
        assert!(problem.get("errors").is_none());
        assert!(!problem.to_string().contains("sensitive-input"));
    }
    use sea_orm::PaginatorTrait;
    assert_eq!(
        rust_starterkit::notes::entity::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        2
    );
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
            &format!("/example/notes/{}", uuid::Uuid::now_v7()),
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
        spec["components"]["schemas"]["Problem"]["properties"]["issues"]["items"]["$ref"],
        "#/components/schemas/ValidationIssue"
    );
    assert_eq!(
        spec["components"]["schemas"]["PathSegment"]["oneOf"][0]["type"],
        "string"
    );
    assert_eq!(
        spec["components"]["schemas"]["PathSegment"]["oneOf"][1]["type"],
        "integer"
    );
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

#[tokio::test]
async fn note_creation_participates_in_the_callers_transaction() {
    use rust_starterkit::notes::{CreateNote, create_note, entity};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};

    let fixture = TestDb::new().await;
    Migrator::up(&fixture.db, None).await.unwrap();

    let transaction = fixture.db.begin().await.unwrap();
    let committed = create_note(
        &transaction,
        CreateNote {
            title: "committed".into(),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("create note in transaction"));
    assert!(
        entity::Entity::find_by_id(committed.id)
            .one(&fixture.db)
            .await
            .unwrap()
            .is_none()
    );
    transaction.commit().await.unwrap();
    assert_eq!(
        entity::Entity::find_by_id(committed.id)
            .one(&fixture.db)
            .await
            .unwrap()
            .unwrap()
            .title,
        committed.title
    );

    let transaction = fixture.db.begin().await.unwrap();
    let rolled_back = create_note(
        &transaction,
        CreateNote {
            title: "rolled back".into(),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("create note in transaction"));
    let duplicate = entity::ActiveModel {
        id: Set(rolled_back.id),
        title: Set("conflicting second write".into()),
    };
    assert!(duplicate.insert(&transaction).await.is_err());
    transaction.rollback().await.unwrap();
    assert!(
        entity::Entity::find_by_id(rolled_back.id)
            .one(&fixture.db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        entity::Entity::find_by_id(committed.id)
            .one(&fixture.db)
            .await
            .unwrap()
            .is_some()
    );
    fixture.cleanup().await;
}
