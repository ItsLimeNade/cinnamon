//! Alarm levels and remote-command notifications.

use serde::{Deserialize, Serialize};

use super::{Extra, Timestamp, time};

/// Nightscout notification levels (`LEVEL_*` in `lib/constants.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Level {
    /// No notification (-3).
    None,
    /// Lowest (-2).
    Lowest,
    /// Low (-1).
    Low,
    /// Informational (0).
    Info,
    /// Warning (1): `alarm` events.
    Warn,
    /// Urgent (2): `urgent_alarm` events.
    Urgent,
}

impl Level {
    /// The numeric level.
    #[must_use]
    pub const fn as_i8(self) -> i8 {
        match self {
            Self::None => -3,
            Self::Lowest => -2,
            Self::Low => -1,
            Self::Info => 0,
            Self::Warn => 1,
            Self::Urgent => 2,
        }
    }

    /// From a numeric level (values out of range clamp to the nearest level).
    #[must_use]
    pub const fn from_i64(level: i64) -> Self {
        match level {
            i64::MIN..=-3 => Self::None,
            -2 => Self::Lowest,
            -1 => Self::Low,
            0 => Self::Info,
            1 => Self::Warn,
            _ => Self::Urgent,
        }
    }
}

impl Serialize for Level {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i8(self.as_i8())
    }
}

impl<'de> Deserialize<'de> for Level {
    /// Accepts integers, floats and numeric strings.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        super::de::int::deserialize(d)?
            .map(Self::from_i64)
            .ok_or_else(|| serde::de::Error::custom("expected a numeric alarm level"))
    }
}

/// A remote command for Loop, delivered by Nightscout as an Apple push notification
/// (`POST /api/v2/notifications/loop`). Nightscout stores nothing; Loop acts on it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopNotification {
    /// `Temporary Override`, `Temporary Override Cancel`, `Remote Carbs Entry` or
    /// `Remote Bolus Entry`.
    #[serde(rename = "eventType")]
    pub event_type: String,
    /// Override preset name (for overrides).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Override display text.
    #[serde(
        rename = "reasonDisplay",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reason_display: Option<String>,
    /// Override duration in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Carbs in grams (remote carbs).
    #[serde(
        rename = "remoteCarbs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub remote_carbs: Option<f64>,
    /// Absorption time in hours (remote carbs).
    #[serde(
        rename = "remoteAbsorption",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub remote_absorption: Option<f64>,
    /// Bolus in units (remote bolus).
    #[serde(
        rename = "remoteBolus",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub remote_bolus: Option<f64>,
    /// One-time password required by Loop for carbs and boluses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub otp: Option<String>,
    /// Notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Who sent the command.
    #[serde(rename = "enteredBy", default, skip_serializing_if = "Option::is_none")]
    pub entered_by: Option<String>,
    /// When the command was created.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "time::iso")]
    pub created_at: Option<Timestamp>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl LoopNotification {
    /// Activates the override preset `name` for `minutes` (`None` = indefinitely).
    #[must_use]
    pub fn override_preset(name: impl Into<String>, minutes: Option<f64>) -> Self {
        let name = name.into();
        Self {
            event_type: "Temporary Override".into(),
            reason_display: Some(name.clone()),
            reason: Some(name),
            duration: minutes,
            created_at: Some(Timestamp::now()),
            ..Self::default()
        }
    }

    /// Cancels the active override.
    #[must_use]
    pub fn cancel_override() -> Self {
        Self {
            event_type: "Temporary Override Cancel".into(),
            created_at: Some(Timestamp::now()),
            ..Self::default()
        }
    }

    /// Enters `grams` of carbs absorbed over `absorption_hours`, authorized by `otp`.
    #[must_use]
    pub fn carbs(grams: f64, absorption_hours: f64, otp: impl Into<String>) -> Self {
        Self {
            event_type: "Remote Carbs Entry".into(),
            remote_carbs: Some(grams),
            remote_absorption: Some(absorption_hours),
            otp: Some(otp.into()),
            created_at: Some(Timestamp::now()),
            ..Self::default()
        }
    }

    /// Requests a bolus of `units`, authorized by `otp`.
    #[must_use]
    pub fn bolus(units: f64, otp: impl Into<String>) -> Self {
        Self {
            event_type: "Remote Bolus Entry".into(),
            remote_bolus: Some(units),
            otp: Some(otp.into()),
            created_at: Some(Timestamp::now()),
            ..Self::default()
        }
    }
}
