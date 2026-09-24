# Database conventions

`src/db.rs::connect` creates one SeaORM pool per process. `AppState` clones its shared handle. Feature entities stay beside application logic (`src/notes/entity.rs`). Use parameterized SeaORM queries; identifiers interpolated in tests are generated UUID schema names, never user input. PostgreSQL is the only supported database.

`src/migration.rs::Migrator` is an ordered history. Give each added migration a unique timestamp name and append it. Run `bash scripts/dev.sh migrate` before serving. Startup never migrates automatically. Run one migrator per deployment, with a DDL-capable role; the serving role should have only the data permissions it needs. SeaORM's migration history records completion; it does not make multi-statement migrations automatically atomic. For a migration that needs multiple statements, use a transaction where PostgreSQL permits it. Plan nontransactional operations such as concurrent index creation separately.

The first migration demonstrates a primary key and storage constraint. Its `down` drops the example table and is exercised only on disposable test schemas. The binary deliberately exposes only forward migration. Do not run destructive rollback on production data; see [recovery](operations.md).

For atomic feature work, begin a SeaORM transaction in the application function and pass its connection to each query. A note plus email intent must commit in the same transaction; sending SMTP inside a transaction does not make the external effect atomic. See [jobs/outbox](features/jobs.md).

`tests/http.rs::TestDb` requires `TEST_DATABASE_URL`, creates a unique schema with its own search path, runs the actual migrations twice, drives HTTP and directly checks the persisted row, then verifies down/up. Tests never silently skip PostgreSQL. A failing test can leave a `test_<uuid>` schema for diagnosis. Remove only that identified schema after inspecting it; use a dedicated disposable test database. Never set this variable to production. CI uses a fresh PostgreSQL service. Tests cover both example settings and pool unavailability.

Additional read APIs, seeds, search and transaction examples are in [data](features/data.md). Database operational procedures and backup drills are in [operations](operations.md).
