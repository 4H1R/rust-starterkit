# Verification record

Date: 2026-09-25 (Asia/Dubai). Executed locally in WSL2 Ubuntu x86_64 with Docker Desktop Linux containers. Rust 1.98.1, Axum 0.8.9, SeaORM/migration 2.0.3, utoipa 6.0.0, cargo-deny 0.20.2. The application dependency graph is locked in `Cargo.lock`; `cargo tree -e features` confirmed no active SQLite/MySQL driver, Redis, SMTP or cloud client in the core. PostgreSQL image: `postgres:18.6-bookworm`, server reported 18.6. PostgreSQL's [18.6 release notes](https://www.postgresql.org/docs/release/18.6/) were checked before finalizing the image; 18.5 was not released.

## Core commands and observed results

The following commands were executed from the repository with `CARGO_TARGET_DIR=/tmp/rust-starterkit-target` and `TEST_DATABASE_URL=postgres://starter:starter@127.0.0.1:5432/starter`. This test database contains local-only credentials; tests isolate data in random schemas.

| Command | Observed result |
| --- | --- |
| `docker compose up -d --wait` | PostgreSQL healthy; final image 18.6 |
| `cargo check --all-targets` | Passed during initial implementation |
| `cargo fmt --all -- --check` | Passed via shared check script |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked --all-targets` | Passed: 4 unit tests + 3 HTTP/PostgreSQL scenario tests; no ignored tests |
| `bash scripts/openapi.sh write` then `bash scripts/openapi.sh check` | Generated checked-in spec; drift check passed |
| `cargo deny --locked check` | Advisories, bans, licenses, sources passed; transitive duplicate-version warnings remain intentionally visible |
| `cargo build --locked --release --bin rust-starterkit` | Passed |
| `bash scripts/check.sh` | Passed as one complete workflow against PostgreSQL 18.6 |
| `bash scripts/container-smoke.sh` | Passed: Docker build, repeated migrations, readiness, both example settings, create/get persistence, sensitive-log sentinel checks, read-only/non-root runtime and clean SIGTERM |

HTTP/database scenarios cover schema not migrated, migration up twice, create/read and direct row inspection, trimming/Unicode validation, invalid JSON/types/unknown fields/content type, oversized body, malformed/missing UUID, 404/405 with Allow, problem media type/status/request ID, representative OpenAPI required fields, database lock deadline, closed pool, health independent of database, and migration down/up. Both enabled and disabled example modes are exercised. Each scenario asserts multiple behaviors; the test function count is not a count of all assertions.

The host Docker CLI is Windows-only in this setup, so Docker scripts used `DOCKER_BIN="/mnt/c/Program Files/Docker/Docker/resources/bin/docker.exe"` in WSL. Normal Linux or Docker Desktop with WSL integration uses the scripts' default `docker`. Initial Docker access needed local sandbox escalation, and Docker Desktop was started. No production environment or remote messaging service was contacted.

## Optional recipe trial

`bash scripts/exercise-email.sh` created a disposable application copy under `.scratch/`, added Lettre 0.11.23 and Askama 0.16.1 there, and used Mailpit 1.31.2 on loopback. The core `Cargo.toml`, `Cargo.lock`, `src/`, and startup configuration remain free of these integrations. The script was rerun after correcting an escaping assertion: Askama emits numeric HTML entities.

Passed in the copy: Clippy for all trial targets, both email tests (HTML escaping/Unicode, invalid recipient, SMTP capture and connection refused), and a Mailpit API assertion for exactly one message with expected recipient, subject, plain text and escaped HTML. No external account or real recipient was used. The capture container was removed on script exit; ignored copies remain for review. The local trial did not exercise a production SMTP TLS provider, provider feedback, durable delivery, or an email-enabled Docker image.

The email recipe records the additional template Docker context/copy steps needed on adoption. Authentication, jobs, scheduling, uploads, caching, metrics, realtime, backup/restore and provider integrations remain documentation-only guidance; they were not compiled or executed merely because email succeeded.

## Corrections and remaining limits

Zod-style validation update on 2026-09-25: replaced the `errors` map with ordered `issues`, each containing a typed code, a path of string fields/integer indexes, and a safe message. HTTP cases assert the new codes, root paths and absence of the old map; a new normalization test verifies nested/indexed paths, literal dotted/numeric keys, multiple issues for one field and merge order. Regenerated OpenAPI with the code enum and path union. All seven tests and the complete `scripts/check.sh` and `scripts/container-smoke.sh` passed. An initial test assumed `anyOf`; it was corrected to the generated `oneOf` schema before the successful run. No dependencies or database migrations changed.

Field-validation update on 2026-09-25: added Laravel-style field-to-message arrays to HTTP 422 problem responses and a reusable `ValidationErrors` collector. HTTP/PostgreSQL checks cover missing/null/blank/wrong-type/overlong title, unknown fields together with a title error, duplicate title, non-object input, redaction of submitted data, and no inserts for rejected requests. Non-validation failures omit `errors`. Regenerated OpenAPI and verified its field-message schema. All six tests and the full `scripts/check.sh` and `scripts/container-smoke.sh` passed; no dependencies or database migrations were added.

UUID v7 update on 2026-09-25: switched note IDs, request IDs and generated test fixtures to `Uuid::now_v7()`. Existing HTTP/PostgreSQL scenarios now assert version 7 for generated note and request IDs and verify a stored legacy v4 note remains readable. Both `scripts/check.sh` (all six tests, OpenAPI drift, dependency policy and release build) and `scripts/container-smoke.sh` passed. `cargo tree -e features -i uuid` confirmed v7 enabled with no v4 feature; `Cargo.lock`, database schema and generated OpenAPI were unchanged.

Zoora-informed extension on 2026-09-25: a read-only sub-agent comparison identified transaction composition, durable audit semantics and negative authorization/serialization tests; [the comparison record](zoora-review.md) contains source evidence and scope. `notes::create_note` now accepts SeaORM's `ConnectionTrait`. A new real-PostgreSQL test verified uncommitted isolation, commit visibility, and rollback after a conflicting second write. The full `scripts/check.sh` and `scripts/container-smoke.sh` passed again. All 20 local Markdown documents' file links resolved. Audit and authorization changes are recipe guidance only; Zoora's tests were not run and its files were not changed.

Refactor verification on 2026-09-25: repeated `scripts/check.sh` and `scripts/container-smoke.sh` successfully after replacing error-body JSON reparsing with typed `AppError` response metadata and one problem-details renderer. Added a routed regression test for header preservation (including CORS/cookies), safe fallback details, stale body-header removal, correlation IDs and unchanged success responses. OpenAPI output remained identical. Also executed the OpenAPI write path with a fake generator that emitted partial output and exited 42: the command failed and the original specification hash remained unchanged. Redundant comments were removed; dependencies and migrations were unchanged.

The first cargo-deny run rejected the Mozilla root certificate data license. The policy now explicitly allows `CDLA-Permissive-2.0` and passes. One base-image download ended with unexpected EOF; a retry succeeded. Initial tests used PostgreSQL 18.3, then the final image was updated to 18.6 and the full checks were repeated. These failures are resolved, not skipped checks.

No remaining local core verification blocker is known. GitHub CI is provided but has not run on a remote host; repository hosting/branch protection and review settings are instructions only. Native Windows and ARM are not tested. Public access control, backups, deployments, provider credentials and production hardening are future-project work. Container tags and advisory databases may change; run the scripts again when adopting or updating the starter.
