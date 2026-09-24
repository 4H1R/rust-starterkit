# Caching and rate limiting

## When and choice

Add a cache only after measuring repeated expensive reads and choosing acceptable staleness. PostgreSQL is sufficient for the default example. For a single process recommend [Moka 0.12.16](https://docs.rs/moka/0.12.16/moka/) (`0.12`, `future`) and [Governor 0.10.4](https://docs.rs/governor/0.10.4/governor/) (`0.10`) for bounded local rate limiting. Versions checked 2026-09-25. **Documentation-only; neither integration compiled or exercised here.**

Multiple replicas require an explicit decision: local budgets multiply with replica count, and local invalidation does not propagate. Use a shared Redis service or an edge gateway for globally enforced quotas and coordinated cache state only when those semantics are needed. That introduces infrastructure, failure policy, TLS/auth and additional tests; do not quietly install Redis for this starter.

## Prerequisites and edits

Measure the target query. Decide cache capacity/TTL, authorization key space, negative caching, quota scope, burst and sustained limits, and fail-open/fail-closed behavior. Read [identity](identity.md) before caching private notes. Edit `Cargo.toml`, `src/config.rs`, `src/lib.rs::AppState`/`app`, and `src/notes/mod.rs::get` and mutation functions. Create `src/cache.rs`, `src/rate_limit.rs`, `tests/performance.rs`. Initialize shared handles once in `src/main.rs::run`, never per request.

## Implement

1. Add Moka with a configured capacity (e.g. 1,000 entries) and TTL (e.g. 30 seconds). Define a key including tenant/user authorization scope, note ID, and representation version. Prefer caching the authorized query result; never return an unscoped cache hit before checking access. Cache successful reads only initially; bound item size as well as count.
2. In the note read application function, use Moka's async loading support to coalesce concurrent misses. Populate after a successful database result; invalidate after committed updates/deletes. A failed commit must not invalidate as if it succeeded. Document that TTL bounds staleness and does not provide immediate revocation: sensitive authorization changes must bypass/evict relevant caches.
3. Add Governor keyed by authenticated principal or a deliberately chosen anonymous source. For unauthenticated traffic use socket peer IP or an edge-generated identity; trust forwarded headers only from configured proxies. Choose limits such as 60 reads/minute with burst 10 and stricter writes. Integrate as Axum middleware at the relevant route group so errors still pass through `request_context`.
4. Return 429 problem details and a valid `Retry-After` header. Preserve it through error normalization (already supported). Bound and periodically evict idle limiter keys; arbitrary IP/key cardinality must not make memory unbounded. Exempt health probes. Apply a global concurrency bound separately if resource protection needs it; rate is not concurrency.
5. For a shared backend, add explicit timeouts and a named outage policy. A security quota should generally fail closed or fall back to a conservative local allowance, while a cache may fall back to PostgreSQL. Prevent outage-induced stampedes with concurrency bounds. Test the selected behavior rather than relying on client-library defaults.

## Failures, tests, and operations

After creating tests run `cargo test --locked --test performance`, then both quality scripts. Test hit/miss/invalidation, TTL using a controlled clock where supported, bounded memory/key cleanup, concurrent miss coalescing, cross-tenant keys, authorization revocation, burst/refill, spoofed forwarded headers, 429 problem content/Retry-After, health exemption, and shared backend outage if selected. Add multi-replica tests before claiming a global quota. Benchmark against uncached reads so a cache does not disguise a missing index.

Monitor hit rate, evictions, load latency, limiter denials, backend failures, and database load after fallback; omit user IDs/IPs from metric labels. Deploy total capacity and budget proportional to replicas, or use a shared limiter. Rollback can disable the cache safely if the database is sized for the load; changing quota config may cause an immediate burst, so document the transition. Optional backend readiness should reflect the chosen failure policy, not automatically take the whole web service down.
