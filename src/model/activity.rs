//! The `activity` collection (API v1 only): steps, heart rate and other activity records.

use serde::{Deserialize, Serialize};

use super::{CollectionName, Extra, Meta, Timestamp, de, impl_document, time};

/// An activity record. The schema is uploader-defined; known fields are typed and everything
/// else is kept in `extra`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Activity {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// When the activity was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "time::iso")]
    pub created_at: Option<Timestamp>,
    /// Step count.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub steps: Option<f64>,
    /// Heart rate in beats per minute.
    #[serde(
        rename = "heartRate",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub heart_rate: Option<f64>,
    /// Recording device.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub device: Option<String>,
    /// Who entered it.
    #[serde(
        rename = "enteredBy",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub entered_by: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Activity {
    /// An empty activity record dated now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: Meta::default(),
            created_at: Some(Timestamp::now()),
            steps: None,
            heart_rate: None,
            device: None,
            entered_by: None,
            extra: Extra::new(),
        }
    }

    fn fill_defaults(&mut self, app: &str) {
        self.created_at.get_or_insert_with(Timestamp::now);
        self.device.get_or_insert_with(|| app.to_owned());
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self::new()
    }
}

impl_document!(
    Activity,
    CollectionName::Activity,
    timestamp = |s| s.created_at,
    device = |s| s.device.as_deref()
);
