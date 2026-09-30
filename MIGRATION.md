# Migrating from cinnamon 1.x to 2.0

2.0 is a rewrite. The 1.x API hid several bugs:
- Deletes never reached a real server.
- Loop device status was dropped.
- v1 queries were capped at four days.

Fixing them properly required a new surface. Most code maps across mechanically.

## Client

| 1.x | 2.0 |
|---|---|
| `NightscoutClient::new(url)?` | `Client::builder(url).build()?` or `Client::connect(url, Credentials::none()).await?` |
| `.with_secret(secret)` | `Credentials::api_secret(secret)` (admin, API v1 only), or better `Credentials::access_token(token)` |
| passing an access token to `with_secret` | `Credentials::access_token(token)` (it is exchanged for a JWT) |
| `client.base_url` / `client.inner` (public fields) | `client.base_url()`; internals are private |
| no timeouts | 10 s connect, 30 s total by default (`.timeout(...)`) |

`Client::connect` verifies the credentials up front, so a wrong token now fails with
`Error::Unauthorized` instead of silently reading as anonymous.

## Queries

| 1.x | 2.0 |
|---|---|
| `client.sgv().get().limit(5).send().await?` | `ns.entries().sgv().list().limit(5).await?` |
| `client.mbg().latest().await?` (errors when empty) | `ns.entries().mbg().latest().await?` (returns `Option`) |
| `.from(date)` / `.to(date)` | `.since(date)` / `.until(date)` |
| `.device(Device::Custom(name))` | `.device(name)` |
| `.device(Device::Auto)` | removed: it cost an extra request and silently widened to all devices on error |
| `client.treatments().get().send()` | `ns.treatments().list().await?` |
| `client.profiles().get().await?` | `ns.profiles().list().limit(10).await?` or `ns.profiles().current().await?` |
| `client.status().get().await?` | `ns.server().status().await?` |
| `client.properties().get().only(&[...]).send()` | `ns.properties().only([...]).await?` |
| `client.properties().get().at(time)` | removed: Nightscout has no such parameter. Use `ns.ddata(Some(time))` |
| `.id(id)` on a builder | `.get(&id)` on the collection |

Every request builder implements `IntoFuture`, so you `.await` it directly. `.send()` still
exists where you want to be explicit.

## Writes and deletes

| 1.x | 2.0 |
|---|---|
| `create(vec![doc])` | `create(&doc)` or `create_many(&docs)`; returns `Created { identifier, deduplicated, .. }` |
| `Treatment { event_type: "...".into(), created_at: ..., ... }` | `Treatment::carbs(15.0)`, `Treatment::bolus(1.5)`, `Treatment::temp_target(..)`, … |
| `SgvEntry::new(sgv, Trend::Flat, date)` | `Sgv::new(sgv, date).with_direction(Direction::Flat)` |
| `delete().id(id).send()` | `delete(id).await?` (`.permanent()` for a hard delete through API v3) |
| `delete().from(a).to(b).send()` | `delete_many(DeleteFilter::between(a, b)).await?` → `DeleteReport` |

## Models

- `SgvEntry` → `model::Sgv`, `MbgEntry` → `model::Mbg`; new: `model::Cal` and `model::Entry`.
- `Trend` → `model::Direction`. All 12 Nightscout values are covered, and unknown values
  round-trip in `Direction::Unknown`.
- `Treatment.event_type: String` → `Option<EventType>`. `created_at: String` →
  `Option<Timestamp>`.
- `DeviceStatus.loop_: Option<Value>` → `loop_status: Option<LoopStatus>`. It's typed, and
  it actually gets populated now.
- Every model keeps unknown fields in `extra`, so read-modify-write is lossless.
- Glucose values are `Glucose` (mg/dL, with `.as_mmol()`), and times are `Timestamp`
  (epoch ms, with `.to_datetime()`).

## Errors

`NightscoutError` → `cinnamon::Error`, which is `#[non_exhaustive]`:
- `AuthError` → `Unauthorized { message }` or `Forbidden { permission, .. }`.
- `ApiError { status, message }` → `Server { status, message }`, plus the specific
  variants `NotFound`, `Gone`, `BadRequest` and `ApiDisabled`.
- `JsonError` → `Decode { path, .. }`, which names the failing field.
- `RequestError` → `Transport(..)` or `Timeout`.
- `Unknown` is gone.
