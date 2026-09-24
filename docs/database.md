# Database conventions

`src/db.rs::connect` creates one SeaORM pool per process. `AppState` clones its shared handle. Feature entities stay beside application logic (`src/notes/entity.rs`). Use parameterized SeaORM queries; identifiers interpolated in tests are generated UUID schema names, never user input. PostgreSQL is the only supported database.

`src/migration.rs::Migrator` is an ordered history. Give each added migration a unique timestamp name and append it. Run `bash scripts/dev.sh migrate` before serving. Startup never migrates automatically. Run one migrator per deployment, with a DDL-capable role; the serving role should have only the data permissions it needs. SeaORM's migration history records completion; it does not make multi-statement migrations automatically atomic. For a migration that needs multiple statements, use a transaction where PostgreSQL permits it. Plan nontransactional operations such as concurrent index creation separately.

New note IDs use UUID v7 via `Uuid::now_v7()`. PostgreSQL stores them in the existing `uuid` column; no schema migration or rewrite of existing IDs is needed. Reads continue to accept existing UUID v4 IDs. UUID v7 embeds a generation timestamp, not a transaction commit time.

The first migration demonstrates a primary key and storage constraint. Its `down` drops the example table and is exercised only on disposable test schemas. The binary deliberately exposes only forward migration. Do not run destructive rollback on production data; see [recovery](operations.md).

For atomic feature work, begin a SeaORM transaction in the application function and pass its connection to each query. `notes::create_note` accepts `&impl ConnectionTrait`, so callers can reuse it with either the pool or an existing transaction. The caller owns commit/rollback; the operation does not start a nested transaction. `note_creation_participates_in_the_callers_transaction` in `tests/http.rs` verifies visibility before/after commit and rollback after a second write fails. A note plus email intent must commit in the same transaction; sending SMTP inside a transaction does not make the external effect atomic. See [jobs/outbox](features/jobs.md).

`tests/http.rs::TestDb` requires `TEST_DATABASE_URL`, creates a unique schema with its own search path, runs the actual migrations twice, drives HTTP and directly checks the persisted row, then verifies down/up. Tests never silently skip PostgreSQL. A failing test can leave a `test_<uuid>` schema for diagnosis. Remove only that identified schema after inspecting it; use a dedicated disposable test database. Never set this variable to production. CI uses a fresh PostgreSQL service. Tests cover both example settings and pool unavailability.

Additional read APIs, seeds, search and transaction examples are in [data](features/data.md). Database operational procedures and backup drills are in [operations](operations.md).
