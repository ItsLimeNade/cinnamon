//! Incremental synchronization: mirror Nightscout collections into your own store.
//!
//! ```no_run
//! # async fn run(ns: cinnamon::Client) -> cinnamon::Result<()> {
//! use cinnamon::sync::{SyncCursor, SyncEvent};
//!
//! let mut sync = ns.sync(SyncCursor::default());
//! loop {
//!     let batch = sync.poll().await?;
//!     for event in batch.events {
//!         match event {
//!             SyncEvent::Upsert(doc) => println!("upsert {} {:?}", doc.collection, doc.identifier()),
//!             SyncEvent::Delete { collection, identifier } => println!("delete {collection} {identifier}"),
//!             _ => {}
//!         }
//!     }
//!     // Persist `batch.cursor` (it is `Serialize`) to resume after a restart.
//!     tokio::time::sleep(std::time::Duration::from_secs(60)).await;
//! }
//! # }
//! ```
//!
//! How it works, with API v3:
//! 1. `GET /api/v3/lastModified` tells which collections changed (one tiny request).
//! 2. On the first run, recent documents are fetched by date (see [`Sync::backfill`]).
//! 3. Afterwards, `GET /api/v3/{collection}/history/{cursor}` returns creations, updates and
//!    deletions since the cursor.
//! 4. Documents uploaded through API v1 by other apps (xDrip+, Loop, Trio, OpenAPS, …) never
//!    get a `srvModified` and are invisible to history, so each poll also searches by date
//!    for documents newer than the last one seen.
//!
//! Without API v3 only step 4 runs, so updates and deletions are not detected.
//!
//! Delivery is **at least once**: apply events idempotently, keyed by identifier.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::collection::{ListSpec, fetch_raw, query_time, raw_id};
use crate::client::transport::{Request, decode_value};
use crate::client::{Api, Client, Support};
use crate::error::{Error, Result};
use crate::model::system::LastModified;
use crate::model::{CollectionName, Document, Timestamp};
use crate::query::Filter;

const PAGE: usize = 500;
/// Re-scan this far behind the newest date seen, to catch late uploads with older dates.
const DATE_OVERLAP_MS: i64 = 10 * 60 * 1000;
/// Dates further than this in the future are ignored for the cursor (a single mis-dated
/// document would otherwise pin it forever).
const FUTURE_TOLERANCE_MS: i64 = 60 * 60 * 1000;
/// Safety cap on pages per collection per poll.
const MAX_PAGES: usize = 200;

/// Where a sync left off. Persist it (it is `Serialize`) and pass it back to
/// [`Client::sync`] to resume.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SyncCursor {
    /// Per-collection progress.
    #[serde(default)]
    pub collections: BTreeMap<CollectionName, CollectionCursor>,
}

/// Progress for one collection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CollectionCursor {
    /// Newest `srvModified` processed through history (API v3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Timestamp>,
    /// Newest document date seen by date search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Timestamp>,
}

/// A change to apply to your mirror.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SyncEvent {
    /// A document was created or changed.
    Upsert(SyncDoc),
    /// A document was deleted (API v3 only).
    Delete {
        /// Its collection.
        collection: CollectionName,
        /// Its identifier.
        identifier: String,
    },
}

/// A synchronized document, kept as JSON until you decode it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SyncDoc {
    /// The collection it belongs to.
    pub collection: CollectionName,
    /// The raw document.
    pub doc: Value,
}

impl SyncDoc {
    /// The document identifier (`identifier`, else `_id`).
    #[must_use]
    pub fn identifier(&self) -> Option<&str> {
        raw_id(&self.doc)
    }

    /// Decodes into a typed model, e.g. `doc.decode::<Treatment>()` or
    /// `doc.decode::<Entry>()`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] when `T` belongs to another collection, [`Error::Decode`]
    /// when the document does not fit `T`.
    pub fn decode<T: Document>(&self) -> Result<T> {
        if T::COLLECTION != self.collection {
            return Err(Error::InvalidInput(format!(
                "a {} document cannot be decoded as a {} model",
                self.collection,
                T::COLLECTION
            )));
        }
        decode_value(self.doc.clone())
    }
}

/// One round of changes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SyncBatch {
    /// Changes, per collection in order of modification.
    pub events: Vec<SyncEvent>,
    /// The cursor after applying `events`; persist it.
    pub cursor: SyncCursor,
}

/// An incremental sync session: `ns.sync(cursor)`.
#[derive(Debug, Clone)]
#[must_use]
pub struct Sync {
    client: Client,
    cursor: SyncCursor,
    collections: Vec<CollectionName>,
    backfill: Duration,
    /// Identifiers already emitted at the current history boundary, per collection.
    boundary: BTreeMap<CollectionName, HashSet<String>>,
    /// Identifiers emitted recently (with their date), so the date-search overlap does not
    /// re-emit them on every poll.
    recent: BTreeMap<CollectionName, HashMap<String, i64>>,
}

impl Client {
    /// Starts (or resumes, from `cursor`) an incremental sync of entries, treatments,
    /// device status, profiles and food.
    pub fn sync(&self, cursor: SyncCursor) -> Sync {
        Sync {
            client: self.clone(),
            cursor,
            collections: vec![
                CollectionName::Entries,
                CollectionName::Treatments,
                CollectionName::DeviceStatus,
                CollectionName::Profile,
                CollectionName::Food,
            ],
            backfill: Duration::days(1),
            boundary: BTreeMap::new(),
            recent: BTreeMap::new(),
        }
    }
}

impl Sync {
    /// Which collections to sync. `Settings` needs `api:settings:admin`; `Activity` only
    /// exists in API v1.
    pub fn collections<I: IntoIterator<Item = CollectionName>>(mut self, collections: I) -> Self {
        self.collections = collections.into_iter().collect();
        self
    }

    /// How far back the first run fetches (default one day).
    pub const fn backfill(mut self, window: Duration) -> Self {
        self.backfill = window;
        self
    }

    /// The current cursor.
    #[must_use]
    pub const fn cursor(&self) -> &SyncCursor {
        &self.cursor
    }

    /// Fetches everything that changed since the last poll.
    ///
    /// # Errors
    ///
    /// Transport and authorization errors. A failed (or cancelled) poll leaves the session
    /// untouched, so it can simply be retried.
    pub async fn poll(&mut self) -> Result<SyncBatch> {
        // Work on copies and commit only once every collection succeeded: committing per
        // collection would advance past events that a later failure never delivers.
        let mut cursors = self.cursor.clone();
        let mut boundaries = self.boundary.clone();
        let mut recents = self.recent.clone();

        let inner = &self.client.inner;
        let v3 = inner.pick_api(Support::Both, "sync").await? == Api::V3;
        let last_modified = if v3 {
            Some(
                inner
                    .execute(Request::get("api/v3/lastModified", "api/v3/lastModified"))
                    .await?
                    .v3::<LastModified>()?,
            )
        } else {
            None
        };

        let mut events = Vec::new();
        for name in self.collections.clone() {
            let api = match name {
                CollectionName::Activity => Api::V1,
                CollectionName::Settings if !v3 => continue,
                _ if v3 => Api::V3,
                _ => Api::V1,
            };
            let mut cursor = cursors.collections.get(&name).copied().unwrap_or_default();
            let recent = recents.entry(name).or_default();

            if api == Api::V3 {
                match cursor.history {
                    None => {
                        // Start history at the server's clock; the date search below does
                        // the backfill.
                        cursor.history = last_modified.as_ref().and_then(|l| l.srv_date);
                        if cursor.date.is_none() {
                            cursor.date = Some(self.backfill_start());
                        }
                    }
                    Some(since) => {
                        let changed = last_modified
                            .as_ref()
                            .and_then(|l| l.collections.get(name.name()))
                            .is_none_or(|newest| *newest > since);
                        if changed {
                            let boundary = boundaries.entry(name).or_default();
                            self.pull_history(name, &mut cursor, boundary, recent, &mut events)
                                .await?;
                        }
                    }
                }
            } else if cursor.date.is_none() {
                cursor.date = Some(self.backfill_start());
            }

            self.pull_by_date(name, api, &mut cursor, recent, &mut events)
                .await?;
            cursors.collections.insert(name, cursor);
        }

        self.cursor = cursors;
        self.boundary = boundaries;
        self.recent = recents;
        Ok(SyncBatch {
            events,
            cursor: self.cursor.clone(),
        })
    }

    fn backfill_start(&self) -> Timestamp {
        Timestamp::from(chrono::Utc::now() - self.backfill)
    }

    async fn pull_history(
        &self,
        name: CollectionName,
        cursor: &mut CollectionCursor,
        boundary: &mut HashSet<String>,
        recent: &mut HashMap<String, i64>,
        events: &mut Vec<SyncEvent>,
    ) -> Result<()> {
        let inner = &self.client.inner;
        for _ in 0..MAX_PAGES {
            let Some(since) = cursor.history else { break };
            // `history/{t}` is exclusive; ask from t-1 and drop what was already emitted at t
            // so documents sharing the boundary millisecond are neither lost nor repeated.
            let path = format!(
                "api/v3/{}/history/{}",
                name.name(),
                since.as_millis().saturating_sub(1)
            );
            let docs: Vec<Value> = inner
                .execute(
                    Request::get(path, "api/v3/{collection}/history/{since}")
                        .query("limit", PAGE.to_string()),
                )
                .await?
                .v3()?;
            let full = docs.len() >= PAGE;
            let mut newest = since;
            let mut fresh = 0usize;
            for doc in docs {
                let modified = doc.get("srvModified").and_then(Timestamp::from_value);
                let id = identifier(&doc);
                if modified == Some(since) && id.as_ref().is_some_and(|id| boundary.contains(id)) {
                    continue;
                }
                if let Some(m) = modified {
                    if m > newest {
                        newest = m;
                        boundary.clear();
                    }
                    if m == newest {
                        boundary.extend(id.clone());
                    }
                }
                fresh += 1;
                let deleted = doc.get("isValid").and_then(Value::as_bool) == Some(false);
                if let Some(id) = &id {
                    if deleted {
                        recent.remove(id);
                    } else {
                        let date =
                            query_time(&doc, name, Api::V3).map_or(i64::MAX, Timestamp::as_millis);
                        recent.insert(id.clone(), date);
                    }
                }
                events.push(match (deleted, id) {
                    (true, Some(identifier)) => SyncEvent::Delete {
                        collection: name,
                        identifier,
                    },
                    _ => SyncEvent::Upsert(SyncDoc {
                        collection: name,
                        doc,
                    }),
                });
            }
            cursor.history = Some(newest);
            // Stop on an empty page or when the cursor cannot advance (Nightscout #8096).
            if fresh == 0 || !full || newest == since {
                break;
            }
        }
        Ok(())
    }

    async fn pull_by_date(
        &self,
        name: CollectionName,
        api: Api,
        cursor: &mut CollectionCursor,
        recent: &mut HashMap<String, i64>,
        events: &mut Vec<SyncEvent>,
    ) -> Result<()> {
        let Some(from) = cursor.date else {
            return Ok(());
        };
        let horizon = Timestamp::now().as_millis() + FUTURE_TOLERANCE_MS;
        let mut since = Timestamp::from_millis(from.as_millis().saturating_sub(DATE_OVERLAP_MS));
        let mut newest = from;
        for _ in 0..MAX_PAGES {
            let spec = ListSpec {
                filter: Filter::new(),
                since: Some(since),
                until: None,
                limit: PAGE,
                skip: 0,
                ascending: true,
                page_size: PAGE,
            };
            let docs = fetch_raw(&self.client, name, &spec, api).await?;
            let full = docs.len() >= PAGE;
            let mut page_max = since;
            for doc in docs {
                let date = query_time(&doc, name, api);
                if let Some(d) = date {
                    page_max = page_max.max(d);
                    if d.as_millis() <= horizon {
                        newest = newest.max(d);
                    }
                }
                let Some(id) = identifier(&doc) else { continue };
                if recent.contains_key(&id) {
                    continue;
                }
                recent.insert(id, date.map_or(i64::MAX, Timestamp::as_millis));
                events.push(SyncEvent::Upsert(SyncDoc {
                    collection: name,
                    doc,
                }));
            }
            // A full page of already-seen documents (a busy overlap window) is no reason to
            // stop: only a page that cannot move the bound forward is.
            if !full || page_max == since {
                break;
            }
            since = page_max;
        }
        cursor.date = Some(newest);
        // Forget what the next overlap window can no longer return.
        let keep_after = newest.as_millis().saturating_sub(DATE_OVERLAP_MS);
        recent.retain(|_, date| *date >= keep_after);
        Ok(())
    }
}

fn identifier(doc: &Value) -> Option<String> {
    raw_id(doc).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_serializes_compactly() {
        let mut cursor = SyncCursor::default();
        cursor.collections.insert(
            CollectionName::Treatments,
            CollectionCursor {
                history: Some(Timestamp::from_millis(1_714_564_800_000)),
                date: None,
            },
        );
        let json = serde_json::to_string(&cursor).unwrap_or_default();
        assert_eq!(
            json,
            r#"{"collections":{"treatments":{"history":1714564800000}}}"#
        );
        let back: SyncCursor = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back, cursor);
    }
}
