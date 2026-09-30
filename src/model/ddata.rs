//! `GET /api/v2/ddata/at/{time}`: the data window Nightscout's plugins compute from.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    DeviceStatus, Extra, ProfileStore, Timestamp, Treatment, de, properties::PropertySgv, time,
};

/// A snapshot of the server's in-memory data (roughly the last two days before `at`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DData {
    /// Sensor glucose samples.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub sgvs: Vec<PropertySgv>,
    /// Meter glucose samples.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub mbgs: Vec<PropertySgv>,
    /// Calibrations.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub cals: Vec<Value>,
    /// Treatments.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub treatments: Vec<Treatment>,
    /// Profile documents.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub profiles: Vec<ProfileStore>,
    /// Device status reports.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub devicestatus: Vec<DeviceStatus>,
    /// Food items.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub food: Vec<Value>,
    /// Database statistics.
    #[serde(default)]
    pub dbstats: Option<Value>,
    /// When the server last refreshed this data.
    #[serde(rename = "lastUpdated", default, with = "time::millis")]
    pub last_updated: Option<Timestamp>,
    /// Pre-filtered treatment lists (`sitechangeTreatments`, `tempbasalTreatments`, …) and
    /// other fields.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Decodes each element independently, skipping the ones that fail.
pub(crate) fn tolerant_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    Ok(Option::<Vec<Value>>::deserialize(d)?
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}
