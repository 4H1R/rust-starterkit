# Capability catalog

Read the relevant recipe before adding dependencies. **Implemented** means present and exercised; **Recipe** means instructions exist, with the verification level below; **Decision required** means project constraints must select the approach first. Dates and results are in each recipe and [verification](../verification.md). Optional libraries are absent from the default manifest.

| Capability | Status / verification | Recommended approach | Prerequisites / recipe |
| --- | --- | --- | --- |
| HTTP, validation, JSON errors, OpenAPI | Implemented; field-error aggregation and runtime/contract tests | Axum, utoipa, problem details with field messages | [HTTP](../http.md) |
| PostgreSQL pool, versioned migrations, transactions | Implemented; pool/migrations/transaction composition tested | SeaORM concrete feature logic | [Database](../database.md), [data](data.md) |
| Logs, request IDs, health, readiness, shutdown, limits | Implemented; core and container checks | JSON tracing, explicit deadlines | [HTTP](../http.md), [operations](../operations.md) |
| Authentication, sessions/tokens, authorization | Recipe; documentation only | OIDC code + PKCE and PostgreSQL sessions for browser apps | Client model/issuer decision; [identity](identity.md) |
| Password reset, email verification | Recipe; documentation only | Identity provider's verified flows | Identity provider; [identity](identity.md) |
| Tenant isolation | Recipe; documentation only | Memberships and tenant-scoped queries | Identity + tenant model; [identity](identity.md) |
| Durable audit history | Recipe; documentation only | Commit success audit and mutation together; separately capture denials | Identity + retention policy; [identity](identity.md#durable-audit-history-when-required) |
| Durable jobs, retries, failed-job inspection/replay | Recipe; documentation only | PostgreSQL lease queue + separate worker | Delivery semantics; [jobs](jobs.md) |
| Events, outbox, webhooks | Recipe; documentation only | Atomic intent + at-least-once delivery | Jobs + signing/recipient policy; [jobs](jobs.md) |
| Scheduling | Recipe; documentation only | UTC schedules enqueue deduplicated jobs | Durable jobs; [scheduling](scheduling.md) |
| Email templates + local capture | Recipe; disposable integration executed | Askama + Lettre + Mailpit | Delivery requirement; [email](email.md) |
| Durable email, provider feedback, notifications | Recipe; documentation only | Jobs/outbox, authenticated feedback, preferences | Jobs + identity/provider choice; [email](email.md), [communication](communication.md) |
| Outbound HTTP | Recipe; documentation only | Shared reqwest client with bounded timeouts | Destination trust; [communication](communication.md) |
| Pagination, filtering, transactions, fixtures | Recipe; documentation only | SeaORM bounded queries + deterministic test fixtures | Relevant feature schema; [data](data.md) |
| Uploads/object storage | Recipe; documentation only | Private S3 objects + PostgreSQL metadata | Authorization + bucket; [uploads](uploads.md) |
| Search | Recipe; documentation only | PostgreSQL full-text search first | Search/language needs; [data](data.md) |
| Caching/rate limiting | Recipe; documentation only | Moka + Governor for one process | Staleness/replica/quota decisions; [performance](performance.md) |
| Streaming/SSE, WebSockets | Recipe; documentation only | Axum streams/socket handlers | Authorization, backpressure/reconnect model; [realtime](realtime.md) |
| Metrics/distributed tracing | Recipe; documentation only | OpenTelemetry to a collector | Telemetry backend; [observability](observability.md) |
| Container and CI | Implemented locally; remote hosting settings pending | Non-root image, shared check scripts | Docker/GitHub; [quality](../quality.md) |
| Deployment, secrets, backup/restore, migration recovery | Recipe; docs + container smoke only | Secret store, expand/contract, PostgreSQL PITR | Host, RPO/RTO; [operations](../operations.md) |
| Billing/domain integrations | Decision required | Select provider after money, tax, reconciliation, and webhook needs are known | [communication](communication.md); jobs + idempotency usually needed |

Dependency direction: identity → authorized uploads/private streams; jobs → scheduling and durable delivery; transaction + jobs → atomic outbox; email alone → synchronous local delivery only. None of these optional recipes is required for core startup.

When implementing a recipe, record the versions actually locked, new tests, real command output, and date. Promote only the implemented portion of a catalog row; one email trial does not validate other recipes or production email delivery.
