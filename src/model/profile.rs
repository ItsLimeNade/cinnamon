//! The `profile` collection: basal rates, insulin sensitivity, carb ratios and targets.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

use super::{CollectionName, Extra, Meta, Timestamp, Units, de, impl_document, time};

/// A profile document: a set of named profiles (`store`) and which one is the default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProfileStore {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// Name of the profile in `store` that is active by default.
    #[serde(
        rename = "defaultProfile",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub default_profile: Option<String>,
    /// When this profile document takes effect.
    #[serde(
        rename = "startDate",
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::iso"
    )]
    pub start_date: Option<Timestamp>,
    /// Named profiles.
    #[serde(default, deserialize_with = "tolerant_store")]
    pub store: BTreeMap<String, Profile>,
    /// Units of the glucose values in the profiles.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub units: Option<String>,
    /// When the document was created.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "time::iso")]
    pub created_at: Option<Timestamp>,
    /// Creation time as epoch millis (API v3).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub date: Option<Timestamp>,
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

impl ProfileStore {
    /// A document holding a single profile named `name`, effective now.
    #[must_use]
    pub fn single(name: impl Into<String>, profile: Profile) -> Self {
        let name = name.into();
        let now = Timestamp::now();
        Self {
            meta: Meta::default(),
            default_profile: Some(name.clone()),
            start_date: Some(now),
            units: profile.units.clone(),
            store: BTreeMap::from([(name, profile)]),
            created_at: Some(now),
            date: None,
            entered_by: None,
            extra: Extra::new(),
        }
    }

    /// The default profile, if it exists in the store.
    #[must_use]
    pub fn default_entry(&self) -> Option<&Profile> {
        self.store.get(self.default_profile.as_deref()?)
    }

    fn fill_defaults(&mut self, _app: &str) {
        let t = self
            .created_at
            .or(self.start_date)
            .or(self.date)
            .unwrap_or_else(Timestamp::now);
        self.created_at.get_or_insert(t);
        self.start_date.get_or_insert(t);
        self.date.get_or_insert(t);
    }
}

impl_document!(
    ProfileStore,
    CollectionName::Profile,
    timestamp = |s| s.created_at.or(s.date).or(s.start_date),
    device = |_s| None
);

/// One named therapy profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Profile {
    /// Duration of insulin action, in hours.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub dia: Option<f64>,
    /// Carb absorption rate in g/h.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub carbs_hr: Option<f64>,
    /// Delay before carbs start absorbing, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub delay: Option<f64>,
    /// IANA time zone the schedules are expressed in (e.g. `Europe/Paris`).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub timezone: Option<String>,
    /// Units of `sens`, `target_low` and `target_high`.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub units: Option<String>,
    /// Insulin-to-carb ratios (g/U).
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub carbratio: Vec<ScheduleEntry>,
    /// Insulin sensitivity factors (glucose units per U).
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub sens: Vec<ScheduleEntry>,
    /// Basal rates (U/h).
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub basal: Vec<ScheduleEntry>,
    /// Lower bounds of the target range.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub target_low: Vec<ScheduleEntry>,
    /// Upper bounds of the target range.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub target_high: Vec<ScheduleEntry>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Profile {
    /// Parsed [`Units`] of the profile, when recognizable.
    #[must_use]
    pub fn units(&self) -> Option<Units> {
        self.units.as_deref().and_then(Units::parse)
    }

    /// Basal rate (U/h) at the given local time.
    ///
    /// Pass a date-time already converted into the profile's [`timezone`](Self::timezone).
    #[must_use]
    pub fn basal_at<Tz: TimeZone>(&self, at: &DateTime<Tz>) -> Option<f64> {
        schedule_value_at(&self.basal, seconds_of_day(at))
    }

    /// Insulin sensitivity factor at the given local time.
    #[must_use]
    pub fn isf_at<Tz: TimeZone>(&self, at: &DateTime<Tz>) -> Option<f64> {
        schedule_value_at(&self.sens, seconds_of_day(at))
    }

    /// Carb ratio at the given local time.
    #[must_use]
    pub fn carb_ratio_at<Tz: TimeZone>(&self, at: &DateTime<Tz>) -> Option<f64> {
        schedule_value_at(&self.carbratio, seconds_of_day(at))
    }

    /// Target range `(low, high)` at the given local time.
    #[must_use]
    pub fn target_at<Tz: TimeZone>(&self, at: &DateTime<Tz>) -> Option<(f64, f64)> {
        let secs = seconds_of_day(at);
        Some((
            schedule_value_at(&self.target_low, secs)?,
            schedule_value_at(&self.target_high, secs)?,
        ))
    }

    /// Total scheduled basal insulin over a day, in units.
    #[must_use]
    pub fn daily_basal(&self) -> f64 {
        let mut entries: Vec<(u32, f64)> = self
            .basal
            .iter()
            .filter_map(|e| Some((e.seconds()?, e.value)))
            .collect();
        entries.sort_by_key(|(s, _)| *s);
        entries
            .iter()
            .enumerate()
            .map(|(i, (start, rate))| {
                let end = entries.get(i + 1).map_or(86_400, |(s, _)| *s);
                rate * f64::from(end.saturating_sub(*start)) / 3600.0
            })
            .sum()
    }
}

/// A scheduled value starting at a time of day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScheduleEntry {
    /// Start time as `HH:MM`.
    #[serde(default)]
    pub time: String,
    /// The value (numbers stored as strings are accepted).
    #[serde(default, with = "de::num_or_zero")]
    pub value: f64,
    /// Start time as seconds since midnight.
    #[serde(
        rename = "timeAsSeconds",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::int"
    )]
    pub time_as_seconds: Option<i64>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl ScheduleEntry {
    /// An entry starting `seconds` after midnight.
    #[must_use]
    pub fn new(seconds: u32, value: f64) -> Self {
        Self {
            time: format!("{:02}:{:02}", seconds / 3600, seconds % 3600 / 60),
            value,
            time_as_seconds: Some(i64::from(seconds)),
            extra: Extra::new(),
        }
    }

    /// Start of the entry in seconds since midnight (`timeAsSeconds`, else parsed `time`).
    #[must_use]
    pub fn seconds(&self) -> Option<u32> {
        if let Some(secs) = self.time_as_seconds.and_then(|s| u32::try_from(s).ok()) {
            return Some(secs);
        }
        let (h, m) = self.time.trim().split_once(':')?;
        Some(h.trim().parse::<u32>().ok()? * 3600 + m.trim().parse::<u32>().ok()? * 60)
    }
}

fn seconds_of_day<Tz: TimeZone>(at: &DateTime<Tz>) -> u32 {
    at.num_seconds_from_midnight()
}

/// The value of the last entry starting at or before `secs` (schedules wrap from midnight).
fn schedule_value_at(schedule: &[ScheduleEntry], secs: u32) -> Option<f64> {
    schedule
        .iter()
        .filter_map(|e| Some((e.seconds()?, e.value)))
        .filter(|(start, _)| *start <= secs)
        .max_by_key(|(start, _)| *start)
        .or_else(|| schedule.first().map(|e| (0, e.value)))
        .map(|(_, v)| v)
}

/// Profiles that fail to decode are skipped instead of failing the whole document.
fn tolerant_store<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, Profile>, D::Error> {
    let raw = Option::<BTreeMap<String, serde_json::Value>>::deserialize(d)?.unwrap_or_default();
    Ok(raw
        .into_iter()
        .filter_map(|(name, v)| serde_json::from_value(v).ok().map(|p| (name, p)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn fixture() -> ProfileStore {
        serde_json::from_value(serde_json::json!({
            "_id": "65f1c2a8e4b0a1b2c3d4e5f6",
            "defaultProfile": "Default",
            "startDate": "2024-01-01T00:00:00.000Z",
            "created_at": "2024-01-01T00:00:00.000Z",
            "units": "mg/dl",
            "store": {
                "Default": {
                    "dia": "5",
                    "timezone": null,
                    "units": "mg/dl",
                    "carbratio": [{"time": "00:00", "value": "10"}],
                    "sens": [{"time": "00:00", "value": 40, "timeAsSeconds": "0"}, {"time": "06:00", "value": "35"}],
                    "basal": [{"time": "00:00", "value": "0.8"}, {"time": "06:30", "value": 1.1}, {"time": "22:00", "value": 0.9}],
                    "target_low": [{"time": "00:00", "value": 90}],
                    "target_high": [{"time": "00:00", "value": 120}]
                }
            }
        }))
        .unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn string_values_and_null_timezone_decode() {
        let store = fixture();
        let p = store
            .default_entry()
            .unwrap_or_else(|| panic!("no default"));
        assert_eq!(p.dia, Some(5.0));
        assert_eq!(p.timezone, None);
        assert!((p.carbratio[0].value - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn schedule_lookup() {
        let store = fixture();
        let p = store
            .default_entry()
            .unwrap_or_else(|| panic!("no default"));
        let at = |h, m| {
            Utc.with_ymd_and_hms(2024, 5, 1, h, m, 0)
                .single()
                .unwrap_or_default()
        };
        assert_eq!(p.basal_at(&at(3, 0)), Some(0.8));
        assert_eq!(p.basal_at(&at(6, 30)), Some(1.1));
        assert_eq!(p.basal_at(&at(23, 59)), Some(0.9));
        assert_eq!(p.isf_at(&at(7, 0)), Some(35.0));
        assert_eq!(p.target_at(&at(12, 0)), Some((90.0, 120.0)));
        assert!((p.daily_basal() - (6.5 * 0.8 + 15.5 * 1.1 + 2.0 * 0.9)).abs() < 1e-9);
    }
}
