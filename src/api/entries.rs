//! The `entries` collection and its API v1 query helpers.

use serde_json::Value;

use super::Collection;
use crate::client::Client;
use crate::client::transport::{Request, encode_segment};
use crate::error::Result;
use crate::model::{Cal, CollectionName, Entry, Mbg, Sgv, Timestamp};
use crate::query::Filter;

/// Access to glucose entries: `ns.entries()`.
#[derive(Debug, Clone)]
pub struct Entries {
    client: Client,
}

impl Entries {
    pub(crate) const fn new(client: Client) -> Self {
        Self { client }
    }

    /// Sensor glucose values.
    #[must_use]
    pub fn sgv(&self) -> Collection<Sgv> {
        Collection::new(self.client.clone(), Filter::new().eq("type", "sgv"))
    }

    /// Meter (fingerstick) glucose values.
    #[must_use]
    pub fn mbg(&self) -> Collection<Mbg> {
        Collection::new(self.client.clone(), Filter::new().eq("type", "mbg"))
    }

    /// Sensor calibrations.
    #[must_use]
    pub fn cal(&self) -> Collection<Cal> {
        Collection::new(self.client.clone(), Filter::new().eq("type", "cal"))
    }

    /// Entries of every type.
    #[must_use]
    pub fn all(&self) -> Collection<Entry> {
        Collection::new(self.client.clone(), Filter::new())
    }

    /// The newest entry of any type in the last four days (`GET /api/v1/entries/current`).
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn current(&self) -> Result<Option<Entry>> {
        let resp = self
            .client
            .inner
            .execute(Request::get(
                "api/v1/entries/current.json",
                "api/v1/entries/current",
            ))
            .await?;
        let entries: Vec<Entry> = resp.json()?;
        Ok(entries.into_iter().next())
    }

    /// Entries whose `dateString` matches `prefix` and a brace-expanded `pattern`
    /// (`GET /api/v1/times/{prefix}/{pattern}`), e.g. `times("2024-04", "T{13..18}:{00..15}")`.
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn times(&self, prefix: &str, pattern: &str, count: usize) -> Result<Vec<Entry>> {
        let path = format!(
            "api/v1/times/{}/{}.json",
            encode_segment(prefix),
            encode_segment(pattern)
        );
        let resp = self
            .client
            .inner
            .execute(
                Request::get(path, "api/v1/times/{prefix}/{regex}")
                    .query("count", count.to_string()),
            )
            .await?;
        Ok(super::collection::decode_docs(resp.json()?))
    }

    /// Documents of `storage` whose `field` matches `prefix` and a brace-expanded `pattern`
    /// (`GET /api/v1/slice/{storage}/{field}/{type}/{prefix}/{pattern}`).
    ///
    /// # Errors
    ///
    /// Transport and authorization errors.
    pub async fn slice(
        &self,
        storage: CollectionName,
        field: &str,
        entry_type: &str,
        prefix: &str,
        pattern: &str,
        count: usize,
    ) -> Result<Vec<Value>> {
        let path = format!(
            "api/v1/slice/{}/{}/{}/{}/{}.json",
            storage.name(),
            encode_segment(field),
            encode_segment(entry_type),
            encode_segment(prefix),
            encode_segment(pattern)
        );
        self.client
            .inner
            .execute(
                Request::get(
                    path,
                    "api/v1/slice/{storage}/{field}/{type}/{prefix}/{regex}",
                )
                .query("count", count.to_string()),
            )
            .await?
            .json()
    }

    /// Counts entries matching `filter` server-side (`GET /api/v1/count/entries/where`).
    ///
    /// # Errors
    ///
    /// Transport and authorization errors.
    pub async fn count(&self, filter: &Filter) -> Result<u64> {
        let mut filter = filter.clone();
        if !filter.mentions("date") && !filter.mentions("dateString") {
            // Without a date term Nightscout adds a default window that matches nothing.
            filter = filter.gte("date", Timestamp::EPOCH);
        }
        let rows: Vec<Value> = self
            .client
            .inner
            .execute(
                Request::get("api/v1/count/entries/where", "api/v1/count/{storage}/where")
                    .queries(filter.v1_pairs()),
            )
            .await?
            .json()?;
        Ok(rows
            .iter()
            .filter_map(|row| row.get("count").and_then(Value::as_u64))
            .sum())
    }

    /// Shows the MongoDB query Nightscout would run for `filter` against `storage`
    /// (`GET /api/v1/echo/{storage}`); useful for debugging filters.
    ///
    /// # Errors
    ///
    /// Transport and authorization errors.
    pub async fn echo(&self, storage: CollectionName, filter: &Filter) -> Result<Value> {
        self.client
            .inner
            .execute(
                Request::get(
                    format!("api/v1/echo/{}", storage.name()),
                    "api/v1/echo/{storage}",
                )
                .queries(filter.v1_pairs()),
            )
            .await?
            .json()
    }
}
