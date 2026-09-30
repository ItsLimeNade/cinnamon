//! End-to-end tests against a real, disposable Nightscout.
//!
//! ```sh
//! docker compose -f tests/e2e/docker-compose.yml up -d
//! set -a; eval "$(tests/e2e/bootstrap.sh)"; set +a
//! CINNAMON_E2E=1 cargo test --all-features --test e2e -- --test-threads=1
//! ```
//!
//! Without `CINNAMON_E2E=1` every test returns immediately. Tests isolate their data with a
//! unique `device` tag, so they can share one database.
#![allow(clippy::unwrap_used, clippy::print_stdout, missing_docs)]

use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

use chrono::{Duration, Utc};
use cinnamon::api::DeleteFilter;
use cinnamon::model::notification::Level;
use cinnamon::model::properties::Property;
use cinnamon::model::{
    Activity, Direction, Document, EventType, Food, Profile, ProfileStore, ScheduleEntry, Setting,
    Sgv, Timestamp, Treatment, TreatmentKind, compute_identifier,
};
use cinnamon::query::Filter;
use cinnamon::{ApiVersion, Client, Credentials, Error};
use futures_util::TryStreamExt;

struct Env {
    url: String,
    secret: String,
    admin: String,
    reader: String,
    careportal: String,
}

fn env() -> Option<Env> {
    if std::env::var("CINNAMON_E2E").ok()? != "1" {
        return None;
    }
    let var =
        |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} must be set for e2e tests"));
    Some(Env {
        url: var("NS_E2E_URL"),
        secret: var("NS_E2E_SECRET"),
        admin: var("NS_E2E_ADMIN_TOKEN"),
        reader: var("NS_E2E_READ_TOKEN"),
        careportal: var("NS_E2E_CAREPORTAL_TOKEN"),
    })
}

macro_rules! e2e_env {
    () => {
        match env() {
            Some(env) => env,
            None => {
                eprintln!("skipped: set CINNAMON_E2E=1 (see tests/e2e.rs)");
                return;
            }
        }
    };
}

fn tag(name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("e2e-{name}-{nanos}")
}

async fn token_client(env: &Env, token: &str, api: ApiVersion) -> Client {
    Client::builder(&env.url)
        .access_token(token)
        .api(api)
        .app_name("cinnamon-e2e")
        .connect()
        .await
        .unwrap()
}

async fn admin(env: &Env) -> Client {
    token_client(env, &env.admin, ApiVersion::Auto).await
}

async fn admin_v1(env: &Env) -> Client {
    token_client(env, &env.admin, ApiVersion::V1).await
}

async fn secret(env: &Env) -> Client {
    Client::builder(&env.url)
        .api_secret(&env.secret)
        .app_name("cinnamon-e2e")
        .connect()
        .await
        .unwrap()
}

/// A timestamp `minutes` ago, rounded to the second.
fn ago(minutes: i64) -> Timestamp {
    let t = Utc::now() - Duration::minutes(minutes);
    Timestamp::from_millis(t.timestamp() * 1000)
}

#[tokio::test]
async fn connect_validates_credentials() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let session = ns.session().await.unwrap().unwrap();
    assert_eq!(session.subject.as_deref(), Some("cinnamon-admin"));
    assert!(session.has_permission("api:treatments:create"));

    let bad = Client::connect(
        &env.url,
        Credentials::access_token("nobody-0000000000000000"),
    )
    .await;
    assert!(matches!(bad, Err(Error::Unauthorized { .. })), "{bad:?}");

    secret(&env).await;
    let wrong = Client::connect(&env.url, Credentials::api_secret("definitely-not-it")).await;
    assert!(
        matches!(wrong, Err(Error::Unauthorized { .. })),
        "{wrong:?}"
    );
}

#[tokio::test]
async fn discovery_and_server_endpoints() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let info = ns.server_info().await.unwrap();
    assert!(info.supports_v3(), "{info:?}");

    let status = ns.server().status().await.unwrap();
    assert_eq!(status.status.as_deref(), Some("ok"));
    assert!(status.settings.unwrap().is_enabled("careportal"));

    let versions = ns.server().versions().await.unwrap();
    assert!(versions.iter().any(|v| v.url == "/api/v3"));

    let auth = ns.server().verify_auth().await.unwrap();
    assert!(auth.can_read && auth.can_write);

    ns.server().last_modified().await.unwrap();
    let v3 = ns.server().v3_status().await.unwrap();
    assert!(v3.version.api_version.is_some());
    ns.server().admin_notifies().await.unwrap();
}

#[tokio::test]
async fn sgv_lifecycle_v3() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let device = tag("sgv-v3");
    let at = ago(60);

    let sgv = Sgv::new(123.0, at)
        .with_direction(Direction::Flat)
        .with_device(&device);
    let created = ns.entries().sgv().create(&sgv).await.unwrap();
    let id = created.identifier.clone().unwrap();
    assert_eq!(id, compute_identifier(Some(&device), at, None));
    assert!(!created.deduplicated);

    // Re-uploading the same reading deduplicates instead of duplicating.
    let again = ns.entries().sgv().create(&sgv).await.unwrap();
    assert!(again.deduplicated);

    let mut stored = ns.entries().sgv().get(&id).await.unwrap();
    assert!((stored.sgv.as_mgdl() - 123.0).abs() < f64::EPSILON);
    assert_eq!(stored.meta.app.as_deref(), Some("cinnamon-e2e"));

    let listed = ns
        .entries()
        .sgv()
        .list()
        .device(&device)
        .limit(5)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);

    stored.direction = Some(Direction::SingleUp);
    ns.entries().sgv().update(&stored).await.unwrap();
    ns.entries()
        .sgv()
        .patch(&id, &serde_json::json!({ "noise": 2 }))
        .await
        .unwrap();
    let stored = ns.entries().sgv().get(&id).await.unwrap();
    assert_eq!(stored.direction, Some(Direction::SingleUp));
    assert_eq!(stored.noise, Some(2));

    ns.entries().sgv().delete(&id).await.unwrap();
    assert!(matches!(
        ns.entries().sgv().get(&id).await,
        Err(Error::Gone)
    ));

    let history = ns
        .entries()
        .sgv()
        .history(
            Timestamp::from_millis(Utc::now().timestamp_millis() - 60_000),
            1000,
        )
        .await
        .unwrap();
    let deleted = history
        .docs
        .iter()
        .find(|d| d.id() == Some(id.as_str()))
        .unwrap();
    assert!(deleted.meta.is_deleted());

    ns.entries().sgv().delete(&id).permanent().await.unwrap();
    assert!(matches!(
        ns.entries().sgv().get(&id).await,
        Err(Error::NotFound)
    ));
}

/// Regression: cinnamon 1.x could not see entries older than 4 days through API v1, and its
/// by-id routes (`/entries/sgv.json/{id}`) returned 404.
#[tokio::test]
async fn v1_sees_old_entries_and_addresses_them_by_id() {
    let env = e2e_env!();
    let ns = secret(&env).await;
    let device = tag("sgv-v1");
    let old = Timestamp::from(Utc::now() - Duration::days(30));

    ns.entries()
        .sgv()
        .create(&Sgv::new(99.0, old).with_device(&device))
        .await
        .unwrap();
    let listed = ns
        .entries()
        .sgv()
        .list()
        .device(&device)
        .limit(10)
        .await
        .unwrap();
    assert_eq!(
        listed.len(),
        1,
        "a 30-day-old entry must be visible through API v1"
    );

    let id = listed[0].meta.id.clone().unwrap();
    let fetched = ns.entries().sgv().get(&id).await.unwrap();
    assert_eq!(fetched.date, old);

    ns.entries().sgv().delete(&id).await.unwrap();
    assert!(
        ns.entries()
            .sgv()
            .list()
            .device(&device)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        ns.entries().sgv().delete(&id).await,
        Err(Error::NotFound)
    ));
}

#[tokio::test]
async fn treatments_v3_dedup_and_typed_view() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let device = tag("treat-v3");
    let snack = Treatment::carbs(15.0)
        .at(ago(30))
        .with_device(&device)
        .with_notes("apple");

    let first = ns.treatments().create(&snack).await.unwrap();
    let second = ns.treatments().create(&snack).await.unwrap();
    assert_eq!(first.identifier, second.identifier);
    assert!(second.deduplicated);

    let id = first.identifier.unwrap();
    let mut stored = ns.treatments().get(&id).await.unwrap();
    assert_eq!(stored.kind(), TreatmentKind::Carbs { grams: 15.0 });
    stored.notes = Some("green apple".into());
    ns.treatments().update(&stored).await.unwrap();
    assert_eq!(
        ns.treatments().get(&id).await.unwrap().notes.as_deref(),
        Some("green apple")
    );
    ns.treatments().delete(&id).permanent().await.unwrap();
}

/// Documents uploaded through API v1 lack `app`/`date`/`utcOffset`, which API v3 refuses to
/// replace; `update` must fall back to patching them.
#[tokio::test]
async fn legacy_treatment_is_updatable_through_v3() {
    let env = e2e_env!();
    let device = tag("legacy");
    let v1 = secret(&env).await;
    let mut legacy = Treatment::note("from v1").at(ago(45)).with_device(&device);
    legacy.meta.app = None;
    v1.treatments().create(&legacy).await.unwrap();

    let ns = admin(&env).await;
    let mut stored = ns
        .treatments()
        .list()
        .device(&device)
        .limit(1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    stored.notes = Some("edited through v3".into());
    ns.treatments().update(&stored).await.unwrap();
    let id = stored.id().unwrap().to_owned();
    assert_eq!(
        ns.treatments().get(&id).await.unwrap().notes.as_deref(),
        Some("edited through v3")
    );
    ns.treatments().delete(&id).permanent().await.unwrap();
}

async fn bulk_delete_roundtrip(ns: &Client, device: &str) {
    let times: Vec<Timestamp> = (0..7).map(|i| ago(200 + i)).collect();
    let docs: Vec<Treatment> = times
        .iter()
        .map(|t| Treatment::note("bulk").at(*t).with_device(device))
        .collect();
    ns.treatments().create_many(&docs).await.unwrap();
    assert_eq!(
        ns.treatments()
            .list()
            .device(device)
            .limit(50)
            .await
            .unwrap()
            .len(),
        7
    );

    let report = ns
        .treatments()
        .delete_many(
            DeleteFilter::between(ago(300), ago(100)).and(Filter::new().eq("device", device)),
        )
        .permanent()
        .await
        .unwrap();
    assert_eq!(report.deleted, 7, "{report:?}");
    assert!(report.failed.is_empty());
    assert!(
        ns.treatments()
            .list()
            .device(device)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn delete_many_v3() {
    let env = e2e_env!();
    bulk_delete_roundtrip(&admin(&env).await, &tag("bulk-v3")).await;
}

#[tokio::test]
async fn delete_many_v1() {
    let env = e2e_env!();
    bulk_delete_roundtrip(&admin_v1(&env).await, &tag("bulk-v1")).await;
}

#[tokio::test]
async fn permissions_are_enforced_and_reported() {
    let env = e2e_env!();
    let reader = token_client(&env, &env.reader, ApiVersion::Auto).await;
    let err = reader
        .treatments()
        .create(&Treatment::note("nope").with_device(tag("reader")))
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Forbidden { ref permission, .. } if permission.as_deref() == Some("api:treatments:create")),
        "{err:?}"
    );

    // A careportal token may create treatments through API v3…
    let careportal = token_client(&env, &env.careportal, ApiVersion::Auto).await;
    let device = tag("careportal");
    let created = careportal
        .treatments()
        .create(&Treatment::carbs(5.0).at(ago(20)).with_device(&device))
        .await
        .unwrap();
    admin(&env)
        .await
        .treatments()
        .delete(created.identifier.unwrap())
        .permanent()
        .await
        .unwrap();

    // …but API v1 also requires read access, which it lacks when default roles are denied.
    let careportal_v1 = token_client(&env, &env.careportal, ApiVersion::V1).await;
    let err = careportal_v1
        .treatments()
        .create(&Treatment::carbs(5.0).with_device(&device))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Unauthorized { .. }), "{err:?}");

    let admin = admin(&env).await;
    assert!(admin.admin().check("api:entries:read").await.unwrap());
}

async fn stream_is_complete_and_ordered(ns: &Client, a: &str, b: &str) {
    let docs: Vec<Sgv> = ns
        .entries()
        .sgv()
        .list()
        .filter(Filter::new().one_of("device", [a, b]))
        .limit(usize::MAX)
        .page_size(4)
        .stream()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(docs.len(), 23, "every document exactly once");
    assert!(
        docs.windows(2).all(|w| w[0].date >= w[1].date),
        "newest first"
    );
    let mut ids: Vec<_> = docs.iter().map(|d| (d.device.clone(), d.date)).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 23, "no duplicates across page boundaries");
}

#[tokio::test]
async fn streams_paginate_across_equal_timestamps() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let (a, b) = (tag("stream-a"), tag("stream-b"));
    let mut docs: Vec<Sgv> = (0..20)
        .map(|i| Sgv::new(100.0 + f64::from(i), ago(500 + i64::from(i))).with_device(&a))
        .collect();
    // Three readings from another device share timestamps with the first ones.
    docs.extend((0..3).map(|i| Sgv::new(150.0, ago(500 + i)).with_device(&b)));
    ns.entries().sgv().create_many(&docs).await.unwrap();

    stream_is_complete_and_ordered(&ns, &a, &b).await;
    stream_is_complete_and_ordered(&admin_v1(&env).await, &a, &b).await;

    for device in [&a, &b] {
        ns.entries()
            .sgv()
            .delete_many(
                DeleteFilter::before(Timestamp::now())
                    .and(Filter::new().eq("device", device.as_str())),
            )
            .permanent()
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn computed_state_endpoints() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let device = tag("props");
    let docs: Vec<Sgv> = (0..4)
        .map(|i| Sgv::new(120.0 + f64::from(i) * 3.0, ago(i64::from(i) * 5)).with_device(&device))
        .collect();
    ns.entries().sgv().create_many(&docs).await.unwrap();
    tokio::time::sleep(StdDuration::from_millis(1500)).await;

    let props = ns
        .properties()
        .only([
            Property::BgNow,
            Property::Delta,
            Property::Direction,
            Property::Iob,
            Property::Basal,
        ])
        .await
        .unwrap();
    assert!(props.bgnow.is_some(), "{props:?}");
    ns.properties().await.unwrap();
    let ddata = ns.ddata(None).await.unwrap();
    assert!(!ddata.sgvs.is_empty());
    ns.ddata(Some(ago(60))).await.unwrap();
    ns.summary(6).await.unwrap();

    ns.entries()
        .sgv()
        .delete_many(
            DeleteFilter::before(Timestamp::now()).and(Filter::new().eq("device", device.as_str())),
        )
        .permanent()
        .await
        .unwrap();
}

#[tokio::test]
async fn profiles_food_settings_activity() {
    let env = e2e_env!();
    let ns = admin(&env).await;

    let mut profile = Profile::default();
    profile.dia = Some(5.0);
    profile.units = Some("mg/dl".into());
    profile.timezone = Some("UTC".into());
    profile.basal = vec![
        ScheduleEntry::new(0, 0.8),
        ScheduleEntry::new(6 * 3600, 1.1),
    ];
    profile.sens = vec![ScheduleEntry::new(0, 40.0)];
    profile.carbratio = vec![ScheduleEntry::new(0, 10.0)];
    profile.target_low = vec![ScheduleEntry::new(0, 90.0)];
    profile.target_high = vec![ScheduleEntry::new(0, 120.0)];
    let created = ns
        .profiles()
        .create(&ProfileStore::single("e2e", profile))
        .await
        .unwrap();
    let current = ns.profiles().current().await.unwrap().unwrap();
    assert!(current.store.contains_key("e2e"), "{current:?}");
    assert!(
        admin_v1(&env)
            .await
            .profiles()
            .current()
            .await
            .unwrap()
            .is_some()
    );
    ns.profiles()
        .delete(created.identifier.unwrap())
        .permanent()
        .await
        .unwrap();

    let mut pick = Food::new(tag("pick"), 20.0, 1.0, "pcs");
    pick.food_type = Some("quickpick".into());
    pick.hidden = Some(false);
    let food = ns.food().create(&pick).await.unwrap();
    assert!(
        ns.food()
            .quickpicks()
            .await
            .unwrap()
            .iter()
            .any(|f| f.name == pick.name)
    );
    ns.food()
        .delete(food.identifier.unwrap())
        .permanent()
        .await
        .unwrap();

    let key = tag("settings");
    let mut values = serde_json::Map::new();
    values.insert("theme".into(), "dark".into());
    ns.settings()
        .create(&Setting::new(&key, values))
        .await
        .unwrap();
    assert_eq!(
        ns.settings().get(&key).await.unwrap().extra["theme"],
        "dark"
    );
    ns.settings().delete(&key).permanent().await.unwrap();
    assert!(matches!(
        secret(&env).await.settings().list().await,
        Err(Error::Unsupported { .. })
    ));

    let device = tag("activity");
    let mut activity = Activity::new();
    activity.steps = Some(1234.0);
    activity.device = Some(device.clone());
    ns.activity().create(&activity).await.unwrap();
    let listed = ns.activity().list().device(&device).await.unwrap();
    assert_eq!(listed[0].steps, Some(1234.0));
    ns.activity().delete(listed[0].id().unwrap()).await.unwrap();
}

#[tokio::test]
async fn admin_and_notifications() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let subjects = ns.admin().subjects().await.unwrap();
    assert!(subjects.iter().any(|s| s.name == "cinnamon-admin"));
    let roles = ns.admin().roles().await.unwrap();
    assert!(roles.iter().any(|r| r.name == "readable"));
    ns.admin().permissions().await.unwrap();
    ns.notifications()
        .ack(Level::Urgent, "default", StdDuration::from_secs(60))
        .await
        .unwrap();
}

#[tokio::test]
async fn entries_v1_helpers() {
    let env = e2e_env!();
    let ns = admin(&env).await;
    let device = tag("helpers");
    let docs: Vec<Sgv> = (0..3)
        .map(|i| Sgv::new(110.0, ago(40 + i)).with_device(&device))
        .collect();
    ns.entries().sgv().create_many(&docs).await.unwrap();

    let count = ns
        .entries()
        .count(
            &Filter::new()
                .eq("type", "sgv")
                .eq("device", device.as_str()),
        )
        .await
        .unwrap();
    assert_eq!(count, 3);
    assert!(ns.entries().current().await.unwrap().is_some());
    ns.entries()
        .echo(
            cinnamon::model::CollectionName::Entries,
            &Filter::new().eq("device", device.as_str()),
        )
        .await
        .unwrap();

    ns.entries()
        .sgv()
        .delete_many(
            DeleteFilter::before(Timestamp::now()).and(Filter::new().eq("device", device.as_str())),
        )
        .permanent()
        .await
        .unwrap();
}

/// Regression: cinnamon 1.x lost Loop's `loop` object (it was read as `loop_`).
#[tokio::test]
async fn loop_devicestatus_round_trips() {
    let env = e2e_env!();
    for ns in [admin(&env).await, admin_v1(&env).await] {
        let device = tag("loop");
        let mut status: cinnamon::model::DeviceStatus = serde_json::from_value(serde_json::json!({
            "device": device,
            "created_at": ago(15).to_iso(),
            "loop": {"name": "Loop", "iob": {"iob": 1.5}, "cob": {"cob": 20},
                     "predicted": {"startDate": ago(15).to_iso(), "values": [120, 118, 115]}},
            "pump": {"reservoir": 88.5, "battery": {"percent": 60}}
        }))
        .unwrap();
        status.meta.app = None;
        ns.devicestatus().create(&status).await.unwrap();

        let stored = ns
            .devicestatus()
            .list()
            .device(&device)
            .await
            .unwrap()
            .pop()
            .unwrap();
        let lp = stored.loop_status.as_ref().unwrap();
        assert_eq!(lp.iob.as_ref().and_then(|i| i.iob), Some(1.5));
        assert_eq!(
            lp.predicted.as_ref().unwrap().values,
            vec![120.0, 118.0, 115.0]
        );
        assert_eq!(stored.pump.as_ref().and_then(|p| p.reservoir), Some(88.5));
        assert!(!stored.extra.contains_key("loop_"));
        ns.devicestatus()
            .delete(stored.id().unwrap())
            .await
            .unwrap();
    }
}

fn events_for<'a>(
    batch: &'a cinnamon::sync::SyncBatch,
    device: &str,
    id: &str,
) -> Vec<&'a cinnamon::sync::SyncEvent> {
    use cinnamon::sync::SyncEvent;
    batch
        .events
        .iter()
        .filter(|e| match e {
            SyncEvent::Upsert(doc) => {
                doc.doc.get("device").and_then(|d| d.as_str()) == Some(device)
            }
            SyncEvent::Delete { identifier, .. } => identifier == id,
            _ => false,
        })
        .collect()
}

#[tokio::test]
async fn sync_sees_creates_updates_deletes_and_legacy_uploads() {
    use cinnamon::model::CollectionName;
    use cinnamon::sync::{SyncCursor, SyncEvent};

    let env = e2e_env!();
    let ns = admin(&env).await;
    let mut sync = ns
        .sync(SyncCursor::default())
        .collections([CollectionName::Treatments])
        .backfill(Duration::hours(1));
    sync.poll().await.unwrap();

    // Create through v3.
    let device = tag("sync");
    let created = ns
        .treatments()
        .create(&Treatment::carbs(12.0).at(ago(5)).with_device(&device))
        .await
        .unwrap();
    let id = created.identifier.unwrap();
    let batch = sync.poll().await.unwrap();
    let seen = events_for(&batch, &device, &id);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(
        match seen[0] {
            SyncEvent::Upsert(doc) => doc.decode::<Treatment>().unwrap().carbs,
            _ => None,
        },
        Some(12.0)
    );

    // Nothing new: nothing emitted (despite the date-search overlap).
    let batch = sync.poll().await.unwrap();
    assert!(
        events_for(&batch, &device, &id).is_empty(),
        "{:?}",
        batch.events
    );

    // Update, then delete.
    ns.treatments()
        .patch(&id, &serde_json::json!({ "notes": "edited" }))
        .await
        .unwrap();
    let batch = sync.poll().await.unwrap();
    assert!(matches!(
        events_for(&batch, &device, &id)[..],
        [SyncEvent::Upsert(_)]
    ));
    ns.treatments().delete(&id).await.unwrap();
    let batch = sync.poll().await.unwrap();
    assert!(
        matches!(
            events_for(&batch, &device, &id)[..],
            [SyncEvent::Delete { .. }]
        ),
        "{:?}",
        batch.events
    );

    // A legacy upload through API v1 has no srvModified; the date search must catch it.
    let legacy_device = tag("sync-legacy");
    let mut legacy = Treatment::note("legacy")
        .at(ago(3))
        .with_device(&legacy_device);
    legacy.meta.app = None;
    secret(&env)
        .await
        .treatments()
        .create(&legacy)
        .await
        .unwrap();
    let batch = sync.poll().await.unwrap();
    assert_eq!(
        events_for(&batch, &legacy_device, "").len(),
        1,
        "{:?}",
        batch.events
    );

    // The cursor survives serialization and resumes without replaying.
    let json = serde_json::to_string(&batch.cursor).unwrap();
    let resumed: SyncCursor = serde_json::from_str(&json).unwrap();
    let mut sync2 = ns.sync(resumed).collections([CollectionName::Treatments]);
    let batch = sync2.poll().await.unwrap();
    assert!(events_for(&batch, &device, &id).is_empty());

    ns.treatments().delete(&id).permanent().await.unwrap();
    let legacy_doc = ns
        .treatments()
        .list()
        .device(&legacy_device)
        .await
        .unwrap()
        .pop()
        .unwrap();
    ns.treatments()
        .delete(legacy_doc.id().unwrap())
        .permanent()
        .await
        .unwrap();
}

#[tokio::test]
async fn missing_documents_are_not_found() {
    let env = e2e_env!();
    for ns in [admin(&env).await, admin_v1(&env).await] {
        let res = ns.treatments().get("65f1c2a8e4b0a1b2c3d4e5f6").await;
        assert!(matches!(res, Err(Error::NotFound)), "{res:?}");
    }
    let _ = EventType::Note;
}

#[cfg(feature = "realtime")]
mod realtime {
    use super::*;
    use cinnamon::model::CollectionName;
    use cinnamon::realtime::{AlarmEvent, StorageEvent, UpdateEvent};
    use futures_util::StreamExt;

    async fn next_matching<S, T, F>(stream: &mut S, secs: u64, mut pred: F) -> T
    where
        S: futures_util::Stream<Item = cinnamon::Result<T>> + Unpin,
        F: FnMut(&T) -> bool,
        T: std::fmt::Debug,
    {
        tokio::time::timeout(StdDuration::from_secs(secs), async {
            loop {
                let item = stream.next().await.expect("stream ended").unwrap();
                if pred(&item) {
                    return item;
                }
            }
        })
        .await
        .expect("timed out waiting for a realtime event")
    }

    #[tokio::test]
    async fn storage_channel_sees_v3_writes() {
        let env = e2e_env!();
        let ns = admin(&env).await;
        let mut sub = ns
            .realtime()
            .storage([CollectionName::Treatments])
            .await
            .unwrap();
        let device = tag("socket");
        let created = ns
            .treatments()
            .create(&Treatment::note("live").at(ago(1)).with_device(&device))
            .await
            .unwrap();
        let id = created.identifier.unwrap();
        let event = next_matching(
            &mut sub,
            10,
            |e| matches!(e, StorageEvent::Created(d) if d.identifier() == Some(id.as_str())),
        )
        .await;
        let StorageEvent::Created(doc) = event else {
            unreachable!()
        };
        assert_eq!(
            doc.decode::<Treatment>().unwrap().notes.as_deref(),
            Some("live")
        );

        ns.treatments().delete(&id).permanent().await.unwrap();
        next_matching(
            &mut sub,
            10,
            |e| matches!(e, StorageEvent::Deleted { identifier, .. } if *identifier == id),
        )
        .await;

        // The storage channel refuses API secrets (it only takes access tokens).
        assert!(matches!(
            secret(&env)
                .await
                .realtime()
                .storage([CollectionName::Entries])
                .await,
            Err(Error::Unsupported { .. })
        ));
    }

    #[tokio::test]
    async fn legacy_updates_and_glucose_follow_v1_uploads() {
        let env = e2e_env!();
        let ns = admin(&env).await;
        let mut updates = ns.realtime().updates().await.unwrap();
        let first = next_matching(&mut updates, 10, |_| true).await;
        assert!(
            matches!(first, UpdateEvent::Data(ref d) if !d.is_delta()),
            "{first:?}"
        );
        let mut glucose = ns.realtime().glucose().await.unwrap();

        let device = tag("socket-v1");
        let v1 = secret(&env).await;
        v1.entries()
            .sgv()
            .create(&Sgv::new(142.0, Timestamp::now()).with_device(&device))
            .await
            .unwrap();

        next_matching(&mut updates, 20, |e| {
            matches!(e, UpdateEvent::Data(d) if d.sgvs.iter().any(|s| s.device.as_deref() == Some(device.as_str())))
        })
        .await;
        let reading = tokio::time::timeout(StdDuration::from_secs(20), async {
            loop {
                let r = glucose.next().await.expect("glucose stream ended").unwrap();
                if r.device.as_deref() == Some(device.as_str()) {
                    return r;
                }
            }
        })
        .await
        .expect("no glucose reading");
        assert_eq!(reading.mgdl, Some(142.0));

        v1.entries()
            .sgv()
            .delete_many(
                DeleteFilter::before(Timestamp::now())
                    .and(Filter::new().eq("device", device.as_str())),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn alarm_channel_subscribes_and_acks() {
        let env = e2e_env!();
        let ns = admin(&env).await;
        let sub = ns.realtime().alarms().await.unwrap();
        sub.ack(Level::Urgent, "default", StdDuration::from_secs(60))
            .unwrap();
        let _ = AlarmEvent::Reconnected;

        let via_secret = secret(&env).await.realtime().alarms().await;
        assert!(via_secret.is_ok(), "{via_secret:?}");
    }

    #[tokio::test]
    async fn bad_credentials_fail_the_subscription_up_front() {
        let env = e2e_env!();
        let ns = Client::builder(&env.url)
            .api_secret("not-the-secret-at-all")
            .build()
            .unwrap();
        let res = ns.realtime().updates().await;
        assert!(matches!(res, Err(Error::Unauthorized { .. })), "{res:?}");
    }
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_client_runs_requests() {
    let env = e2e_env!();
    let ns = cinnamon::blocking::Client::connect(&env.url, Credentials::access_token(&env.admin))
        .unwrap();
    ns.run(ns.entries().sgv().list().limit(1)).unwrap();
    ns.run(ns.server().status()).unwrap();
}
