# Data access extensions

Documentation-only guidance, authored 2026-09-25 against SeaORM 2.0.3 and PostgreSQL 18. The core already verifies migrations/persistence; the extensions here are not implemented. Sources: [SeaORM queries](https://www.sea-ql.org/SeaORM/docs/basic-crud/select/), [transactions](https://www.sea-ql.org/SeaORM/docs/advanced-query/transaction/), [PostgreSQL text search](https://www.postgresql.org/docs/18/textsearch.html).

## Pagination and filtering

Extend `src/notes/mod.rs` with a typed `ListNotes` query and a list handler in `src/lib.rs::app`. Use an explicit field/operator allowlist, a maximum page size of 100, and deterministic ordering. Prefer keyset pagination over `(created_at, id)` when a creation timestamp is introduced by a new migration. New IDs use UUID v7, so ordering by ID follows embedded generation time approximately, not transaction commit order; mixed legacy UUID v4 rows do not sort chronologically. Use an explicit timestamp when chronology is part of the API contract. Return an opaque encoded cursor whose decoded type/length is validated; scope the cursor and query to the user's tenant. Never interpolate raw sort/filter SQL. Add a matching index in `src/migration.rs` after examining `EXPLAIN`.

Add tests to `tests/http.rs` or a new `tests/data.rs`: empty result, page boundary, no duplicate/omitted rows under a fixed snapshot, invalid cursor, out-of-range limit, concurrent insertion semantics, and tenant filtering. Add DTOs/parameters to `ApiDoc`, regenerate OpenAPI and run both quality scripts. Offset pagination is acceptable for small admin lists if its count cost and concurrent-change behavior are documented.

## Transactions and fixtures

Use a SeaORM transaction in the concrete application function for multi-row changes; pass the transaction to entity operations. Keep external calls outside transactions. Verify rollback by deliberately violating a constraint on a later write and asserting the earlier write is absent. For concurrent edits, add a version column and conditional update to reject stale versions with a safe 409; test concurrent writers.

Create `tests/support/mod.rs` for reusable fixture builders when a second feature needs them. Reuse isolated schema setup from `tests/http.rs` and deterministic minimal records; no SQLite substitute. An optional `src/bin/seed.rs` can create clearly named demonstration data via an explicit command and idempotent keys. Require an explicit local seed flag; production startup never seeds, truncates, or recreates the database. Test running the seed twice.

## Search

Start with PostgreSQL full-text search if ranking/tokenized text is sufficient. Add a `tsvector` column or expression index with an explicit language configuration and a GIN index in a new migration. Query with parameterized `websearch_to_tsquery`, combine tenant/ownership filtering before returning results, and bound query length and page size. Test language/stemming, punctuation, Unicode, empty query, ranking ties, authorization and query-plan/index use on representative data. External search engines are a decision only after requirements for typo tolerance, facets, scale or language support justify synchronization/outbox work.
