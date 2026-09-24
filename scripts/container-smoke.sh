#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
d=${DOCKER_BIN:-docker}
name="starter-smoke-$$"
cleanup() {
  "$d" rm -fv "$name-app" "$name-db" >/dev/null 2>&1 || true
  "$d" network rm "$name" >/dev/null 2>&1 || true
}
trap cleanup EXIT
"$d" build -t rust-starterkit:smoke .
"$d" network create "$name" >/dev/null
"$d" run -d --name "$name-db" --network "$name" -e POSTGRES_USER=starter -e POSTGRES_PASSWORD=starter -e POSTGRES_DB=starter postgres:18.6-bookworm >/dev/null
for _ in {1..60}; do
  if "$d" exec "$name-db" pg_isready -U starter -d starter >/dev/null 2>&1; then break; fi
  sleep 1
done
"$d" exec "$name-db" pg_isready -U starter -d starter
db="postgres://starter:starter@$name-db:5432/starter"
"$d" run --rm --network "$name" -e DATABASE_URL="$db" rust-starterkit:smoke migrate
"$d" run --rm --network "$name" -e DATABASE_URL="$db" rust-starterkit:smoke migrate
"$d" run -d --name "$name-app" --network "$name" --read-only --cap-drop ALL --security-opt no-new-privileges -e DATABASE_URL="$db" rust-starterkit:smoke >/dev/null
"$d" run --rm --network "$name" curlimages/curl:8.19.0 --fail --retry 20 --retry-connrefused --retry-delay 1 "http://$name-app:3000/readyz"
code=$("$d" run --rm --network "$name" curlimages/curl:8.19.0 -s -o /dev/null -w '%{http_code}' "http://$name-app:3000/example/notes")
[[ "$code" == 404 ]]
"$d" stop --time 20 "$name-app" >/dev/null
[[ $("$d" inspect --format '{{.State.ExitCode}}' "$name-app") == 0 ]]
"$d" rm "$name-app" >/dev/null
"$d" run -d --name "$name-app" --network "$name" --read-only --cap-drop ALL --security-opt no-new-privileges -e DATABASE_URL="$db" -e ENABLE_EXAMPLE=true rust-starterkit:smoke >/dev/null
"$d" run --rm --network "$name" curlimages/curl:8.19.0 --fail --retry 20 --retry-connrefused --retry-delay 1 "http://$name-app:3000/readyz"
created=$("$d" run --rm --network "$name" curlimages/curl:8.19.0 --fail -s -H 'Content-Type: application/json' -H 'Authorization: Bearer header-secret-sentinel' -d '{"title":"body-secret-sentinel"}' "http://$name-app:3000/example/notes?token=query-secret-sentinel")
id=$(printf '%s' "$created" | sed -n 's/.*"id":"\([a-f0-9-]*\)".*/\1/p')
[[ -n "$id" ]]
retrieved=$("$d" run --rm --network "$name" curlimages/curl:8.19.0 --fail -s "http://$name-app:3000/example/notes/$id")
[[ "$created" == "$retrieved" ]]
logs=$("$d" logs "$name-app" 2>&1)
[[ "$logs" == *'request_id'* ]]
[[ "$logs" != *'secret-sentinel'* ]]
[[ "$logs" != *'starter:starter'* ]]
"$d" stop --time 20 "$name-app" >/dev/null
[[ $("$d" inspect --format '{{.State.ExitCode}}' "$name-app") == 0 ]]
echo 'Container smoke passed: migrations, readiness, both example settings, persistence, log redaction, non-root/read-only runtime, SIGTERM.'
