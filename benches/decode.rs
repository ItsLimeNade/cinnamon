//! Decoding throughput of the lenient models: `cargo bench --bench decode`.
#![allow(missing_docs, clippy::unwrap_used, clippy::print_stdout)]

use std::time::Instant;

use cinnamon::model::{Sgv, Treatment};

fn main() {
    let sgvs: Vec<serde_json::Value> = (0..10_000)
        .map(|i| {
            serde_json::json!({
                "_id": format!("{i:024x}"), "device": "xDrip-DexcomG6", "date": 1_714_564_800_000_i64 + i * 300_000,
                "dateString": "2024-05-01T14:00:00.000+0200", "sgv": 100 + i % 150, "delta": -2.5,
                "direction": "Flat", "type": "sgv", "filtered": 131_000, "unfiltered": 131_000,
                "rssi": 100, "noise": 1, "sysTime": "2024-05-01T14:00:00.000+0200", "utcOffset": 120
            })
        })
        .collect();
    let body = serde_json::to_vec(&sgvs).unwrap();
    let treatments: Vec<serde_json::Value> = (0..10_000)
        .map(|i| serde_json::json!({
            "_id": format!("{i:024x}"), "eventType": "Correction Bolus", "created_at": "2024-05-01T12:00:00.000Z",
            "insulin": 0.35, "isSMB": true, "pumpId": i, "utcOffset": 120, "app": "AAPS"
        }))
        .collect();
    let tbody = serde_json::to_vec(&treatments).unwrap();

    for _ in 0..3 {
        let t = Instant::now();
        let raw: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        let parsed: Vec<Sgv> = raw
            .into_iter()
            .map(|v| serde_json::from_value(v).unwrap())
            .collect();
        let sgv = t.elapsed();
        let t = Instant::now();
        let raw: Vec<serde_json::Value> = serde_json::from_slice(&tbody).unwrap();
        let tparsed: Vec<Treatment> = raw
            .into_iter()
            .map(|v| serde_json::from_value(v).unwrap())
            .collect();
        let tre = t.elapsed();
        println!(
            "10k sgv: {:>6.1} ms ({:.0}/s) | 10k treatments: {:>6.1} ms | {} {}",
            sgv.as_secs_f64() * 1e3,
            10_000.0 / sgv.as_secs_f64(),
            tre.as_secs_f64() * 1e3,
            parsed.len(),
            tparsed.len()
        );
    }
}
