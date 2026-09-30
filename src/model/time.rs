//! Timestamps as Nightscout stores them.

use std::fmt;

use chrono::{DateTime, FixedOffset, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Epoch milliseconds below this are read as epoch *seconds* (the same rule API v3 applies:
/// 2000-01-01T00:00:00Z in milliseconds).
const MIN_MILLIS: i64 = 946_684_800_000;

/// A UTC instant with millisecond precision.
///
/// Deserializes from everything Nightscout and its uploaders produce: epoch milliseconds,
/// epoch seconds, numeric strings, ISO-8601/RFC 3339 (with or without offset, `T` or space
/// separator) and RFC 2822. Serializes as epoch milliseconds, or as an ISO string
/// (`2024-05-01T12:00:00.000Z`) for fields such as `created_at` (see [`iso`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// The Unix epoch.
    pub const EPOCH: Self = Self(0);

    /// From epoch milliseconds.
    #[must_use]
    pub const fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    /// The current time.
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now().timestamp_millis())
    }

    /// Epoch milliseconds.
    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }

    /// As a `chrono` UTC date-time (clamped to chrono's supported range).
    #[must_use]
    pub fn to_datetime(self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.0).unwrap_or_default()
    }

    /// ISO-8601 with milliseconds and `Z`, the format Nightscout stores in `created_at`.
    #[must_use]
    pub fn to_iso(self) -> String {
        self.to_datetime()
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    /// Parses any of the formats listed on [`Timestamp`].
    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        if s.is_empty() {
            return None;
        }
        if let Ok(n) = s.parse::<i64>() {
            return Some(Self::from_number(n));
        }
        if let Ok(f) = s.parse::<f64>() {
            return f.is_finite().then(|| Self::from_number(f.round() as i64));
        }
        parse_datetime(s).map(|dt| Self(dt.timestamp_millis()))
    }

    const fn from_number(n: i64) -> Self {
        if n > 0 && n < MIN_MILLIS {
            Self(n.saturating_mul(1000))
        } else {
            Self(n)
        }
    }

    pub(crate) fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => n
                .as_i64()
                .or_else(|| {
                    n.as_f64()
                        .filter(|f| f.is_finite())
                        .map(|f| f.round() as i64)
                })
                .map(Self::from_number),
            Value::String(s) => Self::parse(s),
            _ => None,
        }
    }
}

fn parse_datetime(s: &str) -> Option<DateTime<FixedOffset>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt);
    }
    let normalized = s.replacen(' ', "T", 1);
    if let Ok(dt) = DateTime::parse_from_rfc3339(&normalized) {
        return Some(dt);
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%dT%H:%M:%S%z"] {
        if let Ok(dt) = DateTime::parse_from_str(&normalized, fmt) {
            return Some(dt);
        }
    }
    // No offset at all: Nightscout treats those as UTC.
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&normalized, fmt) {
            return Some(Utc.from_utc_datetime(&naive).fixed_offset());
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|naive| Utc.from_utc_datetime(&naive).fixed_offset());
    }
    DateTime::parse_from_rfc2822(s).ok()
}

impl<Tz: TimeZone> From<DateTime<Tz>> for Timestamp {
    fn from(dt: DateTime<Tz>) -> Self {
        Self(dt.timestamp_millis())
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        ts.to_datetime()
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({})", self.to_iso())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iso())
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64(self.0)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        Self::from_value(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("expected a timestamp, found {value}")))
    }
}

/// Lenient `Option<Timestamp>`: unparsable values (`null`, `""`, garbage) become `None`
/// instead of failing the document.
pub(crate) fn opt_lenient<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Timestamp>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?
        .as_ref()
        .and_then(Timestamp::from_value))
}

/// `#[serde(with = "iso")]` for `Option<Timestamp>` fields stored as ISO strings
/// (`created_at`, `startDate`, …): serializes as `2024-05-01T12:00:00.000Z`.
pub mod iso {
    use super::{Timestamp, opt_lenient};
    use serde::{Deserializer, Serializer};

    /// Serializes as an ISO-8601 string with milliseconds and `Z`.
    ///
    /// # Errors
    ///
    /// Only those of the underlying serializer.
    pub fn serialize<S: Serializer>(ts: &Option<Timestamp>, s: S) -> Result<S::Ok, S::Error> {
        match ts {
            Some(ts) => s.serialize_str(&ts.to_iso()),
            None => s.serialize_none(),
        }
    }

    /// Deserializes leniently (see [`Timestamp`]).
    ///
    /// # Errors
    ///
    /// Never for well-formed JSON; unparsable values become `None`.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Timestamp>, D::Error> {
        opt_lenient(d)
    }
}

/// `#[serde(with = "millis")]` for lenient `Option<Timestamp>` fields stored as numbers.
pub mod millis {
    use super::{Timestamp, opt_lenient};
    use serde::{Deserializer, Serialize, Serializer};

    /// Serializes as epoch milliseconds.
    ///
    /// # Errors
    ///
    /// Only those of the underlying serializer.
    pub fn serialize<S: Serializer>(ts: &Option<Timestamp>, s: S) -> Result<S::Ok, S::Error> {
        ts.serialize(s)
    }

    /// Deserializes leniently (see [`Timestamp`]).
    ///
    /// # Errors
    ///
    /// Never for well-formed JSON; unparsable values become `None`.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Timestamp>, D::Error> {
        opt_lenient(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: i64 = 1_714_564_800_000; // 2024-05-01T12:00:00Z

    #[test]
    fn parses_every_uploader_format() {
        for input in [
            "1714564800000",
            "1714564800",
            "2024-05-01T12:00:00Z",
            "2024-05-01T12:00:00.000Z",
            "2024-05-01T14:00:00+02:00",
            "2024-05-01T14:00:00.000+0200",
            "2024-05-01 12:00:00Z",
            "2024-05-01T12:00:00",
            "Wed, 01 May 2024 12:00:00 +0000",
        ] {
            assert_eq!(
                Timestamp::parse(input).map(Timestamp::as_millis),
                Some(T),
                "{input}"
            );
        }
        assert_eq!(Timestamp::parse(""), None);
        assert_eq!(Timestamp::parse("now"), None);
    }

    #[test]
    fn numbers_below_2000_are_seconds() {
        let v: Timestamp = serde_json::from_str("1714564800").unwrap_or(Timestamp::EPOCH);
        assert_eq!(v.as_millis(), T);
        let v: Timestamp = serde_json::from_str("1714564800000.0").unwrap_or(Timestamp::EPOCH);
        assert_eq!(v.as_millis(), T);
    }

    #[test]
    fn iso_output_matches_nightscout() {
        assert_eq!(
            Timestamp::from_millis(T).to_iso(),
            "2024-05-01T12:00:00.000Z"
        );
    }
}
