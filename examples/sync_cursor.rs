//! Mirrors treatments into a JSON file, resuming from a saved cursor.
//!
//! ```sh
//! NS_URL=https://my-ns.example.com NS_TOKEN=myapp-0123456789abcdef cargo run --example sync_cursor
//! ```
#![allow(clippy::print_stdout)]

use std::collections::BTreeMap;
use std::{env, fs};

use cinnamon::model::CollectionName;
use cinnamon::sync::{SyncCursor, SyncEvent};
use cinnamon::{Client, Credentials};

const STATE: &str = "treatments-mirror.json";

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Mirror {
    cursor: SyncCursor,
    docs: BTreeMap<String, serde_json::Value>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("NS_URL")?;
    let token = env::var("NS_TOKEN")?;
    let ns = Client::connect(&url, Credentials::access_token(token)).await?;

    let mut mirror: Mirror = fs::read(STATE)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();

    let mut sync = ns
        .sync(mirror.cursor.clone())
        .collections([CollectionName::Treatments])
        .backfill(chrono::Duration::days(7));

    let batch = sync.poll().await?;
    let (mut upserts, mut deletes) = (0, 0);
    for event in batch.events {
        match event {
            SyncEvent::Upsert(doc) => {
                if let Some(id) = doc.identifier() {
                    mirror.docs.insert(id.to_owned(), doc.doc.clone());
                    upserts += 1;
                }
            }
            SyncEvent::Delete { identifier, .. } => {
                mirror.docs.remove(&identifier);
                deletes += 1;
            }
            _ => {}
        }
    }
    mirror.cursor = batch.cursor;
    fs::write(STATE, serde_json::to_vec_pretty(&mirror)?)?;
    println!(
        "{upserts} upserts, {deletes} deletes; {} treatments mirrored in {STATE}",
        mirror.docs.len()
    );
    Ok(())
}
