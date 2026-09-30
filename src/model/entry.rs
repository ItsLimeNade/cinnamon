//! The `entries` collection: sensor glucose (`sgv`), meter glucose (`mbg`) and sensor
//! calibrations (`cal`).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{CollectionName, Direction, Extra, Glucose, Meta, Timestamp, de, impl_document};

/// A sensor glucose value (`type: "sgv"`) from a CGM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sgv {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    /// The glucose value (mg/dL).
    pub sgv: Glucose,
    /// When the reading was taken.
    pub date: Timestamp,
    /// The uploader's ISO rendering of `date` (API v3 uploads usually omit it).
    #[serde(
        rename = "dateString",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub date_string: Option<String>,
    /// Server-normalized ISO time (API v1 uploads only).
    #[serde(
        rename = "sysTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub sys_time: Option<String>,
    /// Trend arrow.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_direction"
    )]
    pub direction: Option<Direction>,
    /// Sensor noise level (1 = clean … 4 = heavy).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::int32")]
    pub noise: Option<i32>,
    /// Filtered raw sensor signal.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub filtered: Option<f64>,
    /// Unfiltered raw sensor signal.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub unfiltered: Option<f64>,
    /// Signal strength.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub rssi: Option<f64>,
    /// Change from the previous reading, as computed by the uploader.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub delta: Option<f64>,
    /// The uploading device or app (e.g. `xDrip-DexcomG6`, `loop://iPhone`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub device: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Sgv {
    /// A new reading at `at`. The device defaults to the client's app name on upload.
    #[must_use]
    pub fn new(sgv: impl Into<Glucose>, at: impl Into<Timestamp>) -> Self {
        let date = at.into();
        Self {
            meta: Meta::default(),
            kind: Some("sgv".into()),
            sgv: sgv.into(),
            date,
            date_string: None,
            sys_time: None,
            direction: None,
            noise: None,
            filtered: None,
            unfiltered: None,
            rssi: None,
            delta: None,
            device: None,
            extra: Extra::new(),
        }
    }

    /// Sets the trend arrow.
    #[must_use]
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Sets the device name.
    #[must_use]
    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }

    /// Sets the noise level.
    #[must_use]
    pub const fn with_noise(mut self, noise: i32) -> Self {
        self.noise = Some(noise);
        self
    }

    fn fill_defaults(&mut self) {
        self.kind.get_or_insert_with(|| "sgv".into());
        self.date_string.get_or_insert_with(|| self.date.to_iso());
    }
}

impl_document!(
    Sgv,
    CollectionName::Entries,
    timestamp = |s| Some(s.date),
    device = |s| s.device.as_deref()
);

/// A meter (fingerstick) glucose value (`type: "mbg"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Mbg {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    /// The glucose value (mg/dL).
    pub mbg: Glucose,
    /// When the measurement was taken.
    pub date: Timestamp,
    /// The uploader's ISO rendering of `date`.
    #[serde(
        rename = "dateString",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub date_string: Option<String>,
    /// Server-normalized ISO time (API v1 uploads only).
    #[serde(
        rename = "sysTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub sys_time: Option<String>,
    /// The meter or uploading app.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub device: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Mbg {
    /// A new meter reading at `at`.
    #[must_use]
    pub fn new(mbg: impl Into<Glucose>, at: impl Into<Timestamp>) -> Self {
        Self {
            meta: Meta::default(),
            kind: Some("mbg".into()),
            mbg: mbg.into(),
            date: at.into(),
            date_string: None,
            sys_time: None,
            device: None,
            extra: Extra::new(),
        }
    }

    /// Sets the device name.
    #[must_use]
    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }

    fn fill_defaults(&mut self) {
        self.kind.get_or_insert_with(|| "mbg".into());
        self.date_string.get_or_insert_with(|| self.date.to_iso());
    }
}

impl_document!(
    Mbg,
    CollectionName::Entries,
    timestamp = |s| Some(s.date),
    device = |s| s.device.as_deref()
);

/// A sensor calibration (`type: "cal"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Cal {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    /// When the calibration was applied.
    pub date: Timestamp,
    /// The uploader's ISO rendering of `date`.
    #[serde(
        rename = "dateString",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub date_string: Option<String>,
    /// Calibration slope.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub slope: Option<f64>,
    /// Calibration intercept.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub intercept: Option<f64>,
    /// Calibration scale.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub scale: Option<f64>,
    /// The uploading device.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub device: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Cal {
    /// A new calibration record at `at`.
    #[must_use]
    pub fn new(at: impl Into<Timestamp>, slope: f64, intercept: f64, scale: f64) -> Self {
        Self {
            meta: Meta::default(),
            kind: Some("cal".into()),
            date: at.into(),
            date_string: None,
            slope: Some(slope),
            intercept: Some(intercept),
            scale: Some(scale),
            device: None,
            extra: Extra::new(),
        }
    }

    fn fill_defaults(&mut self) {
        self.kind.get_or_insert_with(|| "cal".into());
        self.date_string.get_or_insert_with(|| self.date.to_iso());
    }
}

impl_document!(
    Cal,
    CollectionName::Entries,
    timestamp = |s| Some(s.date),
    device = |s| s.device.as_deref()
);

/// Any document of the `entries` collection, dispatched on its `type`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Entry {
    /// A sensor glucose value.
    Sgv(Sgv),
    /// A meter glucose value.
    Mbg(Mbg),
    /// A calibration.
    Cal(Cal),
    /// Any other entry type, kept as raw JSON.
    Other(RawEntry),
}

/// An entry of a type cinnamon does not model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RawEntry {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// The entry fields, including `type` and `date`.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Entry {
    /// The entry time, when known.
    #[must_use]
    pub fn date(&self) -> Option<Timestamp> {
        match self {
            Self::Sgv(e) => Some(e.date),
            Self::Mbg(e) => Some(e.date),
            Self::Cal(e) => Some(e.date),
            Self::Other(raw) => raw.extra.get("date").and_then(Timestamp::from_value),
        }
    }

    /// The entry `type` (`sgv`, `mbg`, `cal`, …).
    #[must_use]
    pub fn kind(&self) -> Option<&str> {
        match self {
            Self::Sgv(_) => Some("sgv"),
            Self::Mbg(_) => Some("mbg"),
            Self::Cal(_) => Some("cal"),
            Self::Other(raw) => raw.extra.get("type").and_then(Value::as_str),
        }
    }
}

impl Serialize for Entry {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Sgv(e) => e.serialize(s),
            Self::Mbg(e) => e.serialize(s),
            Self::Cal(e) => e.serialize(s),
            Self::Other(raw) => raw.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        let kind = value.get("type").and_then(Value::as_str).map(str::to_owned);
        let parse = |v: Value| -> Result<Self, D::Error> {
            let res = match kind.as_deref() {
                Some("sgv") => serde_json::from_value(v).map(Self::Sgv),
                Some("mbg") => serde_json::from_value(v).map(Self::Mbg),
                Some("cal") => serde_json::from_value(v).map(Self::Cal),
                _ => serde_json::from_value(v).map(Self::Other),
            };
            res.map_err(serde::de::Error::custom)
        };
        parse(value)
    }
}

impl super::sealed::Sealed for Entry {}

impl super::Document for Entry {
    const COLLECTION: CollectionName = CollectionName::Entries;

    fn meta(&self) -> &Meta {
        match self {
            Self::Sgv(e) => &e.meta,
            Self::Mbg(e) => &e.meta,
            Self::Cal(e) => &e.meta,
            Self::Other(e) => &e.meta,
        }
    }

    fn meta_mut(&mut self) -> &mut Meta {
        match self {
            Self::Sgv(e) => &mut e.meta,
            Self::Mbg(e) => &mut e.meta,
            Self::Cal(e) => &mut e.meta,
            Self::Other(e) => &mut e.meta,
        }
    }

    fn timestamp(&self) -> Option<Timestamp> {
        self.date()
    }

    fn device(&self) -> Option<&str> {
        match self {
            Self::Sgv(e) => e.device.as_deref(),
            Self::Mbg(e) => e.device.as_deref(),
            Self::Cal(e) => e.device.as_deref(),
            Self::Other(e) => e.extra.get("device").and_then(Value::as_str),
        }
    }

    fn prepare_for_upload(&mut self, app: &str) {
        match self {
            Self::Sgv(e) => super::Document::prepare_for_upload(e, app),
            Self::Mbg(e) => super::Document::prepare_for_upload(e, app),
            Self::Cal(e) => super::Document::prepare_for_upload(e, app),
            Self::Other(_) => super::prepare_meta(self, app),
        }
    }
}

/// Directions arrive as strings, but some uploaders send `null`, numbers or nothing.
fn opt_direction<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Direction>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) if !s.is_empty() => Some(Direction::parse(&s)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdrip_sgv_round_trips_losslessly() {
        let raw = serde_json::json!({
            "_id": "65f1c2a8e4b0a1b2c3d4e5f6",
            "device": "xDrip-DexcomG6",
            "date": 1714564800000_i64,
            "dateString": "2024-05-01T14:00:00.000+0200",
            "sgv": 123,
            "delta": -2.5,
            "direction": "FortyFiveDown",
            "type": "sgv",
            "filtered": 131_000,
            "unfiltered": 131_000,
            "rssi": 100,
            "noise": 1,
            "sysTime": "2024-05-01T14:00:00.000+0200",
            "utcOffset": 120,
            "custom_field": {"kept": true}
        });
        let sgv: Sgv = serde_json::from_value(raw.clone()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(sgv.direction, Some(Direction::FortyFiveDown));
        assert_eq!(sgv.meta.utc_offset, Some(120));
        assert_eq!(serde_json::to_value(&sgv).ok(), Some(raw));
    }

    #[test]
    fn lenient_on_broken_uploads() {
        let sgv: Sgv = serde_json::from_value(serde_json::json!({
            "sgv": "171", "date": "1714564800000", "direction": null, "noise": "", "type": "sgv"
        }))
        .unwrap_or_else(|e| panic!("{e}"));
        assert!((sgv.sgv.as_mgdl() - 171.0).abs() < f64::EPSILON);
        assert_eq!(sgv.direction, None);
        assert_eq!(sgv.noise, None);
    }

    #[test]
    fn entry_dispatches_on_type() {
        let entries: Vec<Entry> = serde_json::from_value(serde_json::json!([
            {"type": "sgv", "sgv": 100, "date": 1},
            {"type": "mbg", "mbg": 104, "date": 2},
            {"type": "cal", "slope": 900, "intercept": 30000, "scale": 1, "date": 3},
            {"type": "sensor", "date": 4}
        ]))
        .unwrap_or_default();
        assert!(matches!(entries[0], Entry::Sgv(_)));
        assert!(matches!(entries[1], Entry::Mbg(_)));
        assert!(matches!(entries[2], Entry::Cal(_)));
        assert_eq!(entries[3].kind(), Some("sensor"));
    }
}
