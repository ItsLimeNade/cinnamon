//! Typed Nightscout documents.
//!
//! Every model is **lenient on input** (numbers as strings, `null`s, missing fields and
//! unknown values are tolerated) and **lossless on output**: fields cinnamon does not know
//! are kept in an `extra` map and written back unchanged, so read-modify-write never drops
//! data another uploader stored.

pub(crate) mod de;

pub mod activity;
pub mod admin;
pub mod ddata;
pub mod devicestatus;
pub mod direction;
pub mod entry;
pub mod food;
pub mod glucose;
pub mod notification;
pub mod profile;
pub mod properties;
pub mod settings;
pub mod summary;
pub mod system;
pub mod time;
pub mod treatment;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use activity::Activity;
pub use devicestatus::DeviceStatus;
pub use direction::Direction;
pub use entry::{Cal, Entry, Mbg, Sgv};
pub use food::Food;
pub use glucose::{Glucose, Units};
pub use profile::{Profile, ProfileStore, ScheduleEntry};
pub use settings::Setting;
pub use time::Timestamp;
pub use treatment::{EventType, Treatment, TreatmentKind};

/// A JSON object of fields cinnamon does not model explicitly.
pub type Extra = serde_json::Map<String, serde_json::Value>;

/// Server-managed bookkeeping fields shared by every document.
///
/// API v3 sets `identifier`, `srvCreated`, `srvModified`, `subject`, `modifiedBy` and
/// `isValid`; API v1 documents carry a Mongo `_id`. Cinnamon strips the server-managed ones
/// before updates, because API v3 rejects writes that change them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Meta {
    /// MongoDB id (24 hex characters) on documents read through API v1.
    #[serde(
        rename = "_id",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub id: Option<String>,
    /// API v3 identifier (a UUID; for legacy documents the hex `_id`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub identifier: Option<String>,
    /// When the server first stored the document.
    #[serde(
        rename = "srvCreated",
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub srv_created: Option<Timestamp>,
    /// When the server last changed the document; the cursor used by history sync.
    #[serde(
        rename = "srvModified",
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub srv_modified: Option<Timestamp>,
    /// Name of the access-token subject that created or last replaced the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Subject that last patched or deleted the document.
    #[serde(
        rename = "modifiedBy",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub modified_by: Option<String>,
    /// `false` once the document is soft-deleted (only visible through history).
    #[serde(
        rename = "isValid",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::flag"
    )]
    pub is_valid: Option<bool>,
    /// Read-only documents cannot be updated or deleted.
    #[serde(
        rename = "isReadOnly",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::flag"
    )]
    pub is_read_only: Option<bool>,
    /// The application that uploaded the document (required by API v3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Local time offset of the uploader, in minutes.
    #[serde(
        rename = "utcOffset",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::int32"
    )]
    pub utc_offset: Option<i32>,
}

impl Meta {
    /// The id to address this document with: the v3 `identifier`, else the v1 `_id`.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.identifier.as_deref().or(self.id.as_deref())
    }

    /// Whether the document is soft-deleted.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.is_valid == Some(false)
    }

    /// Removes the fields API v3 refuses to accept from clients on updates.
    pub(crate) fn strip_server_fields(&mut self) {
        self.srv_created = None;
        self.srv_modified = None;
        self.subject = None;
        self.modified_by = None;
        self.is_valid = None;
    }
}

/// A Nightscout collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum CollectionName {
    /// CGM, meter and calibration records.
    Entries,
    /// Care events: boluses, carbs, temp basals, site changes, …
    Treatments,
    /// Uploader, pump and closed-loop status.
    #[serde(rename = "devicestatus")]
    DeviceStatus,
    /// Therapy profiles.
    Profile,
    /// Food database and quick picks.
    Food,
    /// Per-application settings (API v3 only).
    Settings,
    /// Activity records (API v1 only).
    Activity,
}

impl CollectionName {
    /// Collections served by API v3.
    pub const V3: [Self; 6] = [
        Self::Entries,
        Self::Treatments,
        Self::DeviceStatus,
        Self::Profile,
        Self::Food,
        Self::Settings,
    ];

    /// The collection name used in URLs and API v3 payloads.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Entries => "entries",
            Self::Treatments => "treatments",
            Self::DeviceStatus => "devicestatus",
            Self::Profile => "profile",
            Self::Food => "food",
            Self::Settings => "settings",
            Self::Activity => "activity",
        }
    }

    /// The field holding the document time, and whether it is stored as epoch millis
    /// (`true`) or an ISO string (`false`), as seen by API v1 queries.
    pub(crate) const fn v1_date_field(self) -> (&'static str, bool) {
        match self {
            Self::Entries | Self::Settings => ("date", true),
            Self::Profile => ("startDate", false),
            Self::Treatments | Self::DeviceStatus | Self::Food | Self::Activity => {
                ("created_at", false)
            }
        }
    }

    /// The field to range-filter and sort on through API v3. Documents uploaded through v1
    /// usually lack `date` outside of entries, but every v3 write also sets `created_at`.
    pub(crate) const fn v3_date_field(self) -> (&'static str, bool) {
        match self {
            Self::Entries | Self::Settings => ("date", true),
            _ => ("created_at", false),
        }
    }
}

impl std::fmt::Display for CollectionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A document stored in a Nightscout collection.
///
/// Implemented by every collection model; sealed so cinnamon can evolve it.
pub trait Document:
    Serialize + DeserializeOwned + Clone + Send + Sync + 'static + sealed::Sealed
{
    /// The collection the document lives in.
    const COLLECTION: CollectionName;

    /// Bookkeeping fields.
    fn meta(&self) -> &Meta;

    /// Mutable bookkeeping fields.
    fn meta_mut(&mut self) -> &mut Meta;

    /// The document's primary time (`date` for entries, `created_at` otherwise).
    fn timestamp(&self) -> Option<Timestamp>;

    /// The uploading device, when the document records one.
    fn device(&self) -> Option<&str> {
        None
    }

    /// The treatment event type, for treatments.
    fn event_type(&self) -> Option<&str> {
        None
    }

    /// The id to address the document with (`identifier`, else `_id`).
    fn id(&self) -> Option<&str> {
        self.meta().id()
    }

    /// Fills the fields a write needs: `date`/`created_at` consistency, `app`, `utcOffset`
    /// and the deterministic API v3 `identifier`.
    #[doc(hidden)]
    fn prepare_for_upload(&mut self, app: &str);
}

/// Namespace of Nightscout's API v3 identifiers (`"NightscoutRocks!"` as 16 ASCII bytes).
const IDENTIFIER_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes(*b"NightscoutRocks!");

/// Computes the identifier API v3 assigns to a document, so clients can address (and
/// safely retry) writes before the server answers.
///
/// Mirrors `calculateIdentifier` in Nightscout: UUIDv5 over `"{device}_{date}"`, with
/// `"_{eventType}"` appended when there is one. A missing device is the literal
/// `"undefined"`, as in JavaScript.
#[must_use]
pub fn compute_identifier(
    device: Option<&str>,
    date: Timestamp,
    event_type: Option<&str>,
) -> String {
    let mut key = format!("{}_{}", device.unwrap_or("undefined"), date.as_millis());
    if let Some(event_type) = event_type.filter(|e| !e.is_empty()) {
        key.push('_');
        key.push_str(event_type);
    }
    uuid::Uuid::new_v5(&IDENTIFIER_NAMESPACE, key.as_bytes()).to_string()
}

/// Shared upload preparation: stamps `app`, `utcOffset` and the identifier.
pub(crate) fn prepare_meta<D: Document>(doc: &mut D, app: &str) {
    let date = doc.timestamp();
    let identifier = date.map(|d| compute_identifier(doc.device(), d, doc.event_type()));
    let meta = doc.meta_mut();
    if meta.app.is_none() {
        meta.app = Some(app.to_owned());
    }
    if meta.utc_offset.is_none() {
        meta.utc_offset = Some(local_utc_offset_minutes());
    }
    if meta.identifier.is_none() {
        meta.identifier = identifier;
    }
}

fn local_utc_offset_minutes() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}

macro_rules! impl_document {
    ($ty:ty, $collection:expr, timestamp = |$s:ident| $ts:expr, device = |$d:ident| $dev:expr $(, event_type = |$e:ident| $et:expr)? $(,)?) => {
        impl crate::model::sealed::Sealed for $ty {}
        impl crate::model::Document for $ty {
            const COLLECTION: crate::model::CollectionName = $collection;
            fn meta(&self) -> &crate::model::Meta {
                &self.meta
            }
            fn meta_mut(&mut self) -> &mut crate::model::Meta {
                &mut self.meta
            }
            fn timestamp(&self) -> Option<crate::model::Timestamp> {
                let $s = self;
                $ts
            }
            fn device(&self) -> Option<&str> {
                let $d = self;
                $dev
            }
            $(
            fn event_type(&self) -> Option<&str> {
                let $e = self;
                $et
            }
            )?
            fn prepare_for_upload(&mut self, app: &str) {
                self.fill_defaults();
                crate::model::prepare_meta(self, app);
            }
        }
    };
}
pub(crate) use impl_document;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_matches_the_server() {
        // Created on Nightscout 15.0.8: POST /api/v3/entries
        // {"device":"probe","date":1758790800000,...} → identifier fa0f5891-…
        assert_eq!(
            compute_identifier(
                Some("probe"),
                Timestamp::from_millis(1_758_790_800_000),
                None
            ),
            "fa0f5891-69a8-50c8-b01d-0da5e7000589"
        );
    }
}
