//! The `settings` collection (API v3 only): arbitrary per-application settings documents.

use serde::{Deserialize, Serialize};

use super::{CollectionName, Extra, Meta, Timestamp, impl_document, time};

/// A settings document. Applications pick their own `identifier` (e.g. `"aaps"`) and store
/// whatever they need in `extra`. Reading settings requires the `api:settings:admin`
/// permission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Setting {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// When the settings were written.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub date: Option<Timestamp>,
    /// The settings payload.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Setting {
    /// A settings document addressed by `identifier` holding `values`.
    #[must_use]
    pub fn new(identifier: impl Into<String>, values: Extra) -> Self {
        Self {
            meta: Meta {
                identifier: Some(identifier.into()),
                ..Meta::default()
            },
            date: Some(Timestamp::now()),
            extra: values,
        }
    }

    fn fill_defaults(&mut self, _app: &str) {
        self.date.get_or_insert_with(Timestamp::now);
    }
}

impl_document!(
    Setting,
    CollectionName::Settings,
    timestamp = |s| s.date,
    device = |_s| None
);
