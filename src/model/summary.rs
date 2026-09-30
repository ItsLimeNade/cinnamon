//! `GET /api/v2/summary`: a compact recent-history summary designed for watches and widgets.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Extra, Timestamp, de, time};

/// The summary document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Summary {
    /// Glucose samples.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub sgvs: Vec<SummarySgv>,
    /// Treatments, temp basals and temp targets.
    #[serde(default)]
    pub treatments: SummaryTreatments,
    /// The active profile store entry.
    #[serde(default)]
    pub profile: Option<Value>,
    /// Current plugin state (iob, cob, bwp, device ages, battery).
    #[serde(default)]
    pub state: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A glucose sample in the summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummarySgv {
    /// Glucose in mg/dL.
    #[serde(default, with = "de::num")]
    pub sgv: Option<f64>,
    /// Sample time.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Noise level.
    #[serde(default, with = "de::int")]
    pub noise: Option<i64>,
}

/// Treatments in the summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryTreatments {
    /// Temp basals.
    #[serde(rename = "tempBasals", default, deserialize_with = "de::null_as_empty")]
    pub temp_basals: Vec<SummaryTempBasal>,
    /// Boluses and carbs.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub treatments: Vec<SummaryTreatment>,
    /// Temporary targets.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub targets: Vec<SummaryTarget>,
}

/// A temp basal in the summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryTempBasal {
    /// Start time.
    #[serde(default, with = "time::millis")]
    pub start: Option<Timestamp>,
    /// Duration in seconds.
    #[serde(default, with = "de::num")]
    pub duration: Option<f64>,
    /// Rate in U/h (Nightscout reports percent-only temps as 0).
    #[serde(default, with = "de::num")]
    pub absolute: Option<f64>,
    /// Profile name.
    #[serde(default, with = "de::text")]
    pub profile: Option<String>,
}

/// A bolus/carb entry in the summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryTreatment {
    /// Time.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Carbs in grams.
    #[serde(default, with = "de::num")]
    pub carbs: Option<f64>,
    /// Insulin in units.
    #[serde(default, with = "de::num")]
    pub insulin: Option<f64>,
}

/// A temporary target in the summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SummaryTarget {
    /// Start time.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Upper bound.
    #[serde(rename = "targetTop", default, with = "de::num")]
    pub target_top: Option<f64>,
    /// Lower bound.
    #[serde(rename = "targetBottom", default, with = "de::num")]
    pub target_bottom: Option<f64>,
    /// Duration in seconds.
    #[serde(default, with = "de::num")]
    pub duration: Option<f64>,
}
