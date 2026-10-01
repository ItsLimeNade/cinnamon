# Changelog

## Unreleased

### Fixed
- A sync poll that failed part-way still advanced the cursor for the collections before
  the failure, so retrying lost their events. A failed poll now changes nothing.
- A sync could stop advancing when more than one page of documents fell inside its
  ten-minute overlap window.
- API v1 lists with a `limit` above 1000 returned only 1000 documents. Lists above one page
  now page by date (like `stream`) on both APIs instead of by `skip`.
- `stream()` re-applied `skip` on every page, dropping documents at each page boundary.
- A page of documents that did not decode ended a stream early; pagination now follows the
  raw documents on the field the query sorts on.
- A network drop while a realtime subscription reconnected was reported as `Unauthorized`
  and stopped the subscription for good. Only an explicit refusal stops it now.
- An `/alarm` event whose payload did not decode was silently dropped. It is now delivered
  with the event's own severity, and alarm levels accept numeric strings.
- JWT expiry was judged with the local wall clock, so clock skew could keep an expired token
  or force an exchange on every request. It now uses the token's lifetime.

### Changed
- Uploads default `device` to the client's app name (as documented), which also feeds the
  API v3 identifier.

### Security
- Transport errors never include the request URL, which can contain the access token.

## 2.0.0-alpha.1 (unreleased)

This is a rewrite. See [MIGRATION.md](MIGRATION.md).

### Added
- API v3 support with automatic v1/v2 fallback, and a `ApiVersion` preference to override it.
- Access-token authentication, exchanged for JWTs that are refreshed before they expire.
  The client checks credentials up front with `Client::connect`.
- Every collection: entries (`sgv`, `mbg`, `cal`), treatments, devicestatus, profile, food,
  settings (v3) and activity (v1).
- Full CRUD on collections: `get`, `create`/`create_many` (with deterministic identifiers,
  so retries deduplicate), `update`, `patch`, soft and permanent `delete`, `delete_many`
  and `history`.
- Typed filters (`query::Filter`), date-keyed streaming pagination (`List::stream`), and
  raw JSON with field projection.
- Endpoints: properties for every plugin (typed), ddata, summary, status, versions,
  verifyauth, adminnotifies, v3 status/lastModified, and notification ack/Loop push.
  Admin routes: subjects, roles, permissions and permission checks. Entries helpers:
  current, times, slice, count and echo.
- Incremental sync (`Client::sync`) with a persistable cursor and a fallback for legacy
  uploads.
- Realtime streams (feature `realtime`) over a minimal in-crate socket.io client:
  `/storage`, `/alarm` (with ack), `dataUpdate` and live glucose.
- Blocking wrapper (feature `blocking`), `tracing` instrumentation, and a prelude.
- An e2e test suite against a real Nightscout (Docker in CI).

### Fixed
- Loop device status was dropped: it was read from `loop_` instead of `loop`.
- Get-by-id and delete-by-id used routes that don't exist (`/entries/sgv.json/{id}`).
- Bulk deletes ignored failures and reported success.
- Device filtering on treatments compared the wrong field.
- Trends `NONE`, `NOT COMPUTABLE`, `RATE OUT OF RANGE`, `TripleUp` and `TripleDown` were lost
  or rewritten as `"Else"`.
- One unusual field (a string number, a `null`, a missing optional) failed a whole
  response.
- API v1 lists were silently capped at four days.
- `properties().at()` sent a parameter Nightscout ignores.

### Security
- Credentials are never sent over plain HTTP to remote hosts, and redirects are never
  followed. reqwest forwards custom headers such as `api-secret` across redirects.
- Secrets live in `secrecy` types with redacted `Debug`. No public fields expose them.
- Error bodies are truncated. A 401 is never retried in a loop, which would feed
  Nightscout's per-IP delay.

### Changed
- Timeouts by default (10 s connect, 30 s total), retries with backoff for idempotent
  requests, gzip/brotli, HTTP/2, and a `User-Agent`.
- `tokio` is no longer pulled in with `full`, and reqwest types no longer leak into the
  public API.
- MSRV 1.85, edition 2024.
