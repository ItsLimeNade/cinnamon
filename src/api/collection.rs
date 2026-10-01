//! The generic handle for a Nightscout collection: list, stream, get, create, update,
//! patch, delete and history, over API v3 with API v1 fallback.

use std::collections::HashSet;
use std::future::IntoFuture;
use std::marker::PhantomData;

use futures_util::future::BoxFuture;
use futures_util::stream::{self, BoxStream, StreamExt, TryStreamExt};
use serde::Deserialize;
use serde_json::Value;

use crate::client::transport::{Request, Response, decode_value, encode_segment};
use crate::client::{Api, Client, Support};
use crate::error::{Error, Result};
use crate::model::{CollectionName, Document, Timestamp, time};
use crate::query::{Filter, Op};

/// API v3 refuses larger pages (`API3_MAX_LIMIT`, default 1000).
const V3_MAX_LIMIT: usize = 1000;
/// Nightscout accepts at most 10 000 documents per API v1 write.
const V1_MAX_BATCH: usize = 1000;
/// Default page size for streams.
const DEFAULT_PAGE_SIZE: usize = 500;
/// Parallel requests used for one-by-one writes and deletes.
const DEFAULT_CONCURRENCY: usize = 4;

/// How a collection is reached through API v1.
struct V1Routes {
    /// `GET` list endpoint.
    list: &'static str,
    /// Whether the list endpoint understands `find[...]` and `count`.
    find: bool,
    /// `POST` / `PUT` / bulk `DELETE` endpoint.
    write: &'static str,
    /// Whether `PUT` (replace) is supported.
    put: bool,
    /// Whether a single document is deleted through `DELETE write/{id}` (otherwise through
    /// `DELETE write?find[_id]=…`).
    delete_by_path: bool,
    /// Whether a v1 `POST` deduplicates, making it safe to retry.
    dedup_on_post: bool,
}

const fn v1_routes(name: CollectionName) -> Option<V1Routes> {
    Some(match name {
        CollectionName::Entries => V1Routes {
            list: "api/v1/entries.json",
            find: true,
            write: "api/v1/entries",
            put: false,
            delete_by_path: false,
            dedup_on_post: true,
        },
        CollectionName::Treatments => V1Routes {
            list: "api/v1/treatments.json",
            find: true,
            write: "api/v1/treatments",
            put: true,
            delete_by_path: false,
            dedup_on_post: true,
        },
        CollectionName::DeviceStatus => V1Routes {
            list: "api/v1/devicestatus.json",
            find: true,
            write: "api/v1/devicestatus",
            put: false,
            delete_by_path: false,
            dedup_on_post: false,
        },
        CollectionName::Profile => V1Routes {
            list: "api/v1/profile.json",
            find: false,
            write: "api/v1/profile",
            put: true,
            delete_by_path: true,
            dedup_on_post: false,
        },
        CollectionName::Food => V1Routes {
            list: "api/v1/food.json",
            find: false,
            write: "api/v1/food",
            put: true,
            delete_by_path: true,
            dedup_on_post: false,
        },
        CollectionName::Activity => V1Routes {
            list: "api/v1/activity.json",
            find: true,
            write: "api/v1/activity",
            put: true,
            delete_by_path: true,
            dedup_on_post: false,
        },
        CollectionName::Settings => return None,
    })
}

const fn support(name: CollectionName) -> Support {
    match name {
        CollectionName::Settings => Support::V3Only,
        CollectionName::Activity => Support::V1Only,
        _ => Support::Both,
    }
}

fn no_v1(name: CollectionName) -> Error {
    Error::Unsupported {
        feature: "API v1 access",
        reason: format!("the {name} collection only exists in API v3"),
    }
}

/// A handle on one Nightscout collection.
///
/// Obtained from [`Client`], e.g. `ns.treatments()` or `ns.entries().sgv()`.
#[derive(Debug, Clone)]
pub struct Collection<T> {
    client: Client,
    /// Conditions every query of this handle adds (e.g. `type == "sgv"`).
    base: Filter,
    _doc: PhantomData<fn() -> T>,
}

impl<T: Document> Collection<T> {
    pub(crate) const fn new(client: Client, base: Filter) -> Self {
        Self {
            client,
            base,
            _doc: PhantomData,
        }
    }

    const fn name() -> CollectionName {
        T::COLLECTION
    }

    async fn api(&self) -> Result<Api> {
        self.client
            .inner
            .pick_api(support(Self::name()), "this operation")
            .await
    }

    async fn v3_only(&self, feature: &'static str) -> Result<()> {
        self.client
            .inner
            .pick_api(Support::V3Only, feature)
            .await
            .map(|_| ())
    }

    /// Starts a query, newest first, 10 documents by default.
    ///
    /// ```no_run
    /// # async fn run(ns: cinnamon::Client) -> cinnamon::Result<()> {
    /// use chrono::{Duration, Utc};
    ///
    /// let day = ns.entries().sgv().list().since(Utc::now() - Duration::hours(24)).limit(288).await?;
    /// # Ok(()) }
    /// ```
    pub fn list(&self) -> List<T> {
        List {
            client: self.client.clone(),
            spec: ListSpec {
                filter: self.base.clone(),
                since: None,
                until: None,
                limit: 10,
                skip: 0,
                ascending: false,
                page_size: DEFAULT_PAGE_SIZE,
            },
            _doc: PhantomData,
        }
    }

    /// The newest document, if any.
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn latest(&self) -> Result<Option<T>> {
        Ok(self.list().limit(1).send().await?.into_iter().next())
    }

    /// Reads one document by its `identifier` (API v3) or `_id` (API v1).
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`], [`Error::Gone`] for soft-deleted documents, and transport errors.
    pub async fn get(&self, id: &str) -> Result<T> {
        let id = checked_id(id)?;
        let name = Self::name();
        match self.api().await? {
            Api::V3 => {
                let resp = self
                    .client
                    .inner
                    .execute(Request::get(
                        format!("api/v3/{}/{}", name.name(), encode_segment(id)),
                        "api/v3/{collection}/{id}",
                    ))
                    .await?;
                resp.v3()
            }
            Api::V1 => {
                let routes = v1_routes(name).ok_or_else(|| no_v1(name))?;
                let docs: Vec<Value> = if routes.find {
                    self.client
                        .inner
                        .execute(
                            Request::get(routes.list, "api/v1/{collection}")
                                .query(v1_id_key(id), id)
                                .query("count", "1"),
                        )
                        .await?
                        .json()?
                } else {
                    let all: Vec<Value> = self
                        .client
                        .inner
                        .execute(
                            Request::get(routes.list, "api/v1/{collection}")
                                .query("count", "10000"),
                        )
                        .await?
                        .json()?;
                    all.into_iter()
                        .filter(|d| {
                            d.get("_id").and_then(Value::as_str) == Some(id)
                                || d.get("identifier").and_then(Value::as_str) == Some(id)
                        })
                        .collect()
                };
                let doc = docs.into_iter().next().ok_or(Error::NotFound)?;
                decode_value(doc)
            }
        }
    }

    /// Uploads a document. Missing `app`, `device` (the client's app name), `utcOffset`,
    /// `date`/`created_at` and the API v3 `identifier` are filled in, so a retried upload
    /// deduplicates instead of duplicating.
    ///
    /// Through API v3, re-uploading an existing document replaces it and needs the
    /// `update` permission in addition to `create`.
    ///
    /// # Errors
    ///
    /// [`Error::Forbidden`] / [`Error::Unauthorized`] without write permission,
    /// [`Error::ApiDisabled`] when the server does not accept v1 writes, and transport errors.
    pub async fn create(&self, doc: &T) -> Result<Created> {
        let mut created = self.create_many(std::slice::from_ref(doc)).await?;
        created.pop().ok_or_else(|| Error::Server {
            status: 200,
            message: "Nightscout returned no result for the created document".into(),
        })
    }

    /// Uploads several documents: one request per document through API v3 (bounded
    /// concurrency, results in input order), batched requests through API v1.
    ///
    /// # Errors
    ///
    /// The first failure; documents before it may already be stored.
    pub async fn create_many(&self, docs: &[T]) -> Result<Vec<Created>> {
        let name = Self::name();
        let app = self.client.app_name().to_owned();
        let prepared: Vec<T> = docs
            .iter()
            .cloned()
            .map(|mut d| {
                d.prepare_for_upload(&app);
                d
            })
            .collect();

        match self.api().await? {
            Api::V3 => {
                let path = format!("api/v3/{}", name.name());
                let requests = prepared.into_iter().map(|doc| {
                    let inner = &self.client.inner;
                    let path = path.clone();
                    async move {
                        let body = v3_body(&doc, false)?;
                        let resp = inner
                            .execute(
                                Request::post(path, "api/v3/{collection}")
                                    .json(&body)?
                                    .idempotent(true),
                            )
                            .await?;
                        let raw: V3Created = resp.json()?;
                        Ok(Created {
                            identifier: raw
                                .deduplicated_identifier
                                .or(raw.identifier)
                                .or_else(|| doc.meta().identifier.clone()),
                            deduplicated: raw.is_deduplication,
                            last_modified: raw.last_modified,
                        })
                    }
                });
                stream::iter(requests)
                    .buffered(DEFAULT_CONCURRENCY)
                    .try_collect()
                    .await
            }
            Api::V1 => {
                let routes = v1_routes(name).ok_or_else(|| no_v1(name))?;
                let mut out = Vec::with_capacity(prepared.len());
                for chunk in prepared.chunks(V1_MAX_BATCH) {
                    let resp = self
                        .client
                        .inner
                        .execute(
                            Request::post(routes.write, "api/v1/{collection}")
                                .json(chunk)?
                                .idempotent(routes.dedup_on_post),
                        )
                        .await?;
                    let stored: Vec<Value> = resp.json().unwrap_or_default();
                    for (i, doc) in chunk.iter().enumerate() {
                        let echoed = stored.get(i);
                        let stored_id = echoed.and_then(|v| v.get("_id")).and_then(Value::as_str);
                        out.push(Created {
                            identifier: doc
                                .meta()
                                .identifier
                                .clone()
                                .or_else(|| stored_id.map(str::to_owned)),
                            deduplicated: echoed.is_some() && stored_id.is_none(),
                            last_modified: None,
                        });
                    }
                }
                Ok(out)
            }
        }
    }

    /// Writes back a modified document (addressed by its `identifier`, else `_id`).
    ///
    /// Through API v3, documents created by API v3 clients are replaced; legacy documents
    /// (uploaded through API v1, without `app`/`date`/`utcOffset`) cannot be replaced by
    /// v3 and are patched instead, which updates every field you send but does not remove
    /// fields you deleted locally. Through API v1 the document is replaced (treatments,
    /// profiles, food and activity only).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] when the document has no id, [`Error::Unsupported`] for
    /// entries and device status on API v1, [`Error::Gone`] for soft-deleted documents.
    pub async fn update(&self, doc: &T) -> Result<()> {
        let name = Self::name();
        let id = doc
            .id()
            .ok_or_else(|| Error::InvalidInput("the document has no identifier or _id".into()))?
            .to_owned();
        match self.api().await? {
            Api::V3 => {
                let path = format!(
                    "api/v3/{}/{}",
                    name.name(),
                    encode_segment(checked_id(&id)?)
                );
                let mut body = v3_body(doc, true)?;
                let native = ["app", "date", "utcOffset"]
                    .iter()
                    .all(|k| body.contains_key(*k));
                let req = if native {
                    Request::put(path, "api/v3/{collection}/{id}").json(&body)?
                } else {
                    for key in IMMUTABLE_V3_FIELDS {
                        body.remove(*key);
                    }
                    Request::patch(path, "api/v3/{collection}/{id}")
                        .json(&body)?
                        .idempotent(true)
                };
                self.client.inner.execute(req).await.map(|_| ())
            }
            Api::V1 => {
                let routes = v1_routes(name).ok_or_else(|| no_v1(name))?;
                if !routes.put {
                    return Err(Error::Unsupported {
                        feature: "update",
                        reason: format!("API v1 cannot update {name}; use API v3"),
                    });
                }
                let mut doc = doc.clone();
                doc.meta_mut().strip_server_fields();
                self.client
                    .inner
                    .execute(Request::put(routes.write, "api/v1/{collection}").json(&doc)?)
                    .await
                    .map(|_| ())
            }
        }
    }

    /// Sets the given fields on a document (API v3 only). Nested objects are replaced, not
    /// merged.
    ///
    /// # Errors
    ///
    /// [`Error::Unsupported`] on API v1, [`Error::NotFound`], [`Error::Gone`],
    /// [`Error::BadRequest`] when a field is immutable.
    pub async fn patch(&self, id: &str, changes: &Value) -> Result<()> {
        self.v3_only("patch").await?;
        let path = format!(
            "api/v3/{}/{}",
            Self::name().name(),
            encode_segment(checked_id(id)?)
        );
        self.client
            .inner
            .execute(
                Request::patch(path, "api/v3/{collection}/{id}")
                    .json(changes)?
                    .idempotent(true),
            )
            .await
            .map(|_| ())
    }

    /// Deletes one document. Through API v3 this is a soft delete (visible in history)
    /// unless [`Delete::permanent`] is set; API v1 always deletes permanently.
    pub fn delete(&self, id: impl Into<String>) -> Delete<T> {
        Delete {
            collection: self.clone(),
            id: id.into(),
            permanent: false,
        }
    }

    /// Deletes every document matching `filter`. The filter always carries an upper time
    /// bound, so an empty filter can never wipe a collection.
    pub fn delete_many(&self, filter: DeleteFilter) -> DeleteMany<T> {
        DeleteMany {
            collection: self.clone(),
            filter,
            permanent: false,
        }
    }

    /// Documents changed (created, updated or deleted) after `since`, oldest change first
    /// (API v3 only). Soft-deleted documents are included with `isValid: false`.
    ///
    /// Documents written through API v1 by other uploaders do not carry `srvModified` and
    /// never appear here; see [`Client::sync`](crate::Client::sync) for a complete feed.
    ///
    /// # Errors
    ///
    /// [`Error::Unsupported`] without API v3.
    pub async fn history(&self, since: Timestamp, limit: usize) -> Result<HistoryPage<T>> {
        self.v3_only("history").await?;
        let path = format!(
            "api/v3/{}/history/{}",
            Self::name().name(),
            since.as_millis()
        );
        let resp = self
            .client
            .inner
            .execute(
                Request::get(path, "api/v3/{collection}/history/{since}")
                    .query("limit", limit.clamp(1, V3_MAX_LIMIT).to_string()),
            )
            .await?;
        let raw: Vec<Value> = resp.v3()?;
        let last_modified = raw
            .iter()
            .filter_map(|d| d.get("srvModified").and_then(Timestamp::from_value))
            .max();
        let docs = decode_docs(
            raw.into_iter()
                .filter(|d| matches_base(d, &self.base))
                .collect(),
        );
        Ok(HistoryPage {
            docs,
            last_modified,
        })
    }

    /// Search used by bulk deletes: ids of up to `limit` matching documents.
    async fn matching_ids(
        &self,
        api: Api,
        filter: &DeleteFilter,
        limit: usize,
    ) -> Result<Vec<String>> {
        let spec = ListSpec {
            filter: self.base.clone().and(filter.filter.clone()),
            since: filter.from,
            until: Some(filter.until),
            limit,
            skip: 0,
            ascending: true,
            page_size: DEFAULT_PAGE_SIZE,
        };
        let docs = fetch_raw(&self.client, T::COLLECTION, &spec, api).await?;
        Ok(docs
            .iter()
            .filter_map(|d| raw_id(d).map(str::to_owned))
            .collect())
    }

    async fn delete_one(&self, api: Api, id: &str, permanent: bool) -> Result<()> {
        let name = Self::name();
        let id = checked_id(id)?;
        let inner = &self.client.inner;
        match api {
            Api::V3 => {
                let mut req = Request::delete(
                    format!("api/v3/{}/{}", name.name(), encode_segment(id)),
                    "api/v3/{collection}/{id}",
                );
                if permanent {
                    req = req.query("permanent", "true");
                }
                inner.execute(req).await.map(|_| ())
            }
            Api::V1 => {
                let routes = v1_routes(name).ok_or_else(|| no_v1(name))?;
                if routes.delete_by_path {
                    // These routes only accept the 24-hex Mongo `_id`; resolve identifiers.
                    let object_id = if is_object_id(id) {
                        id.to_owned()
                    } else {
                        self.get(id)
                            .await?
                            .meta()
                            .id
                            .clone()
                            .ok_or(Error::NotFound)?
                    };
                    inner
                        .execute(Request::delete(
                            format!("{}/{}", routes.write, encode_segment(&object_id)),
                            "api/v1/{collection}/{id}",
                        ))
                        .await
                        .map(|_| ())
                } else {
                    let resp = inner
                        .execute(
                            Request::delete(routes.write, "api/v1/{collection}")
                                .query(v1_id_key(id), id),
                        )
                        .await?;
                    match deleted_count(&resp) {
                        Some(0) => Err(Error::NotFound),
                        _ => Ok(()),
                    }
                }
            }
        }
    }
}

impl Collection<crate::model::ProfileStore> {
    /// The newest profile document (by start date).
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn current(&self) -> Result<Option<crate::model::ProfileStore>> {
        match self.api().await? {
            Api::V3 => self.latest().await,
            Api::V1 => {
                let resp = self
                    .client
                    .inner
                    .execute(Request::get(
                        "api/v1/profile/current",
                        "api/v1/profile/current",
                    ))
                    .await?;
                if resp.body.trim_ascii().is_empty() || &resp.body[..] == b"null" {
                    return Ok(None);
                }
                resp.json().map(Some)
            }
        }
    }
}

impl Collection<crate::model::Food> {
    /// Quick picks that are not hidden, in display order.
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn quickpicks(&self) -> Result<Vec<crate::model::Food>> {
        let mut picks: Vec<crate::model::Food> = self
            .list()
            .filter(Filter::new().eq("type", "quickpick"))
            .limit(V3_MAX_LIMIT)
            .send()
            .await?
            .into_iter()
            .filter(|f| f.hidden != Some(true))
            .collect();
        picks.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(picks)
    }
}

/// Fields API v3 refuses to change on an existing document.
const IMMUTABLE_V3_FIELDS: &[&str] = &[
    "identifier",
    "date",
    "utcOffset",
    "eventType",
    "device",
    "app",
    "srvCreated",
    "subject",
    "srvModified",
    "modifiedBy",
    "isValid",
    "_id",
];

/// Serializes a document for an API v3 write: no `_id`, no server-managed fields.
fn v3_body<T: Document>(doc: &T, strip_identifier: bool) -> Result<serde_json::Map<String, Value>> {
    let mut doc = doc.clone();
    doc.meta_mut().strip_server_fields();
    doc.meta_mut().id = None;
    if strip_identifier {
        doc.meta_mut().identifier = None;
    }
    match serde_json::to_value(&doc) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(Error::InvalidInput(
            "documents must serialize to JSON objects".into(),
        )),
        Err(e) => Err(Error::InvalidInput(format!(
            "could not serialize the document: {e}"
        ))),
    }
}

#[derive(Deserialize)]
struct V3Created {
    #[serde(default)]
    identifier: Option<String>,
    #[serde(rename = "lastModified", default, with = "time::millis")]
    last_modified: Option<Timestamp>,
    #[serde(rename = "isDeduplication", default)]
    is_deduplication: bool,
    #[serde(rename = "deduplicatedIdentifier", default)]
    deduplicated_identifier: Option<String>,
}

/// The outcome of an upload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Created {
    /// The stored document's identifier (API v3) or `_id` (API v1).
    pub identifier: Option<String>,
    /// Whether an existing document was updated instead of a new one being inserted.
    pub deduplicated: bool,
    /// Server modification time (API v3).
    pub last_modified: Option<Timestamp>,
}

/// One page of [`Collection::history`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HistoryPage<T> {
    /// Changed documents, oldest change first.
    pub docs: Vec<T>,
    /// The newest `srvModified` in the page; pass it to the next call. `None` when empty.
    pub last_modified: Option<Timestamp>,
}

/// The time window and conditions of a bulk delete.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct DeleteFilter {
    filter: Filter,
    from: Option<Timestamp>,
    until: Timestamp,
}

impl DeleteFilter {
    /// Documents dated at or before `until`.
    pub fn before(until: impl Into<Timestamp>) -> Self {
        Self {
            filter: Filter::new(),
            from: None,
            until: until.into(),
        }
    }

    /// Documents dated between `from` and `until` (inclusive).
    pub fn between(from: impl Into<Timestamp>, until: impl Into<Timestamp>) -> Self {
        Self {
            filter: Filter::new(),
            from: Some(from.into()),
            until: until.into(),
        }
    }

    /// Additionally requires `filter` to match.
    pub fn and(mut self, filter: Filter) -> Self {
        self.filter = self.filter.and(filter);
        self
    }
}

/// The outcome of [`Collection::delete_many`].
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct DeleteReport {
    /// Documents deleted.
    pub deleted: u64,
    /// Documents that could not be deleted, with the reason.
    pub failed: Vec<(String, Error)>,
}

/// A pending single-document delete; `.await` it.
#[derive(Debug)]
#[must_use = "a delete does nothing until awaited"]
pub struct Delete<T> {
    collection: Collection<T>,
    id: String,
    permanent: bool,
}

impl<T: Document> Delete<T> {
    /// Removes the document for good instead of soft-deleting it (API v3).
    pub const fn permanent(mut self) -> Self {
        self.permanent = true;
        self
    }
}

impl<T: Document> IntoFuture for Delete<T> {
    type Output = Result<()>;
    type IntoFuture = BoxFuture<'static, Result<()>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let api = self.collection.api().await?;
            self.collection
                .delete_one(api, &self.id, self.permanent)
                .await
        })
    }
}

/// A pending bulk delete; `.await` it.
#[derive(Debug)]
#[must_use = "a delete does nothing until awaited"]
pub struct DeleteMany<T> {
    collection: Collection<T>,
    filter: DeleteFilter,
    permanent: bool,
}

impl<T: Document> DeleteMany<T> {
    /// Removes documents for good instead of soft-deleting them (API v3).
    pub const fn permanent(mut self) -> Self {
        self.permanent = true;
        self
    }

    async fn run(self) -> Result<DeleteReport> {
        let collection = &self.collection;
        let name = T::COLLECTION;
        let api = collection.api().await?;
        let routes = v1_routes(name);

        // API v1 deletes server-side in one request.
        if let (Api::V1, Some(routes)) = (api, &routes) {
            if routes.find {
                let (field, _) = name.v1_date_field();
                let mut filter = collection.base.clone().and(self.filter.filter.clone());
                filter = filter.lte(field, self.filter.until);
                if let Some(from) = self.filter.from {
                    filter = filter.gte(field, from);
                }
                let resp = collection
                    .client
                    .inner
                    .execute(
                        Request::delete(routes.write, "api/v1/{collection}")
                            .queries(filter.v1_pairs()),
                    )
                    .await?;
                return Ok(DeleteReport {
                    deleted: deleted_count(&resp).unwrap_or(0),
                    failed: Vec::new(),
                });
            }
        }

        // API v3 (and v1 collections without `find`): search, then delete one by one.
        let mut report = DeleteReport::default();
        let mut failed: HashSet<String> = HashSet::new();
        loop {
            let ids = collection
                .matching_ids(api, &self.filter, DEFAULT_PAGE_SIZE + failed.len())
                .await?;
            let todo: Vec<String> = ids.into_iter().filter(|id| !failed.contains(id)).collect();
            if todo.is_empty() {
                break;
            }
            let results: Vec<(String, Result<()>)> = stream::iter(todo)
                .map(|id| async move {
                    let res = collection.delete_one(api, &id, self.permanent).await;
                    (id, res)
                })
                .buffer_unordered(DEFAULT_CONCURRENCY)
                .collect()
                .await;
            let mut progressed = false;
            for (id, res) in results {
                match res {
                    Ok(()) | Err(Error::NotFound | Error::Gone) => {
                        report.deleted += 1;
                        progressed = true;
                    }
                    Err(err) => {
                        failed.insert(id.clone());
                        report.failed.push((id, err));
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        Ok(report)
    }
}

impl<T: Document> IntoFuture for DeleteMany<T> {
    type Output = Result<DeleteReport>;
    type IntoFuture = BoxFuture<'static, Result<DeleteReport>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.run())
    }
}

/// Query parameters shared by list and stream.
#[derive(Debug, Clone)]
pub(crate) struct ListSpec {
    pub(crate) filter: Filter,
    pub(crate) since: Option<Timestamp>,
    pub(crate) until: Option<Timestamp>,
    pub(crate) limit: usize,
    pub(crate) skip: usize,
    pub(crate) ascending: bool,
    pub(crate) page_size: usize,
}

/// A query on a collection. `.await` it for a `Vec<T>`, or call [`List::stream`].
#[derive(Debug)]
#[must_use = "a query does nothing until awaited or streamed"]
pub struct List<T> {
    client: Client,
    spec: ListSpec,
    _doc: PhantomData<fn() -> T>,
}

impl<T: Document> List<T> {
    /// Only documents dated at or after `time`.
    pub fn since(mut self, time: impl Into<Timestamp>) -> Self {
        self.spec.since = Some(time.into());
        self
    }

    /// Only documents dated at or before `time`.
    pub fn until(mut self, time: impl Into<Timestamp>) -> Self {
        self.spec.until = Some(time.into());
        self
    }

    /// Maximum number of documents (default 10). Limits above one page (1000) are fetched
    /// page by page, as with [`List::stream`]; pass `usize::MAX` for no cap.
    pub const fn limit(mut self, limit: usize) -> Self {
        self.spec.limit = limit;
        self
    }

    /// Skips the first `n` matching documents.
    pub const fn skip(mut self, n: usize) -> Self {
        self.spec.skip = n;
        self
    }

    /// Documents per request when streaming (default 500, at most 1000).
    pub fn page_size(mut self, size: usize) -> Self {
        self.spec.page_size = size.clamp(1, V3_MAX_LIMIT);
        self
    }

    /// Oldest first instead of newest first.
    pub const fn oldest_first(mut self) -> Self {
        self.spec.ascending = true;
        self
    }

    /// Adds conditions (see [`Filter`]).
    pub fn filter(mut self, filter: Filter) -> Self {
        self.spec.filter = self.spec.filter.and(filter);
        self
    }

    /// Only documents whose `device` equals `device`.
    pub fn device(self, device: impl Into<String>) -> Self {
        self.filter(Filter::new().eq("device", device.into()))
    }

    /// Runs the query.
    ///
    /// Documents that cannot be decoded are skipped (and reported through `tracing` when
    /// that feature is on) rather than failing the whole page.
    ///
    /// # Errors
    ///
    /// Transport, authorization and response-shape errors.
    pub async fn send(mut self) -> Result<Vec<T>> {
        if self.spec.limit > V3_MAX_LIMIT {
            // Page by date like `stream`: `skip`-based pages would shift (repeating or
            // missing documents) whenever data is written during the read.
            self.spec.page_size = V3_MAX_LIMIT;
            return self.stream().try_collect().await;
        }
        let api = self
            .client
            .inner
            .pick_api(support(T::COLLECTION), "listing")
            .await?;
        let page = fetch_raw(&self.client, T::COLLECTION, &self.spec, api).await?;
        Ok(decode_docs(page))
    }

    /// Runs the query and returns raw JSON documents, optionally projected to `fields`
    /// (API v3 only; ignored by API v1). Returns at most one page (1000 documents).
    ///
    /// # Errors
    ///
    /// Transport and authorization errors.
    pub async fn send_raw(self, fields: &[&str]) -> Result<Vec<Value>> {
        let api = self
            .client
            .inner
            .pick_api(support(T::COLLECTION), "listing")
            .await?;
        let mut spec = self.spec.clone();
        spec.limit = spec.limit.min(V3_MAX_LIMIT);
        fetch_raw_with(&self.client, T::COLLECTION, &spec, api, fields).await
    }

    /// Streams every matching document page by page (up to `limit` in total; pass
    /// `usize::MAX` for no cap). Pagination follows the date field, so documents inserted
    /// while streaming do not shift pages.
    pub fn stream(self) -> BoxStream<'static, Result<T>> {
        let state = PagerState {
            client: self.client,
            spec: self.spec,
            api: None,
            boundary: HashSet::new(),
            remaining: None,
            done: false,
        };
        stream::try_unfold(state, |mut state| async move {
            if state.done {
                return Ok::<_, Error>(None);
            }
            let page = state.next_page::<T>().await?;
            Ok(Some((page, state)))
        })
        .map_ok(|page| stream::iter(page.into_iter().map(Ok)))
        .try_flatten()
        .boxed()
    }
}

impl<T: Document> IntoFuture for List<T> {
    type Output = Result<Vec<T>>;
    type IntoFuture = BoxFuture<'static, Result<Vec<T>>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

struct PagerState {
    client: Client,
    spec: ListSpec,
    api: Option<Api>,
    /// Ids already yielded at the current boundary timestamp.
    boundary: HashSet<String>,
    remaining: Option<usize>,
    done: bool,
}

impl PagerState {
    async fn next_page<T: Document>(&mut self) -> Result<Vec<T>> {
        let api = match self.api {
            Some(api) => api,
            None => {
                let api = self
                    .client
                    .inner
                    .pick_api(support(T::COLLECTION), "streaming")
                    .await?;
                self.api = Some(api);
                api
            }
        };
        let remaining = *self.remaining.get_or_insert(self.spec.limit);
        if remaining == 0 {
            self.done = true;
            return Ok(Vec::new());
        }
        let page_size = remaining.min(self.spec.page_size);
        let mut spec = self.spec.clone();
        spec.limit = (page_size + self.boundary.len()).min(V3_MAX_LIMIT);
        let raw = fetch_raw(&self.client, T::COLLECTION, &spec, api).await?;
        // `skip` applies to the first page only; later pages start from the time bound.
        self.spec.skip = 0;
        let full_page = raw.len() >= spec.limit;

        let mut page: Vec<Value> = raw
            .into_iter()
            .filter(|d| raw_id(d).is_none_or(|id| !self.boundary.contains(id)))
            .collect();
        page.truncate(remaining);

        // Advance the time bound to the last timestamp seen, remembering which ids sit on it.
        // Both come from the raw documents and the field the query sorts on, so documents
        // that fail to decode still move the bound instead of ending the stream.
        let time = |d: &Value| query_time(d, T::COLLECTION, api);
        let edge = if self.spec.ascending {
            page.iter().filter_map(time).max()
        } else {
            page.iter().filter_map(time).min()
        };
        match edge {
            Some(edge) => {
                let moved = if self.spec.ascending {
                    self.spec.since != Some(edge)
                } else {
                    self.spec.until != Some(edge)
                };
                if moved {
                    self.boundary.clear();
                }
                self.boundary.extend(
                    page.iter()
                        .filter(|d| time(d) == Some(edge))
                        .filter_map(|d| raw_id(d).map(str::to_owned)),
                );
                if self.spec.ascending {
                    self.spec.since = Some(edge);
                } else {
                    self.spec.until = Some(edge);
                }
            }
            None => self.done = true,
        }

        let consumed = page.len();
        let docs: Vec<T> = decode_docs(page);
        self.remaining = Some(remaining - docs.len());
        if consumed == 0 || !full_page || self.remaining == Some(0) {
            self.done = true;
        }
        Ok(docs)
    }
}

/// Fetches one page of raw documents.
pub(crate) async fn fetch_raw(
    client: &Client,
    name: CollectionName,
    spec: &ListSpec,
    api: Api,
) -> Result<Vec<Value>> {
    fetch_raw_with(client, name, spec, api, &[]).await
}

async fn fetch_raw_with(
    client: &Client,
    name: CollectionName,
    spec: &ListSpec,
    api: Api,
    fields: &[&str],
) -> Result<Vec<Value>> {
    let inner = &client.inner;
    match api {
        Api::V3 => {
            let (date_field, _) = name.v3_date_field();
            let mut filter = spec.filter.clone();
            if let Some(since) = spec.since {
                filter = filter.gte(date_field, since);
            }
            if let Some(until) = spec.until {
                filter = filter.lte(date_field, until);
            }
            let mut req = Request::get(format!("api/v3/{}", name.name()), "api/v3/{collection}")
                .query("limit", spec.limit.clamp(1, V3_MAX_LIMIT).to_string())
                .query(
                    if spec.ascending { "sort" } else { "sort$desc" },
                    date_field,
                )
                .queries(filter.v3_pairs());
            if spec.skip > 0 {
                req = req.query("skip", spec.skip.to_string());
            }
            if !fields.is_empty() {
                req = req.query("fields", fields.join(","));
            }
            inner.execute(req).await?.v3()
        }
        Api::V1 => {
            let routes = v1_routes(name).ok_or_else(|| no_v1(name))?;
            let (date_field, _) = name.v1_date_field();
            let mut docs: Vec<Value> = if routes.find {
                let mut filter = spec.filter.clone();
                // Without an explicit bound on the date field, API v1 silently limits every
                // query to the last 4 days.
                filter = filter.gte(date_field, spec.since.unwrap_or(Timestamp::EPOCH));
                if let Some(until) = spec.until {
                    filter = filter.lte(date_field, until);
                }
                let mut req = Request::get(routes.list, "api/v1/{collection}")
                    .query("count", (spec.limit + spec.skip).max(1).to_string())
                    .queries(filter.v1_pairs());
                if spec.ascending {
                    req = req.query(format!("sort[{date_field}]"), "1");
                }
                inner.execute(req).await?.json()?
            } else {
                if spec
                    .filter
                    .conditions
                    .iter()
                    .any(|c| !matches!(c.op, Op::Eq))
                {
                    return Err(Error::Unsupported {
                        feature: "filters",
                        reason: format!("API v1 only supports equality filters on {name}"),
                    });
                }
                let all: Vec<Value> = inner
                    .execute(
                        Request::get(routes.list, "api/v1/{collection}").query("count", "10000"),
                    )
                    .await?
                    .json()?;
                all.into_iter()
                    .filter(|d| matches_base(d, &spec.filter))
                    .filter(|d| {
                        let t = doc_time(d, name);
                        spec.since.is_none_or(|s| t.is_some_and(|t| t >= s))
                            && spec.until.is_none_or(|u| t.is_some_and(|t| t <= u))
                    })
                    .collect()
            };
            // API v1 sorts entries newest-first regardless of `sort`; normalize the order.
            docs.sort_by_key(|d| doc_time(d, name));
            if !spec.ascending {
                docs.reverse();
            }
            Ok(docs.into_iter().skip(spec.skip).take(spec.limit).collect())
        }
    }
}

/// The id of a raw document (`identifier`, else `_id`), as [`Document::id`] reads it.
pub(crate) fn raw_id(doc: &Value) -> Option<&str> {
    doc.get("identifier")
        .or_else(|| doc.get("_id"))
        .and_then(Value::as_str)
}

/// The time of a raw document on the field `api` range-filters and sorts on, so pagination
/// bounds agree with what the server (or the v1 fallback filter) compares against.
pub(crate) fn query_time(doc: &Value, name: CollectionName, api: Api) -> Option<Timestamp> {
    match api {
        Api::V3 => doc
            .get(name.v3_date_field().0)
            .and_then(Timestamp::from_value),
        Api::V1 => doc_time(doc, name),
    }
}

/// The time API v1 filters and sorts on: the collection's v1 date field first.
fn doc_time(doc: &Value, name: CollectionName) -> Option<Timestamp> {
    let fields: &[&str] = match name {
        CollectionName::Entries | CollectionName::Settings => &["date"],
        CollectionName::Profile => &["startDate", "created_at", "date"],
        _ => &["created_at", "date"],
    };
    fields
        .iter()
        .find_map(|f| doc.get(*f).and_then(Timestamp::from_value))
}

/// Checks the equality conditions of `filter` against a raw document.
fn matches_base(doc: &Value, filter: &Filter) -> bool {
    filter.conditions.iter().all(|c| match (&c.op, &c.value) {
        (Op::Eq, crate::query::Value::Text(want)) => {
            doc.get(&c.field).and_then(Value::as_str) == Some(want.as_str())
        }
        _ => true,
    })
}

/// Decodes documents one by one, skipping (and tracing) the ones that do not fit `T`.
pub(crate) fn decode_docs<T: serde::de::DeserializeOwned>(values: Vec<Value>) -> Vec<T> {
    values
        .into_iter()
        .filter_map(
            |value| match serde_path_to_error::deserialize::<_, T>(&value) {
                Ok(doc) => Some(doc),
                Err(_err) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        target: "cinnamon",
                        path = %_err.path(),
                        error = %_err.inner(),
                        id = raw_id(&value),
                        "skipping a document that does not decode"
                    );
                    None
                }
            },
        )
        .collect()
}

fn deleted_count(resp: &Response) -> Option<u64> {
    let v: Value = resp.json().ok()?;
    v.get("deletedCount")
        .or_else(|| v.get("n"))
        .and_then(Value::as_u64)
}

/// API v1 looks documents up by Mongo `_id` (24 hex) or by the API v3 `identifier`.
fn v1_id_key(id: &str) -> &'static str {
    if is_object_id(id) {
        "find[_id]"
    } else {
        "find[identifier]"
    }
}

fn is_object_id(id: &str) -> bool {
    id.len() == 24 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Rejects ids that would change the meaning of a URL (an empty id once deleted a whole
/// entry type through Nightscout's `/entries/:spec` route).
fn checked_id(id: &str) -> Result<&str> {
    let id = id.trim();
    if id.is_empty() || id.contains(['/', '?', '#', '*']) || id.chars().any(char::is_whitespace) {
        return Err(Error::InvalidInput(format!("invalid document id {id:?}")));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_that_change_url_meaning_are_rejected() {
        assert!(checked_id("").is_err());
        assert!(checked_id("  ").is_err());
        assert!(checked_id("abc/def").is_err());
        assert!(checked_id("*").is_err());
        assert!(checked_id("65f1c2a8e4b0a1b2c3d4e5f6").is_ok());
        assert!(checked_id("fa0f5891-69a8-50c8-b01d-0da5e7000589").is_ok());
    }
}
