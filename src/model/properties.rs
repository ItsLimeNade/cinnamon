//! `GET /api/v2/properties`: the live state Nightscout's plugins compute (IOB, COB,
//! delta, device ages, pump and loop status, …).
//!
//! A property is only present when its plugin is enabled on the server (`ENABLE=iob cob …`).
//! Every property is decoded independently and tolerantly: a plugin changing its output
//! shape leaves that one property `None` (the raw JSON stays in [`Properties::extra`]).

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Extra, Timestamp, de, time};

/// Names of the properties Nightscout can compute.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Property {
    /// Latest glucose and the 5-minute buckets it is computed from.
    BgNow,
    /// Glucose delta.
    Delta,
    /// 5-minute glucose buckets.
    Buckets,
    /// Trend arrow.
    Direction,
    /// Raw BG from unfiltered sensor data.
    RawBg,
    /// Uploader battery.
    Upbat,
    /// AR2 forecast.
    Ar2,
    /// Insulin on board.
    Iob,
    /// Carbs on board.
    Cob,
    /// Pump status.
    Pump,
    /// OpenAPS / AAPS status.
    OpenAps,
    /// Loop status.
    Loop,
    /// Bolus wizard preview.
    Bwp,
    /// Cannula age.
    Cage,
    /// Sensor age.
    Sage,
    /// Insulin (reservoir) age.
    Iage,
    /// Pump battery age.
    Bage,
    /// Current basal.
    Basal,
    /// Database size.
    DbSize,
    /// Server runtime state.
    RuntimeState,
    /// Any other plugin property.
    Custom(String),
}

impl Property {
    /// The property key used in URLs and responses.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::BgNow => "bgnow",
            Self::Delta => "delta",
            Self::Buckets => "buckets",
            Self::Direction => "direction",
            Self::RawBg => "rawbg",
            Self::Upbat => "upbat",
            Self::Ar2 => "ar2",
            Self::Iob => "iob",
            Self::Cob => "cob",
            Self::Pump => "pump",
            Self::OpenAps => "openaps",
            Self::Loop => "loop",
            Self::Bwp => "bwp",
            Self::Cage => "cage",
            Self::Sage => "sage",
            Self::Iage => "iage",
            Self::Bage => "bage",
            Self::Basal => "basal",
            Self::DbSize => "dbsize",
            Self::RuntimeState => "runtimestate",
            Self::Custom(s) => s,
        }
    }
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Everything `/api/v2/properties` returned.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Properties {
    /// Latest glucose.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub bgnow: Option<BgNow>,
    /// Glucose delta.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub delta: Option<Delta>,
    /// 5-minute buckets.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub buckets: Option<Vec<Bucket>>,
    /// Trend arrow.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub direction: Option<DirectionProperty>,
    /// Raw BG.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub rawbg: Option<RawBg>,
    /// Uploader battery.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub upbat: Option<Upbat>,
    /// AR2 forecast.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub ar2: Option<Ar2>,
    /// Insulin on board.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub iob: Option<Iob>,
    /// Carbs on board.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub cob: Option<Cob>,
    /// Pump status (the latest devicestatus with pump data, plus a display summary).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub pump: Option<PumpProperty>,
    /// OpenAPS status.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub openaps: Option<LoopProperty>,
    /// Loop status.
    #[serde(
        rename = "loop",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub loop_status: Option<LoopProperty>,
    /// Bolus wizard preview.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub bwp: Option<Bwp>,
    /// Cannula age.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub cage: Option<DeviceAge>,
    /// Sensor age (`Sensor Start` and `Sensor Change` tracked separately).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub sage: Option<SensorAge>,
    /// Insulin age.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub iage: Option<DeviceAge>,
    /// Pump battery age.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub bage: Option<DeviceAge>,
    /// Current basal.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub basal: Option<Basal>,
    /// Database size.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub dbsize: Option<DbSize>,
    /// Runtime state.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub runtimestate: Option<RuntimeState>,
    /// Any other property, and the raw JSON of properties that did not decode.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A glucose sample inside properties (the server's in-memory `ddata` shape).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PropertySgv {
    /// Document id.
    #[serde(rename = "_id", default, with = "de::text")]
    pub id: Option<String>,
    /// Glucose in mg/dL.
    #[serde(default, with = "de::num")]
    pub mgdl: Option<f64>,
    /// Sample time.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Uploading device.
    #[serde(default, with = "de::text")]
    pub device: Option<String>,
    /// Trend arrow.
    #[serde(default, with = "de::text")]
    pub direction: Option<String>,
    /// Glucose in display units.
    #[serde(default)]
    pub scaled: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `bgnow`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BgNow {
    /// Mean of the latest bucket (mg/dL).
    #[serde(default, with = "de::num")]
    pub mean: Option<f64>,
    /// Latest value (mg/dL).
    #[serde(default, with = "de::num")]
    pub last: Option<f64>,
    /// Time of the latest value.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Samples in the latest bucket.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub sgvs: Vec<PropertySgv>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `delta`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Delta {
    /// Absolute change (mg/dL).
    #[serde(default, with = "de::num")]
    pub absolute: Option<f64>,
    /// Minutes between the compared buckets.
    #[serde(rename = "elapsedMins", default, with = "de::num")]
    pub elapsed_mins: Option<f64>,
    /// Whether the previous value was interpolated over a gap.
    #[serde(default, with = "de::flag")]
    pub interpolated: Option<bool>,
    /// Mean of the bucket 5 minutes earlier.
    #[serde(rename = "mean5MinsAgo", default, with = "de::num")]
    pub mean_5_mins_ago: Option<f64>,
    /// Delta normalized to 5 minutes, in mg/dL.
    #[serde(default, with = "de::num")]
    pub mgdl: Option<f64>,
    /// Delta in display units.
    #[serde(default, with = "de::num")]
    pub scaled: Option<f64>,
    /// Display string, e.g. `+3`.
    #[serde(default, with = "de::text")]
    pub display: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One 5-minute bucket.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Bucket {
    /// Bucket index (0 = newest).
    #[serde(default, with = "de::int")]
    pub index: Option<i64>,
    /// Bucket start.
    #[serde(rename = "fromMills", default, with = "time::millis")]
    pub from_mills: Option<Timestamp>,
    /// Bucket end.
    #[serde(rename = "toMills", default, with = "time::millis")]
    pub to_mills: Option<Timestamp>,
    /// Mean glucose in the bucket.
    #[serde(default, with = "de::num")]
    pub mean: Option<f64>,
    /// Newest glucose in the bucket.
    #[serde(default, with = "de::num")]
    pub last: Option<f64>,
    /// Newest sample time.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Samples.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub sgvs: Vec<PropertySgv>,
    /// Whether the bucket has no samples.
    #[serde(rename = "isEmpty", default, with = "de::flag")]
    pub is_empty: Option<bool>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `direction`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DirectionProperty {
    /// Direction name, e.g. `Flat`.
    #[serde(default, with = "de::text")]
    pub value: Option<String>,
    /// Arrow character.
    #[serde(default, with = "de::text")]
    pub label: Option<String>,
    /// HTML entity of the arrow.
    #[serde(default, with = "de::text")]
    pub entity: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `rawbg`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RawBg {
    /// Raw glucose in mg/dL.
    #[serde(default, with = "de::num")]
    pub mgdl: Option<f64>,
    /// Noise label (`Clean`, `Light`, …).
    #[serde(rename = "noiseLabel", default, with = "de::text")]
    pub noise_label: Option<String>,
    /// Display line.
    #[serde(rename = "displayLine", default, with = "de::text")]
    pub display_line: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `upbat`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Upbat {
    /// Lowest battery level across devices.
    #[serde(default, with = "de::num")]
    pub level: Option<f64>,
    /// Display string, e.g. `58%`.
    #[serde(default, with = "de::text")]
    pub display: Option<String>,
    /// Alert status (`normal`, `warn`, `urgent`).
    #[serde(default, with = "de::text")]
    pub status: Option<String>,
    /// Per-device battery details.
    #[serde(default)]
    pub devices: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `ar2`: Nightscout's autoregressive forecast.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ar2 {
    /// Forecast points and error.
    #[serde(default, deserialize_with = "de::tolerant")]
    pub forecast: Option<Ar2Forecast>,
    /// Alarm level raised by the forecast.
    #[serde(default, with = "de::int")]
    pub level: Option<i64>,
    /// Event name (`high`, `low`, …).
    #[serde(rename = "eventName", default, with = "de::text")]
    pub event_name: Option<String>,
    /// Display line.
    #[serde(rename = "displayLine", default, with = "de::text")]
    pub display_line: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// AR2 forecast points.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ar2Forecast {
    /// Predicted values.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub predicted: Vec<ForecastPoint>,
    /// Average loss of the forecast (drives alarm levels).
    #[serde(rename = "avgLoss", default, with = "de::num")]
    pub avg_loss: Option<f64>,
}

/// A predicted glucose value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ForecastPoint {
    /// Predicted time.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Predicted glucose in mg/dL.
    #[serde(default, with = "de::num")]
    pub mgdl: Option<f64>,
}

/// `iob`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Iob {
    /// Insulin on board in units.
    #[serde(default, with = "de::num")]
    pub iob: Option<f64>,
    /// IOB attributable to basal (loop sources).
    #[serde(default, with = "de::num")]
    pub basaliob: Option<f64>,
    /// Insulin activity.
    #[serde(default, with = "de::num")]
    pub activity: Option<f64>,
    /// Where the value comes from: `OpenAPS`, `Loop`, `MM Connect` or `Care Portal`.
    #[serde(default, with = "de::text")]
    pub source: Option<String>,
    /// Device that reported it.
    #[serde(default, with = "de::text")]
    pub device: Option<String>,
    /// When it was computed.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// IOB computed from treatments, for comparison.
    #[serde(rename = "treatmentIob", default)]
    pub treatment_iob: Option<Value>,
    /// Last bolus (treatment source only).
    #[serde(rename = "lastBolus", default, deserialize_with = "de::tolerant")]
    pub last_bolus: Option<super::Treatment>,
    /// Display value.
    #[serde(default, with = "de::text")]
    pub display: Option<String>,
    /// Display line, e.g. `IOB: 1.25U`.
    #[serde(rename = "displayLine", default, with = "de::text")]
    pub display_line: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `cob`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Cob {
    /// Carbs on board in grams.
    #[serde(default, with = "de::num")]
    pub cob: Option<f64>,
    /// Where the value comes from.
    #[serde(default, with = "de::text")]
    pub source: Option<String>,
    /// Device that reported it.
    #[serde(default, with = "de::text")]
    pub device: Option<String>,
    /// When it was computed.
    #[serde(default, with = "time::millis")]
    pub mills: Option<Timestamp>,
    /// Whether carbs are currently decaying (treatment source).
    #[serde(rename = "isDecaying", default, with = "de::flag")]
    pub is_decaying: Option<bool>,
    /// When the carbs will be absorbed (treatment source).
    #[serde(rename = "decayedBy", default, with = "time::iso")]
    pub decayed_by: Option<Timestamp>,
    /// Absorption rate in g/h (treatment source).
    #[serde(default, with = "de::num")]
    pub carbs_hr: Option<f64>,
    /// Display value.
    #[serde(default)]
    pub display: Option<Value>,
    /// Display line.
    #[serde(rename = "displayLine", default, with = "de::text")]
    pub display_line: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `pump`: the newest devicestatus carrying pump data, with a display summary in `data`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PumpProperty {
    /// The pump object of the devicestatus.
    #[serde(default, deserialize_with = "de::tolerant")]
    pub pump: Option<super::devicestatus::Pump>,
    /// Pump clock.
    #[serde(rename = "clockMills", default, with = "time::millis")]
    pub clock_mills: Option<Timestamp>,
    /// Display summary (levels, labels, reservoir, battery, status).
    #[serde(default)]
    pub data: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `openaps` and `loop`: closed-loop status.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopProperty {
    /// Status display (`symbol`, `code`, `label`).
    #[serde(default, alias = "display")]
    pub status: Option<LoopStatusDisplay>,
    /// Last enacted action.
    #[serde(rename = "lastEnacted", default)]
    pub last_enacted: Option<Value>,
    /// Last suggestion (OpenAPS).
    #[serde(rename = "lastSuggested", default)]
    pub last_suggested: Option<Value>,
    /// Last prediction (Loop: `lastPredicted`; OpenAPS: `lastPredBGs`).
    #[serde(rename = "lastPredicted", alias = "lastPredBGs", default)]
    pub last_predicted: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Closed-loop status display.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopStatusDisplay {
    /// Symbol shown in the UI.
    #[serde(default, with = "de::text")]
    pub symbol: Option<String>,
    /// Status code: `enacted`, `looping`, `recommendation`, `warning`, `error`, …
    #[serde(default, with = "de::text")]
    pub code: Option<String>,
    /// Status label.
    #[serde(default, with = "de::text")]
    pub label: Option<String>,
}

/// `bwp`: bolus wizard preview.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Bwp {
    /// Suggested bolus in units.
    #[serde(rename = "bolusEstimate", default, with = "de::num")]
    pub bolus_estimate: Option<f64>,
    /// Current IOB considered.
    #[serde(default, with = "de::num")]
    pub iob: Option<f64>,
    /// Expected glucose outcome.
    #[serde(default, with = "de::num")]
    pub outcome: Option<f64>,
    /// Display line.
    #[serde(rename = "displayLine", default, with = "de::text")]
    pub display_line: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `cage`, `iage`, `bage`: time since the last device change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DeviceAge {
    /// Whether a change event was found.
    #[serde(default, with = "de::flag")]
    pub found: Option<bool>,
    /// Age in hours.
    #[serde(default, with = "de::num")]
    pub age: Option<f64>,
    /// Whole days of the age.
    #[serde(default, with = "de::num")]
    pub days: Option<f64>,
    /// Remaining hours of the age.
    #[serde(default, with = "de::num")]
    pub hours: Option<f64>,
    /// When the change happened.
    #[serde(rename = "treatmentDate", default, with = "time::millis")]
    pub treatment_date: Option<Timestamp>,
    /// Notes on the change treatment.
    #[serde(default, with = "de::text")]
    pub notes: Option<String>,
    /// Alarm level.
    #[serde(default, with = "de::int")]
    pub level: Option<i64>,
    /// Display string, e.g. `2d3h`.
    #[serde(default, with = "de::text")]
    pub display: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `sage`: sensor age, tracked from both `Sensor Start` and `Sensor Change`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SensorAge {
    /// Age since the last `Sensor Start`.
    #[serde(rename = "Sensor Start", default, deserialize_with = "de::tolerant")]
    pub sensor_start: Option<DeviceAge>,
    /// Age since the last `Sensor Change`.
    #[serde(rename = "Sensor Change", default, deserialize_with = "de::tolerant")]
    pub sensor_change: Option<DeviceAge>,
    /// Which of the two is the most recent.
    #[serde(default, with = "de::text")]
    pub min: Option<String>,
}

impl SensorAge {
    /// The age from the most recent of the two events.
    #[must_use]
    pub fn current(&self) -> Option<&DeviceAge> {
        match self.min.as_deref() {
            Some("Sensor Change") => self.sensor_change.as_ref(),
            _ => self.sensor_start.as_ref().or(self.sensor_change.as_ref()),
        }
    }
}

/// `basal`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Basal {
    /// Display string, e.g. `T: 0.850U`.
    #[serde(default, with = "de::text")]
    pub display: Option<String>,
    /// Current rates.
    #[serde(default, deserialize_with = "de::tolerant")]
    pub current: Option<BasalCurrent>,
}

/// Current basal rates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BasalCurrent {
    /// Scheduled profile basal (U/h).
    #[serde(default, with = "de::num")]
    pub basal: Option<f64>,
    /// Active temp basal (U/h).
    #[serde(default, with = "de::num")]
    pub tempbasal: Option<f64>,
    /// Extended bolus rate (U/h).
    #[serde(default, with = "de::num")]
    pub combobolusbasal: Option<f64>,
    /// Total delivered basal (U/h).
    #[serde(default, with = "de::num")]
    pub totalbasal: Option<f64>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `dbsize`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DbSize {
    /// Display string.
    #[serde(default, with = "de::text")]
    pub display: Option<String>,
    /// Alert status.
    #[serde(default, with = "de::text")]
    pub status: Option<String>,
    /// Stored data in MiB.
    #[serde(rename = "totalDataSize", default, with = "de::num")]
    pub total_data_size: Option<f64>,
    /// Percent of the configured maximum.
    #[serde(rename = "dataPercentage", default, with = "de::num")]
    pub data_percentage: Option<f64>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `runtimestate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RuntimeState {
    /// `booting`, `booted` or `loaded`.
    #[serde(default, with = "de::text")]
    pub state: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_odd_property_does_not_fail_the_rest() {
        let props: Properties = serde_json::from_value(serde_json::json!({
            "iob": {"iob": 1.25, "source": "Loop", "display": "1.25", "displayLine": "IOB: 1.25U"},
            "cob": {"cob": 30, "isDecaying": 1, "decayedBy": "2024-05-01T13:30:00.000Z", "display": 30},
            "delta": "this plugin changed its shape",
            "sage": {"Sensor Start": {"found": true, "age": 50, "display": "2d2h"}, "min": "Sensor Start"},
            "future_plugin": {"x": 1}
        }))
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(props.iob.and_then(|i| i.iob), Some(1.25));
        assert_eq!(props.cob.as_ref().and_then(|c| c.is_decaying), Some(true));
        assert!(props.delta.is_none());
        assert_eq!(
            props.sage.and_then(|s| s.current().and_then(|a| a.age)),
            Some(50.0)
        );
        assert!(props.extra.contains_key("future_plugin"));
    }
}
