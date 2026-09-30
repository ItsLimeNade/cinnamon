//! Transport behavior that a healthy Nightscout cannot easily produce: retries, redirects,
//! JWT refresh, credential placement and decode errors. Real API semantics are covered by
//! the e2e suite (`tests/e2e.rs`) instead of mocks.
#![allow(clippy::unwrap_used, missing_docs)]

use std::time::Duration;

use cinnamon::{ApiVersion, Client, Error, RetryPolicy};
use serde_json::json;
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
