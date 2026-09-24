# HTTP conventions

`src/lib.rs::app` wires routes and limits; feature handlers live in `src/notes/mod.rs`. Handlers decode input, call application logic (`create_note`), and map results. Validate before database work. Reject unknown JSON fields. Notes trim Unicode whitespace and accept 1–200 Unicode scalar values. Database checks provide a second boundary for the stored title.

All application errors, including route/method fallbacks, body limits, JSON/path rejections, database failure, and deadline expiration, are [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) JSON with `Content-Type: application/problem+json`. `type` is `about:blank`; `title` is the HTTP reason; `status` matches the response; `detail` is safe client-facing text. `request_id` is a project extension matching the response `X-Request-ID`. No request URI is included as an `instance` because queries may contain secrets. Transport-level malformed HTTP is handled by Hyper and is outside this JSON contract.

Return `AppError` for deliberate public error details. Its `IntoResponse` attaches typed metadata; `request_context` calls `error::normalize` to render the problem once the request ID is known. Mount routes through `app` so this middleware always runs. Other error responses receive the generic detail `Request failed`; their bodies are never inspected or exposed. Normalization preserves response headers such as CORS, cookies, Allow, Retry-After and WWW-Authenticate, while replacing stale body metadata. Avoid serializing provider errors into client responses.

Validation failures return HTTP 422 with an `issues` array inspired by [Zod's structured errors](https://zod.dev/error-customization). Every issue has a stable `code`, a `path` array, and an application-authored `message`. The existing problem fields and request ID remain present. For example, posting `{"title":" "}` returns:

```json
{
  "type": "about:blank",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "The given data was invalid.",
  "request_id": "01998c9e-8000-7000-8000-000000000001",
  "issues": [
    { "code": "too_small", "path": ["title"], "message": "The title field is required." }
  ]
}
```

Use `ValidationErrors::add(path, code, message)` to collect failures and `finish()?` after evaluating the fields. Multiple issues for a field and multiple failing fields are supported, in insertion order. Paths contain string field names and integer array indexes: `["items", 0, "name"]`. An empty path `[]` identifies the whole request. String keys containing dots or numeric strings remain literal keys. For example:

```rust
errors.add(["items".into(), 0usize.into(), "name".into()], IssueCode::TooSmall, "Name is required.");
errors.finish()?;
```

The supported codes are `invalid_type` (including missing/null required values), `too_small`, `too_big`, `unrecognized_keys`, and `custom`. Clients should use codes and paths for behavior and messages for display. Keep messages application-authored: never include submitted values, arbitrary unknown field names, or raw deserializer/database errors. `AppError::new(status, detail)` handles non-validation failures, which omit `issues`.

`NoteInput` is the HTTP decoding form; it collects missing/null title, wrong type, trimmed-empty/overlong title and unknown-field failures before constructing the typed `CreateNote`. A title error and unknown fields are reported together. Unknown fields use `unrecognized_keys` at `[]`; non-object input and duplicate known fields share a safe `custom` issue at `[]`. `create_note` still validates typed input for callers outside HTTP. Malformed JSON remains 400, unsupported content type 415, and an oversized body 413.

This replaces the earlier Laravel-style `errors` map: consumers must read `issues`. It implements Zod-style issue reporting without an additional dependency, not the complete Zod schema API or issue metadata (such as `expected`, `minimum` or submitted key names). Nested paths are supported by the error format; nested validation rules still belong to each feature.

Every request gets a new server-generated UUID v7. The middleware ignores inbound correlation IDs to prevent untrusted strings entering logs. Structured JSON logs contain only matched route template, generated request ID, status, and duration. Neither raw URL/query nor headers nor bodies are logged. SQL logging is off. Never log `Config`, database errors, auth tokens, or provider response bodies. Add stable error categories and a correlation ID when diagnosing failures. Unknown route logs say `unmatched`.

Defaults: 16 KiB JSON body, 10-second handler/body-read deadline, database pool acquisition/connect 3 seconds, readiness query 2 seconds. Config bounds are enforced by `Config::from_lookup`. The current body limit covers JSON extractors; when adding raw-body/multipart routes install an explicit streaming byte limit too. Handler timeouts cancel the Rust future, but a database write may already have committed: clients must not blindly retry writes. Add idempotency keys when retryable create APIs are required. No outbound HTTP client exists by default; follow [outbound HTTP](features/communication.md) when adding one. Reverse proxies must enforce connection, header-read, idle, and request-count limits before public exposure.

`GET /healthz` returns 200 while the HTTP process can serve requests; it does not touch PostgreSQL. `GET /readyz` checks a query against `notes`, so a missing initial migration or unavailable database produces 503. A deliberately shorter global deadline can yield 408 first. Neither endpoint exposes connection details. During shutdown the server stops accepting connections and drains existing ones; deployment should stop routing before SIGTERM and allow at least 20 seconds (longer if changing limits).

The teaching API is `POST /example/notes` (201) and `GET /example/notes/{id}` (200/404). It is absent unless explicitly enabled. It has no authentication or ownership checks. There is no CORS policy until a client model requires one.

`ApiDoc` and handler annotations generate [openapi.json](openapi.json). This artifact describes the opt-in example as well as health routes; runtime does not serve docs. Change DTO schemas and response annotations together. Run `bash scripts/openapi.sh write`, review the diff, then `bash scripts/check.sh`. `tests/http.rs` asserts representative status, content-type, UUID, fields, and schema references at runtime. When adding a response or extractor, add a contract assertion for it. Problem details omit internal exception text.
