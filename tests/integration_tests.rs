use chrono::Utc;
use cinnamon::client::NightscoutClient;
use cinnamon::models::entries::SgvEntry;
use cinnamon::models::properties::PropertyType;
use cinnamon::models::treatments::Treatment;
use cinnamon::models::trends::Trend;
use cinnamon::query_builder::Device;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn get_client(mock_server: &MockServer) -> NightscoutClient {
    NightscoutClient::new(&mock_server.uri())
        .expect("Failed to create client")
        .with_secret("test-secret-123")
}

#[tokio::test]
async fn test_profile_get() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let mock_profile = json!([{
        "_id": "653b6...",
        "defaultProfile": "Default",
        "startDate": "2023-01-01T00:00:00.000Z",
        "created_at": "2023-01-01T00:00:00.000Z",
        "store": {
            "Default": {
                "dia": 3.0,
                "timezone": "UTC",
                "units": "mg/dl",
                "carbratio": [{"time": "00:00", "value": 10.0}],
                "sens": [{"time": "00:00", "value": 30.0}],
                "basal": [{"time": "00:00", "value": 1.5}],
                "target_low": [{"time": "00:00", "value": 80.0}],
                "target_high": [{"time": "00:00", "value": 120.0}]
            }
        }
    }]);

    Mock::given(method("GET"))
        .and(path("/api/v2/profile.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_profile))
        .mount(&mock_server)
        .await;

    let profiles = client
        .profiles()
        .get()
        .await
        .expect("Failed to fetch profiles");
    assert!(!profiles.is_empty());
    assert_eq!(profiles[0].default_profile_name, "Default");
}

#[tokio::test]
async fn test_sgv_get_limit() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let mock_sgvs = json!([
        {
            "_id": "1",
            "sgv": 120,
            "date": 1698393600000i64,
            "dateString": "2023-10-27T10:00:00Z",
            "direction": "Flat",
            "type": "sgv",
            "device": "xDrip"
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v2/entries/sgv.json"))
        .and(query_param("count", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_sgvs))
        .mount(&mock_server)
        .await;

    let result = client
        .sgv()
        .get()
        .limit(5)
        .send()
        .await
        .expect("Failed to get SGV");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sgv, 120);
}

#[tokio::test]
async fn test_sgv_create() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let new_entry = SgvEntry::new(150, Trend::SingleUp, Utc::now());
    let entries_vec = vec![new_entry.clone()];

    Mock::given(method("POST"))
        .and(path("/api/v2/entries.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([new_entry])))
        .mount(&mock_server)
        .await;

    let created = client
        .sgv()
        .create(entries_vec)
        .await
        .expect("Failed to create SGV");
    assert_eq!(created[0].sgv, 150);
}

#[tokio::test]
async fn test_sgv_delete_by_id() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let entry_id = "test-id-123";

    Mock::given(method("GET"))
        .and(path(format!("/api/v2/entries/sgv.json/{}", entry_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{ "sgv": 100, "date": 0, "dateString": "", "direction": "Flat", "type": "sgv" }])))
        .mount(&mock_server)
        .await;

    Mock::given(method("DELETE"))
        .and(path(format!("/api/v2/entries/sgv.json/{}", entry_id)))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let result = client.sgv().delete().id(entry_id).send().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_treatments_create_and_read() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let mock_treatment = json!({
        "eventType": "Correction Bolus",
        "created_at": "2023-10-27T10:00:00Z",
        "insulin": 2.5,
        "enteredBy": "TestRunner"
    });

    Mock::given(method("POST"))
        .and(path("/api/v2/treatments.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([mock_treatment])))
        .mount(&mock_server)
        .await;

    let treatment_obj: Treatment = serde_json::from_value(mock_treatment.clone()).unwrap();
    let created = client
        .treatments()
        .create(vec![treatment_obj])
        .await
        .expect("Failed to create treatment");
    assert_eq!(created[0].insulin, Some(2.5));

    Mock::given(method("GET"))
        .and(path("/api/v2/treatments.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([mock_treatment])))
        .mount(&mock_server)
        .await;

    let fetched = client
        .treatments()
        .get()
        .send()
        .await
        .expect("Failed to get treatments");
    assert_eq!(fetched[0].event_type, "Correction Bolus");
}

#[tokio::test]
async fn test_properties_filter() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let mock_props = json!({
        "iob": {
            "iob": 1.25,
            "activity": 0.1,
            "source": "openaps",
            "display": "IOB: 1.25 U",
            "displayLine": "1.25 U"
        },
        "cob": {
            "cob": 30.0,
            "isDecaying": 1,
            "decayedBy": "test",
            "source": "openaps",
            "display": {},
            "displayLine": "30g"
        }
    });

    Mock::given(method("GET"))
        .and(path("/api/v2/properties/iob,cob"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_props))
        .mount(&mock_server)
        .await;

    let result = client
        .properties()
        .get()
        .only(&[PropertyType::Iob, PropertyType::Cob])
        .send()
        .await
        .expect("Failed to fetch properties");

    assert!(result.iob.is_some());
    assert!(result.cob.is_some());
    assert!(result.basal.is_none());
    assert_eq!(result.iob.unwrap().iob, 1.25);
}

#[tokio::test]
async fn test_devicestatus_custom_device() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let mock_ds = json!([{
        "device": "MyPump",
        "created_at": "2023-10-27T10:00:00Z",
        "pump": { "battery": { "percent": 50 } }
    }]);

    Mock::given(method("GET"))
        .and(path("/api/v2/devicestatus.json"))
        .and(query_param("find[device]", "MyPump"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_ds))
        .mount(&mock_server)
        .await;

    let result = client
        .devicestatus()
        .get()
        .device(Device::Custom("MyPump".to_string()))
        .send()
        .await
        .expect("Failed to fetch devicestatus");

    assert_eq!(result[0].device, Some("MyPump".to_string()));
}

#[tokio::test]
async fn test_query_builder_auto_device() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let probe_response = json!([{
        "_id": "probe1",
        "sgv": 100,
        "date": 1000,
        "dateString": "now",
        "direction": "Flat",
        "type": "sgv",
        "device": "FoundDeviceName"
    }]);

    Mock::given(method("GET"))
        .and(path("/api/v2/entries/sgv.json"))
        .and(query_param("count", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(probe_response))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    let actual_response = json!([
        {
            "_id": "real1",
            "sgv": 110,
            "date": 2000,
            "dateString": "later",
            "direction": "Flat",
            "type": "sgv",
            "device": "FoundDeviceName"
        },
        {
            "_id": "real2",
            "sgv": 115,
            "date": 3000,
            "dateString": "later2",
            "direction": "Flat",
            "type": "sgv",
            "device": "FoundDeviceName"
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v2/entries/sgv.json"))
        .and(query_param("count", "10"))
        .and(query_param("find[device]", "FoundDeviceName"))
        .respond_with(ResponseTemplate::new(200).set_body_json(actual_response))
        .mount(&mock_server)
        .await;

    let result = client
        .sgv()
        .get()
        .device(Device::Auto)
        .limit(10)
        .send()
        .await
        .expect("Auto device fetch failed");

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].device.as_deref(), Some("FoundDeviceName"));
}

#[tokio::test]
async fn test_mbg_latest() {
    let mock_server = MockServer::start().await;
    let client = get_client(&mock_server).await;

    let mock_mbg = json!([
        {
            "_id": "m1",
            "mbg": 105,
            "date": 1000,
            "dateString": "now",
            "type": "mbg",
            "device": "Contour"
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v2/entries/mbg.json"))
        .and(query_param("count", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_mbg))
        .mount(&mock_server)
        .await;

    let entry = client
        .mbg()
        .latest()
        .await
        .expect("Failed to fetch latest MBG");
    assert_eq!(entry.mbg, 105);
}
