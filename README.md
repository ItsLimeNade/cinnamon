<p align="center">
  <img src="https://raw.githubusercontent.com/ItsLimeNade/cinnamon/main/assets/cinnamonlogo.png" alt="Cinnamon Logo" width="220">
</p>

<h1 align="center">Cinnamon</h1>

<p align="center">
A type-safe, async Rust client for <a href="https://nightscout.github.io/">Nightscout</a>.
</p>

- **Every Nightscout API.** Entries (sgv/mbg/cal), treatments, device status, profiles,
  food, settings and activity; properties, ddata and summary; status, auth and
  notifications; admin (subjects, roles, permissions). It uses API v3 when available and
  falls back to v1/v2 automatically.
- **Safe auth.** Access tokens are exchanged for short-lived JWTs and refreshed before
  they expire. Credentials never follow redirects and never travel over plain HTTP, and
  a failed login is never retried in a loop that would lock you out.
- **Models that survive real data.** Numbers stored as strings, `null`s and unknown
  fields from any uploader (xDrip+, Loop, AAPS, Trio, OpenAPS) are tolerated. Fields
  cinnamon doesn't know are kept, so read-modify-write never loses data.
- **Incremental sync.** A persistable cursor mirrors collections, including the v1
  uploads that API v3 history misses.
- **Realtime (optional).** Live `/storage`, `/alarm` and `dataUpdate` streams over
  socket.io, with automatic reconnects.
- **Tested against a real Nightscout.** The suite runs against Nightscout 15.0.8 in
  Docker, not only against mocks.

## Installation

```toml
[dependencies]
cinnamon = "2.0.0-alpha.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

| Feature | Default | Enables |
|---|---|---|
| `rustls` | ✅ | TLS through rustls |
| `native-tls` | | TLS through the platform stack instead |
| `compression` | ✅ | gzip/brotli responses |
| `tracing` | ✅ | `tracing` events per request (never bodies or secrets) |
| `realtime` | | socket.io streams (`ns.realtime()`) |
| `blocking` | | a synchronous wrapper (`cinnamon::blocking::Client`) |

## Quick start

```rust,no_run
use chrono::{Duration, Utc};
use cinnamon::model::Treatment;
use cinnamon::{Client, Credentials};

#[tokio::main]
async fn main() -> cinnamon::Result<()> {
    // Create a token in Nightscout under Admin Tools → Subjects (role `readable` to read,
    // `careportal` to add treatments, `admin` for everything).
    let ns = Client::connect(
        "https://my-ns.example.com",
        Credentials::access_token("myapp-0123456789abcdef"),
    )
    .await?;

    // Latest glucose.
    if let Some(bg) = ns.entries().sgv().latest().await? {
        let arrow = bg.direction.as_ref().map_or("", |d| d.arrow());
        println!("{} mg/dL {arrow} at {}", bg.sgv, bg.date);
    }

    // The last 24 hours, newest first.
    let day = ns
        .entries()
        .sgv()
        .list()
        .since(Utc::now() - Duration::hours(24))
        .limit(288)
        .await?;
    println!("{} readings today", day.len());

    // Log a snack. Retrying is safe: uploads carry a deterministic identifier.
    ns.treatments()
        .create(&Treatment::carbs(15.0).with_notes("apple"))
        .await?;
    Ok(())
}
```

## Authentication

```rust,no_run
use cinnamon::{ApiVersion, Client, Credentials};
# async fn run() -> cinnamon::Result<()> {
// Recommended: a role-scoped access token (works with API v3 and v1).
let ns = Client::connect("https://my-ns.example.com", Credentials::access_token("myapp-0123456789abcdef")).await?;
println!("{:?}", ns.session().await?);

// Legacy: the server's API_SECRET grants full admin access and only works with API v1/v2.
let admin = Client::builder("https://my-ns.example.com")
    .api_secret("my-long-api-secret")
    .api(ApiVersion::V1)
    .connect()
    .await?;
# Ok(()) }
```

`connect` checks the credentials before returning, so a typo fails immediately instead of
silently degrading to anonymous access. Use `Client::builder(url).build()` to skip the
round trip.

## Queries, filters and streaming

```rust,no_run
use chrono::{Duration, Utc};
use cinnamon::model::EventType;
use cinnamon::query::Filter;
use futures_util::TryStreamExt;
# async fn run(ns: cinnamon::Client) -> cinnamon::Result<()> {
// Typed filters are encoded for whichever API the request goes through.
let highs = ns
    .entries()
    .sgv()
    .list()
    .filter(Filter::new().gte("sgv", 250))
    .since(Utc::now() - Duration::days(7))
    .limit(100)
    .await?;

let temp_basals = ns
    .treatments()
    .list()
    .filter(Filter::new().eq("eventType", EventType::TempBasal.as_str()))
    .await?;

// Stream a whole quarter page by page, without holding it in memory.
let mut stream = ns
    .entries()
    .sgv()
    .list()
    .since(Utc::now() - Duration::days(90))
    .limit(usize::MAX)
    .stream();
while let Some(sgv) = stream.try_next().await? {
    let _ = sgv.sgv.as_mmol();
}
# Ok(()) }
```

Unlike raw API v1, cinnamon never silently caps results at four days, and it never sends
an unbounded delete: `delete_many` requires a time range.

## Reading, updating and deleting

```rust,no_run
use cinnamon::api::DeleteFilter;
use cinnamon::model::Timestamp;
# async fn run(ns: cinnamon::Client) -> cinnamon::Result<()> {
let created = ns.treatments().create(&cinnamon::model::Treatment::note("hello")).await?;
let id = created.identifier.unwrap_or_default();

let mut note = ns.treatments().get(&id).await?;
note.notes = Some("hello again".into());
ns.treatments().update(&note).await?;

ns.treatments().delete(&id).await?; // soft delete (API v3); `.permanent()` to purge
let report = ns
    .treatments()
    .delete_many(DeleteFilter::before(Timestamp::from(chrono::Utc::now() - chrono::Duration::days(365))))
    .await?;
println!("deleted {}", report.deleted);
# Ok(()) }
```

## Incremental sync

```rust,no_run
use cinnamon::sync::{SyncCursor, SyncEvent};
use cinnamon::model::Treatment;
# async fn run(ns: cinnamon::Client, saved: SyncCursor) -> cinnamon::Result<()> {
let mut sync = ns.sync(saved);
let batch = sync.poll().await?;
for event in batch.events {
    match event {
        SyncEvent::Upsert(doc) => {
            if let Ok(t) = doc.decode::<Treatment>() {
                println!("treatment {:?}", t.kind());
            }
        }
        SyncEvent::Delete { collection, identifier } => println!("{collection}/{identifier} deleted"),
        _ => {}
    }
}
let cursor_json = serde_json::to_string(&batch.cursor).unwrap_or_default(); // persist it
# let _ = cursor_json;
# Ok(()) }
```

## Realtime (feature `realtime`)

```rust,ignore
use futures_util::StreamExt;

let mut glucose = ns.realtime().glucose().await?;
while let Some(reading) = glucose.next().await {
    println!("{:?} mg/dL", reading?.mgdl);
}
```

`ns.realtime().storage([...])` follows API v3 writes, `alarms()` follows alarms (and can
`ack` them), and `updates()` follows `dataUpdate` deltas. That last one is the only live
feed for the many uploaders that still write through API v1. See `examples/`.

## Blocking (feature `blocking`)

```rust,ignore
let ns = cinnamon::blocking::Client::connect(url, cinnamon::Credentials::access_token(token))?;
let latest = ns.run(ns.entries().sgv().latest())?;
```

## Errors

Every call returns `cinnamon::Result<T>`. `cinnamon::Error` separates `Unauthorized`,
`Forbidden { permission }`, `NotFound`, `Gone` (soft-deleted), `BadRequest`, `ApiDisabled`
(the server refuses writes), `Unsupported` (for example, settings on a v1-only server),
`Timeout`, `Transport` and `Decode { path }`. A decode error names the exact field that
broke.

## Development

```sh
cargo test --all-features                     # unit, transport and doc tests
docker compose -f tests/e2e/docker-compose.yml up -d
set -a; eval "$(tests/e2e/bootstrap.sh)"; set +a
CINNAMON_E2E=1 cargo test --all-features --test e2e -- --test-threads=1
```

See [`MIGRATION.md`](MIGRATION.md) for upgrading from 1.x, and [`PARITY.md`](PARITY.md)
for how each Nightscout endpoint maps to cinnamon.

## Disclaimer

NO MEDICAL ADVICE: This library is for educational and informational purposes only. It is
not intended to be relied upon for medical decisions, insulin dosing, or treatment
adjustments. Always consult with a qualified healthcare professional.

**If you have any concerns regarding your health or diabetes management, please contact
your healthcare provider immediately.**

**NO WARRANTY**: This software is provided "as is", without warranty of any kind. The data
retrieved may be inaccurate, delayed, or incomplete. The authors and contributors
explicitly disclaim any liability for any direct or indirect damage or health consequences
resulting from the use of this code.
