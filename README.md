# Rust backend starter

A small Axum + SeaORM + PostgreSQL application, plus a [capability cookbook](docs/features/index.md) for adding features later. The only runtime dependency is PostgreSQL. Authentication, Redis, email, jobs, and cloud services are recipe-only.

## Run locally

Prerequisites: Rust via rustup (the repository installs/selects 1.98.1), a C compiler/linker, Bash, diff, and Docker with Compose. Linux is the supported build/deployment platform. On Windows use WSL2 Ubuntu with Rust/build-essential installed and Docker Desktop's WSL integration enabled. Native Windows builds are not in CI. The email exercise additionally uses Python 3 and curl.

```bash
cp .env.example .env
bash scripts/dev.sh up
bash scripts/dev.sh migrate
bash scripts/dev.sh run
```

The example environment deliberately opts into unauthenticated example routes on loopback. Without `ENABLE_EXAMPLE=true`, those routes return 404. Keep them off on a public deployment until you implement access control. `.env` is sourced as shell configuration by the development script; the binary itself only reads process environment. Only source trusted local files.

In another terminal:

```bash
curl -i http://127.0.0.1:3000/healthz
curl -i http://127.0.0.1:3000/readyz
curl -i -H 'Content-Type: application/json' \
  -d '{"title":"My first note"}' http://127.0.0.1:3000/example/notes
# Copy the returned id:
curl http://127.0.0.1:3000/example/notes/REPLACE_WITH_ID
```

`healthz` checks the process; `readyz` queries the migrated database. Ctrl-C drains in-flight requests. `bash scripts/dev.sh down` stops dependencies and retains the database volume.

## Check your changes

```bash
cargo install cargo-deny --version 0.20.2 --locked
export TEST_DATABASE_URL=postgres://starter:starter@127.0.0.1:5432/starter
bash scripts/check.sh
bash scripts/container-smoke.sh
```

Database tests create random isolated schemas and remove them after success; use a disposable database. Checks fail if the database is absent. CI runs these same scripts. [Quality policy](docs/quality.md) explains the supported configurations and required hosting settings. [Verification record](docs/verification.md) distinguishes observed results from instructions.

## Make this your project

1. Copy or use this repository as a template. Keep `Cargo.lock`, `rust-toolchain.toml`, and `STARTER_VERSION`. Initialize a new Git repository if needed. Change the package name in `Cargo.toml`; update the Rust crate imports in `src/main.rs`, `src/bin/openapi.rs`, and `tests/http.rs`, plus binary/image names in scripts and Dockerfile. Run `cargo check` to refresh the lockfile package entry, then run the checks.
2. Replace `src/notes/`, its routes in `src/lib.rs`, tests, and the OpenAPI annotations with your first real feature. For a fresh, unused database, replace the initial migration. For any database already in use, add a new migration instead of editing history. Also change the readiness query to a table your application requires. Remove `ENABLE_EXAMPLE` once the teaching routes are gone.
3. Read `AGENTS.md`, the [catalog](docs/features/index.md), and the relevant recipe. Ask, for example: **“Add durable email notifications for newly created notes using `docs/features/email.md` and `docs/features/jobs.md`. Store the note and delivery intent atomically, test retries and duplicate delivery, and update the catalog with actual results.”**
4. Record the source URL and source commit in `STARTER_VERSION` when adopting the starter. Tag your initial import. Keep an optional `starter` Git remote and review future upstream diffs; cherry-pick reviewed changes rather than overwriting your application. Re-run all checks after dependency, migration, or recipe updates.

The [architecture decision](docs/adr/0001-stack.md) explains why this uses Axum rather than Loco. [HTTP](docs/http.md), [database](docs/database.md), and [operations](docs/operations.md) describe the core. The starter is a teaching base, not an access-control implementation or a deployment to a hosted environment.
