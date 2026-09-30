//! The `devicestatus` collection: uploader, pump and closed-loop (Loop, OpenAPS/AAPS/Trio)
//! status reports.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{CollectionName, Extra, Meta, Timestamp, de, impl_document, time};

/// A status report from an uploader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DeviceStatus {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// Reporting device, e.g. `openaps://samsung SM-G970F` or `loop://iPhone`.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub device: Option<String>,
    /// When the report was made.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "time::iso")]
    pub created_at: Option<Timestamp>,
    /// Report time as epoch millis (API v3).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub date: Option<Timestamp>,
    /// Phone / uploader battery.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub uploader: Option<Uploader>,
    /// Legacy top-level uploader battery percentage.
    #[serde(
        rename = "uploaderBattery",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub uploader_battery: Option<f64>,
    /// Whether the uploader is charging (AAPS).
    #[serde(
        rename = "isCharging",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::flag"
    )]
    pub is_charging: Option<bool>,
    /// Pump state.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub pump: Option<Pump>,
    /// OpenAPS / AAPS / Trio loop state.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub openaps: Option<OpenAps>,
    /// Loop (iOS) state.
    #[serde(
        rename = "loop",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub loop_status: Option<LoopStatus>,
    /// Active Loop override.
    #[serde(
        rename = "override",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub override_status: Option<Override>,
    /// xDrip-js / Lookout CGM transmitter state (untyped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xdripjs: Option<Value>,
    /// AAPS configuration snapshot (untyped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl DeviceStatus {
    /// An empty report from `device`, dated now.
    #[must_use]
    pub fn new(device: impl Into<String>) -> Self {
        Self {
            meta: Meta::default(),
            device: Some(device.into()),
            created_at: Some(Timestamp::now()),
            date: None,
            uploader: None,
            uploader_battery: None,
            is_charging: None,
            pump: None,
            openaps: None,
            loop_status: None,
            override_status: None,
            xdripjs: None,
            configuration: None,
            extra: Extra::new(),
        }
    }

    /// When the report was made: `created_at`, falling back to `date`.
    #[must_use]
    pub fn time(&self) -> Option<Timestamp> {
        self.created_at.or(self.date)
    }

    /// Uploader battery percentage from wherever the uploader put it.
    #[must_use]
    pub fn uploader_battery_percent(&self) -> Option<f64> {
        self.uploader
            .as_ref()
            .and_then(|u| u.battery)
            .or(self.uploader_battery)
    }

    fn fill_defaults(&mut self) {
        match (self.created_at, self.date) {
            (None, None) => {
                let now = Timestamp::now();
                self.created_at = Some(now);
                self.date = Some(now);
            }
            (Some(t), None) => self.date = Some(t),
            (None, Some(t)) => self.created_at = Some(t),
            (Some(_), Some(_)) => {}
        }
    }
}

impl_document!(
    DeviceStatus,
    CollectionName::DeviceStatus,
    timestamp = |s| s.time(),
    device = |s| s.device.as_deref()
);

/// Uploader (phone) state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Uploader {
    /// Battery percentage.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub battery: Option<f64>,
    /// Battery voltage.
    #[serde(
        rename = "batteryVoltage",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub battery_voltage: Option<f64>,
    /// Uploader name.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub name: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Insulin pump state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Pump {
    /// Pump clock (ISO).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub clock: Option<String>,
    /// Insulin left in the reservoir, in units.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub reservoir: Option<f64>,
    /// Pump battery.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub battery: Option<PumpBattery>,
    /// Pump status flags.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub status: Option<PumpStatus>,
    /// Manufacturer.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub manufacturer: Option<String>,
    /// Model.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub model: Option<String>,
    /// Pump-reported insulin on board (shape varies by uploader).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iob: Option<Value>,
    /// Extended, uploader-specific pump data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Pump battery.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PumpBattery {
    /// Percentage.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub percent: Option<f64>,
    /// Voltage.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub voltage: Option<f64>,
    /// Status string.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub status: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Pump status flags.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PumpStatus {
    /// Human-readable status (`normal`, `suspended`, …).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub status: Option<String>,
    /// Whether a bolus is in progress.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::flag")]
    pub bolusing: Option<bool>,
    /// Whether delivery is suspended.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::flag")]
    pub suspended: Option<bool>,
    /// Status time.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub timestamp: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// OpenAPS-family (OpenAPS, AAPS, Trio, iAPS) loop state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OpenAps {
    /// Insulin on board: an object, or an array of forecasts (shape varies by uploader).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iob: Option<Value>,
    /// The latest recommendation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub suggested: Option<ApsResult>,
    /// The latest recommendation that was actually applied.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub enacted: Option<ApsResult>,
    /// OpenAPS version.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub version: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl OpenAps {
    /// Current IOB in units, from either the object or the first element of the array form.
    #[must_use]
    pub fn iob_units(&self) -> Option<f64> {
        let iob = match self.iob.as_ref()? {
            Value::Array(items) => items.first()?,
            other => other,
        };
        iob.get("iob").and_then(Value::as_f64)
    }
}

/// An OpenAPS `determine-basal` result (`suggested` / `enacted`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApsResult {
    /// When the result was computed.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub timestamp: Option<String>,
    /// Current glucose used for the decision.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub bg: Option<f64>,
    /// Temp basal type (`absolute`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub temp: Option<String>,
    /// Temp basal rate in U/h.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub rate: Option<f64>,
    /// Temp basal duration in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub duration: Option<f64>,
    /// SMB delivered, in units.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub units: Option<f64>,
    /// Human-readable reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub reason: Option<String>,
    /// Predicted eventual glucose.
    #[serde(
        rename = "eventualBG",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub eventual_bg: Option<f64>,
    /// IOB used for the decision.
    #[serde(
        rename = "IOB",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub iob: Option<f64>,
    /// COB used for the decision.
    #[serde(
        rename = "COB",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub cob: Option<f64>,
    /// Autosens ratio.
    #[serde(
        rename = "sensitivityRatio",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub sensitivity_ratio: Option<f64>,
    /// Insulin required.
    #[serde(
        rename = "insulinReq",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub insulin_req: Option<f64>,
    /// Whether the pump acknowledged the command (`received`; oref0 spells it `recieved`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::flag")]
    pub received: Option<bool>,
    /// Prediction curves.
    #[serde(
        rename = "predBGs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub pred_bgs: Option<PredBgs>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// OpenAPS prediction curves (5-minute steps, mg/dL).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PredBgs {
    /// Insulin-only prediction.
    #[serde(
        rename = "IOB",
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "de::nums"
    )]
    pub iob: Vec<f64>,
    /// Prediction including carbs on board.
    #[serde(
        rename = "COB",
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "de::nums"
    )]
    pub cob: Vec<f64>,
    /// Unannounced-meal prediction.
    #[serde(
        rename = "UAM",
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "de::nums"
    )]
    pub uam: Vec<f64>,
    /// Zero-temp prediction.
    #[serde(
        rename = "ZT",
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "de::nums"
    )]
    pub zt: Vec<f64>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Loop (iOS) state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopStatus {
    /// App name (`Loop`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub name: Option<String>,
    /// App version.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub version: Option<String>,
    /// When the loop ran.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub timestamp: Option<String>,
    /// Insulin on board.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub iob: Option<LoopValue>,
    /// Carbs on board.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub cob: Option<LoopValue>,
    /// Glucose prediction.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de::tolerant"
    )]
    pub predicted: Option<LoopPrediction>,
    /// Last enacted temp basal / bolus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enacted: Option<Value>,
    /// Recommended bolus in units.
    #[serde(
        rename = "recommendedBolus",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::num"
    )]
    pub recommended_bolus: Option<f64>,
    /// Why the last loop failed, if it did.
    #[serde(
        rename = "failureReason",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub failure_reason: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A Loop IOB or COB value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopValue {
    /// IOB in units (for `iob`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub iob: Option<f64>,
    /// COB in grams (for `cob`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub cob: Option<f64>,
    /// When the value was computed.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub timestamp: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A Loop glucose prediction.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopPrediction {
    /// Start of the prediction (ISO).
    #[serde(
        rename = "startDate",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub start_date: Option<String>,
    /// Predicted values in 5-minute steps (mg/dL).
    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "de::nums")]
    pub values: Vec<f64>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A Loop override (preset or custom).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Override {
    /// Preset name.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub name: Option<String>,
    /// Whether it is active.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::flag")]
    pub active: Option<bool>,
    /// Insulin needs multiplier.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub multiplier: Option<f64>,
    /// Remaining duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub duration: Option<f64>,
    /// When it started.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub timestamp: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_status_is_typed_not_lost() {
        let raw = serde_json::json!({
            "_id": "65f1c2a8e4b0a1b2c3d4e5f6",
            "device": "loop://iPhone",
            "created_at": "2024-05-01T12:00:00.000Z",
            "pump": {"clock": "2024-05-01T12:00:00Z", "reservoir": 104.5, "battery": {"percent": 75},
                     "status": {"status": "normal", "bolusing": false, "suspended": false}},
            "uploader": {"battery": 58, "name": "iPhone"},
            "loop": {"name": "Loop", "version": "3.4.0", "timestamp": "2024-05-01T12:00:00Z",
                     "iob": {"iob": 1.25, "timestamp": "2024-05-01T12:00:00Z"},
                     "cob": {"cob": 12, "timestamp": "2024-05-01T12:00:00Z"},
                     "predicted": {"startDate": "2024-05-01T12:00:00Z", "values": [110, 112, 115]},
                     "recommendedBolus": 0},
            "override": {"name": "Workout", "active": true, "multiplier": 0.5}
        });
        let ds: DeviceStatus =
            serde_json::from_value(raw.clone()).unwrap_or_else(|e| panic!("{e}"));
        let lp = ds
            .loop_status
            .as_ref()
            .unwrap_or_else(|| panic!("loop lost"));
        assert_eq!(lp.iob.as_ref().and_then(|i| i.iob), Some(1.25));
        assert_eq!(lp.predicted.as_ref().map(|p| p.values.len()), Some(3));
        assert_eq!(ds.uploader_battery_percent(), Some(58.0));
        assert_eq!(serde_json::to_value(&ds).ok(), Some(raw));
    }

    #[test]
    fn openaps_iob_array_form() {
        let ds: DeviceStatus = serde_json::from_value(serde_json::json!({
            "created_at": "2024-05-01T12:00:00Z",
            "openaps": {"iob": [{"iob": 2.1, "activity": 0.02}, {"iob": 1.9}],
                        "suggested": {"bg": 140, "eventualBG": 120, "predBGs": {"IOB": [140, 138], "ZT": [140]}}}
        }))
        .unwrap_or_else(|e| panic!("{e}"));
        let aps = ds.openaps.unwrap_or_default();
        assert_eq!(aps.iob_units(), Some(2.1));
        assert_eq!(
            aps.suggested.and_then(|s| s.pred_bgs).map(|p| p.iob),
            Some(vec![140.0, 138.0])
        );
    }
}
