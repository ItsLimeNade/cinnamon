//! Behavior that a healthy Nightscout cannot easily produce: retries, redirects, JWT
//! refresh, credential placement, decode errors, failures in the middle of a sync, and
//! paging over more data than an e2e test can cheaply create. Real API semantics are
//! covered by the e2e suite (`tests/e2e.rs`) instead of mocks.
#![allow(clippy::unwrap_used, missing_docs)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cinnamon::model::{CollectionName, Sgv};
use cinnamon::sync::SyncCursor;
use cinnamon::{ApiVersion, Client, Error, RetryPolicy};
use futures_util::TryStreamExt;
use serde_json::{Value, json};
use wiremock::matchers::{header, header_exists, method, path, path_regex};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn fast_retries() -> RetryPolicy {
    RetryPolicy::new(2, Duration::from_millis(1), Duration::from_millis(5))
}

async fn mount_v3(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v3/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": 200,
            "result": {"version": "15.0.8", "apiVersion": "3.0.5", "srvDate": 1, "storage": {"storage": "mongodb", "version": "7"}}
        })))
        .mount(server)
        .await;
}

fn jwt_response(exp_in_secs: i64) -> ResponseTemplate {
    let now = chrono::Utc::now().timestamp();
    ResponseTemplate::new(200).set_body_json(json!({
        "token": format!("jwt-{now}-{exp_in_secs}"),
        "sub": "tester",
        "permissionGroups": [["*"]],
        "iat": now,
        "exp": now + exp_in_secs
    }))
}

#[tokio::test]
async fn idempotent_requests_retry_on_5xx_then_succeed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/entries.json"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/entries.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"type": "sgv", "sgv": 101, "date": 1_714_564_800_000_i64}
        ])))
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .retry(fast_retries())
        .build()
        .unwrap();
    let sgvs = ns.entries().sgv().list().limit(1).await.unwrap();
    assert_eq!(sgvs.len(), 1);
}

#[tokio::test]
async fn non_idempotent_writes_are_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/devicestatus"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .api_secret("0123456789abcdef")
        .retry(fast_retries())
        .build()
        .unwrap();
    let err = ns
        .devicestatus()
        .create(&cinnamon::model::DeviceStatus::new("test"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Server { status: 503, .. }), "{err:?}");
}

#[tokio::test]
async fn redirects_are_refused_so_secrets_never_follow_them() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/status.json"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("Location", "https://evil.example.com/api/v1/status.json"),
        )
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .api_secret("0123456789abcdef")
        .build()
        .unwrap();
    let err = ns.server().status().await.unwrap_err();
    assert!(
        matches!(err, Error::Redirected { status: 307, ref location } if location.contains("evil")),
        "{err:?}"
    );
}

#[tokio::test]
async fn api_secret_is_sent_as_sha1_and_never_to_v3() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/status.json"))
        .and(header(
            "api-secret",
            "30143ac058893ef38d5e090c9719103474b9ec19",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .api_secret("cinnamon-e2e-secret")
        .build()
        .unwrap();
    ns.server().status().await.unwrap();

    let err = ns.settings().list().await.unwrap_err();
    assert!(matches!(err, Error::Unsupported { .. }), "{err:?}");
    let v3_hits = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.url.path().starts_with("/api/v3/") && r.url.path() != "/api/v3/version")
        .count();
    assert_eq!(v3_hits, 0);
}

#[tokio::test]
async fn access_tokens_are_exchanged_once_and_sent_as_bearer() {
    let server = MockServer::start().await;
    mount_v3(&server).await;
    Mock::given(method("GET"))
        .and(path_regex("^/api/v2/authorization/request/.+$"))
        .respond_with(jwt_response(8 * 3600))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/treatments"))
        .and(header_exists("authorization"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status": 200, "result": []})),
        )
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .access_token("tester-0123456789abcdef")
        .build()
        .unwrap();
    for _ in 0..3 {
        ns.treatments().list().await.unwrap();
    }
    let requests = server.received_requests().await.unwrap();
    let bearer = |r: &&Request| {
        r.headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("Bearer jwt-"))
    };
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path() == "/api/v3/treatments")
            .filter(bearer)
            .count(),
        3
    );
    // The raw token only ever travels to the exchange endpoint.
    assert!(
        requests
            .iter()
            .filter(|r| !r.url.path().starts_with("/api/v2/authorization/request/"))
            .all(|r| !r.url.as_str().contains("0123456789abcdef"))
    );
}

#[tokio::test]
async fn expiring_jwts_are_refreshed_proactively() {
    let server = MockServer::start().await;
    mount_v3(&server).await;
    // Expires in 60 s, inside the 5-minute refresh margin: every call must re-exchange.
    Mock::given(method("GET"))
        .and(path_regex("^/api/v2/authorization/request/.+$"))
        .respond_with(jwt_response(60))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/treatments"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status": 200, "result": []})),
        )
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .access_token("tester-0123456789abcdef")
        .build()
        .unwrap();
    ns.treatments().list().await.unwrap();
    ns.treatments().list().await.unwrap();
}

#[tokio::test]
async fn a_401_with_a_fresh_jwt_is_not_retried() {
    let server = MockServer::start().await;
    mount_v3(&server).await;
    Mock::given(method("GET"))
        .and(path_regex("^/api/v2/authorization/request/.+$"))
        .respond_with(jwt_response(8 * 3600))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/treatments"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({"status": 401, "message": "Bad access token or JWT"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let ns = Client::builder(server.uri())
        .access_token("tester-0123456789abcdef")
        .retry(fast_retries())
        .build()
        .unwrap();
    let err = ns.treatments().list().await.unwrap_err();
    assert!(matches!(err, Error::Unauthorized { .. }), "{err:?}");
}

#[tokio::test]
async fn requests_ask_for_json_and_identify_themselves() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/status.json"))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .mount(&server)
        .await;

    let ns = Client::new_local_for_tests(&server.uri());
    ns.server().status().await.unwrap();
    let ua = server.received_requests().await.unwrap()[0]
        .headers
        .get("user-agent")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(ua.starts_with("cinnamon/"), "{ua}");
}

#[tokio::test]
async fn undecodable_documents_are_skipped_not_fatal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/entries.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"type": "sgv", "sgv": 101, "date": 1_714_564_800_000_i64},
            {"type": "sgv", "sgv": null, "date": 1_714_564_500_000_i64},
            {"type": "sgv", "sgv": "99", "date": "2024-05-01T11:50:00Z"}
        ])))
        .mount(&server)
        .await;
    let ns = Client::new_local_for_tests(&server.uri());
    let sgvs = ns.entries().sgv().list().limit(3).await.unwrap();
    assert_eq!(sgvs.len(), 2);
}

#[tokio::test]
async fn malformed_responses_name_the_failing_field() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/verifyauth"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status": 200, "message": {"canRead": "maybe"}})),
        )
        .mount(&server)
        .await;
    let ns = Client::new_local_for_tests(&server.uri());
    match ns.server().verify_auth().await.unwrap_err() {
        Error::Decode { path, .. } => assert_eq!(path, "message.canRead"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn v1_lists_always_bound_the_date_to_defeat_the_4_day_window() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/treatments.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    let ns = Client::builder(server.uri())
        .api(ApiVersion::V1)
        .build()
        .unwrap();
    ns.treatments().list().await.unwrap();
    let query = server.received_requests().await.unwrap()[0]
        .url
        .query()
        .unwrap_or_default()
        .to_owned();
    let decoded = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>();
    assert!(
        decoded.contains(&"find[created_at][$gte]=1970-01-01T00:00:00.000Z".to_owned()),
        "{decoded:?}"
    );
}

trait LocalClient {
    fn new_local_for_tests(uri: &str) -> Client;
}

impl LocalClient for Client {
    fn new_local_for_tests(uri: &str) -> Client {
        Client::builder(uri)
            .retry(RetryPolicy::none())
            .build()
            .unwrap()
    }
}

/// `count` sgvs, newest first: `newest`, `newest - step_ms`, …
fn sgvs(count: i64, newest: i64, step_ms: i64) -> Vec<Value> {
    (0..count)
        .map(|i| {
            json!({"_id": format!("{i:024x}"), "type": "sgv", "sgv": 100, "date": newest - i * step_ms})
        })
        .collect()
}

/// An API v1 `entries.json` over `docs` that honors `count`, the `date` bounds and
/// `sort[date]` the way Nightscout does.
async fn mount_v1_entries(server: &MockServer, docs: Arc<Mutex<Vec<Value>>>) {
    Mock::given(method("GET"))
        .and(path("/api/v1/entries.json"))
        .respond_with(move |req: &Request| {
            let param = |key: &str| {
                req.url
                    .query_pairs()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.into_owned())
            };
            let bound = |key: &str| param(key).and_then(|v| v.parse::<i64>().ok());
            let (gte, lte) = (bound("find[date][$gte]"), bound("find[date][$lte]"));
            let count = bound("count").unwrap_or(10) as usize;
            let mut page: Vec<Value> = docs
                .lock()
                .unwrap()
                .iter()
                .filter(|d| {
                    let t = d["date"].as_i64().unwrap();
                    gte.is_none_or(|g| t >= g) && lte.is_none_or(|l| t <= l)
                })
                .cloned()
                .collect();
            page.sort_by_key(|d| d["date"].as_i64());
            if param("sort[date]").as_deref() != Some("1") {
                page.reverse();
            }
            page.truncate(count);
            ResponseTemplate::new(200).set_body_json(page)
        })
        .mount(server)
        .await;
}

const NOW: i64 = 1_758_790_800_000;

#[tokio::test]
async fn lists_above_one_page_are_paged_not_capped() {
    let server = MockServer::start().await;
    mount_v1_entries(&server, Arc::new(Mutex::new(sgvs(2500, NOW, 1000)))).await;
    let ns = Client::new_local_for_tests(&server.uri());
    let docs = ns.entries().sgv().list().limit(2000).await.unwrap();
    assert_eq!(docs.len(), 2000);
    assert!(
        docs.windows(2).all(|w| w[0].date > w[1].date),
        "newest first, no repeats"
    );
}

#[tokio::test]
async fn stream_skip_applies_to_the_first_page_only() {
    let server = MockServer::start().await;
    mount_v1_entries(&server, Arc::new(Mutex::new(sgvs(100, NOW, 1000)))).await;
    let ns = Client::new_local_for_tests(&server.uri());
    let docs: Vec<Sgv> = ns
        .entries()
        .sgv()
        .list()
        .skip(2)
        .limit(20)
        .page_size(5)
        .stream()
        .try_collect()
        .await
        .unwrap();
    let dates: Vec<i64> = docs.iter().map(|d| d.date.as_millis()).collect();
    let expected: Vec<i64> = (2..22).map(|i| NOW - i * 1000).collect();
    assert_eq!(dates, expected);
}

#[tokio::test]
async fn a_page_that_does_not_decode_does_not_end_the_stream() {
    let server = MockServer::start().await;
    let mut docs = sgvs(10, NOW, 1000);
    for doc in &mut docs[..5] {
        doc.as_object_mut().unwrap().remove("sgv");
    }
    mount_v1_entries(&server, Arc::new(Mutex::new(docs))).await;
    let ns = Client::new_local_for_tests(&server.uri());
    let docs: Vec<Sgv> = ns
        .entries()
        .sgv()
        .list()
        .limit(usize::MAX)
        .page_size(5)
        .stream()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(
        docs.len(),
        5,
        "the five readable readings behind the broken page"
    );
}

#[tokio::test]
async fn a_failed_sync_poll_is_retried_without_losing_events() {
    let server = MockServer::start().await;
    let now = chrono::Utc::now().timestamp_millis();
    mount_v1_entries(&server, Arc::new(Mutex::new(sgvs(1, now - 60_000, 1)))).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/treatments.json"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/treatments.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let ns = Client::new_local_for_tests(&server.uri());
    let mut sync = ns
        .sync(SyncCursor::default())
        .collections([CollectionName::Entries, CollectionName::Treatments]);
    assert!(sync.poll().await.is_err());
    assert_eq!(sync.cursor(), &SyncCursor::default(), "nothing committed");
    let batch = sync.poll().await.unwrap();
    assert_eq!(batch.events.len(), 1, "{:?}", batch.events);
}

#[tokio::test]
async fn sync_moves_past_a_full_page_of_already_seen_documents() {
    let server = MockServer::start().await;
    let now = chrono::Utc::now().timestamp_millis();
    // 600 readings inside the last five minutes: more than one page (500) sits in the
    // ten-minute overlap every poll re-reads.
    let docs = Arc::new(Mutex::new(sgvs(600, now - 60_000, 400)));
    mount_v1_entries(&server, Arc::clone(&docs)).await;
    let ns = Client::new_local_for_tests(&server.uri());
    let mut sync = ns
        .sync(SyncCursor::default())
        .collections([CollectionName::Entries]);
    assert_eq!(sync.poll().await.unwrap().events.len(), 600);

    docs.lock().unwrap().push(json!(
        {"_id": "ffffffffffffffffffffffff", "type": "sgv", "sgv": 180, "date": now - 30_000}
    ));
    let batch = sync.poll().await.unwrap();
    assert_eq!(batch.events.len(), 1, "{:?}", batch.events);
}
