//! Server status, versions, authorization checks and admin notifications.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Extra, Timestamp, Units, de, time};

/// `GET /api/v3/version`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct V3Version {
    /// Nightscout release, e.g. `15.0.8`.
    #[serde(default, with = "de::text")]
    pub version: Option<String>,
    /// API v3 version, e.g. `3.0.5`.
    #[serde(rename = "apiVersion", default, with = "de::text")]
    pub api_version: Option<String>,
    /// Server time.
    #[serde(rename = "srvDate", default, with = "time::millis")]
    pub srv_date: Option<Timestamp>,
    /// Storage backend.
    #[serde(default, deserialize_with = "de::tolerant")]
    pub storage: Option<StorageInfo>,
}

/// Storage backend details.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StorageInfo {
    /// Backend name (`mongodb`).
    #[serde(default, alias = "type", with = "de::text")]
    pub storage: Option<String>,
    /// Backend version.
    #[serde(default, with = "de::text")]
    pub version: Option<String>,
}

/// `GET /api/v3/status`: version info plus the caller's permissions per collection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct V3Status {
    /// Version details.
    #[serde(flatten)]
    pub version: V3Version,
    /// Collection name → granted operations as a `crud` subset (e.g. `"cr"`).
    ///
    /// Nightscout 15.0.8 only reports wildcard grants correctly here; treat it as advisory.
    #[serde(rename = "apiPermissions", default)]
    pub api_permissions: BTreeMap<String, String>,
}

/// `GET /api/v3/lastModified`: newest change time per collection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LastModified {
    /// Server time.
    #[serde(rename = "srvDate", default, with = "time::millis")]
    pub srv_date: Option<Timestamp>,
    /// Collection name → newest modification time. Collections without data are absent.
    #[serde(default)]
    pub collections: BTreeMap<String, Timestamp>,
}

/// One entry of `GET /api/versions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApiVersionInfo {
    /// API version (`1.0.0`, `2.0.0`, `3.0.5`).
    pub version: String,
    /// Mount point (`/api/v1`, …).
    pub url: String,
}

/// `GET /api/v1/status.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Status {
    /// `ok` when the server is healthy.
    #[serde(default, with = "de::text")]
    pub status: Option<String>,
    /// Site name.
    #[serde(default, with = "de::text")]
    pub name: Option<String>,
    /// Nightscout release.
    #[serde(default, with = "de::text")]
    pub version: Option<String>,
    /// Server time.
    #[serde(rename = "serverTime", default, with = "time::iso")]
    pub server_time: Option<Timestamp>,
    /// Server time as epoch millis.
    #[serde(rename = "serverTimeEpoch", default, with = "time::millis")]
    pub server_time_epoch: Option<Timestamp>,
    /// Whether the REST API accepts writes.
    #[serde(rename = "apiEnabled", default, with = "de::flag")]
    pub api_enabled: Option<bool>,
    /// Whether the careportal is enabled.
    #[serde(rename = "careportalEnabled", default, with = "de::flag")]
    pub careportal_enabled: Option<bool>,
    /// Whether the bolus calculator is enabled.
    #[serde(rename = "boluscalcEnabled", default, with = "de::flag")]
    pub boluscalc_enabled: Option<bool>,
    /// Public server settings.
    #[serde(default, deserialize_with = "de::tolerant")]
    pub settings: Option<Settings>,
    /// Plugin-specific settings.
    #[serde(rename = "extendedSettings", default)]
    pub extended_settings: Option<Value>,
    /// The caller's access-token authorization, when a token was supplied.
    #[serde(default)]
    pub authorized: Option<Value>,
    /// `booting`, `booted` or `loaded`.
    #[serde(rename = "runtimeState", default, with = "de::text")]
    pub runtime_state: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Public server settings from `status.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Settings {
    /// Display units (`mg/dl` or `mmol`).
    #[serde(default, with = "de::text")]
    pub units: Option<String>,
    /// 12 or 24.
    #[serde(rename = "timeFormat", default, with = "de::int")]
    pub time_format: Option<i64>,
    /// Site title.
    #[serde(rename = "customTitle", default, with = "de::text")]
    pub custom_title: Option<String>,
    /// UI theme.
    #[serde(default, with = "de::text")]
    pub theme: Option<String>,
    /// UI language.
    #[serde(default, with = "de::text")]
    pub language: Option<String>,
    /// Night mode enabled.
    #[serde(rename = "nightMode", default, with = "de::flag")]
    pub night_mode: Option<bool>,
    /// Roles granted to anonymous visitors.
    #[serde(rename = "authDefaultRoles", default, with = "de::text")]
    pub auth_default_roles: Option<String>,
    /// Enabled plugins and features.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub enable: Vec<String>,
    /// Plugins shown by default.
    #[serde(rename = "showPlugins", default, with = "de::text")]
    pub show_plugins: Option<String>,
    /// Alarm types (`simple`, `predict`).
    #[serde(rename = "alarmTypes", default, deserialize_with = "de::null_as_empty")]
    pub alarm_types: Vec<String>,
    /// Urgent-high alarm enabled.
    #[serde(rename = "alarmUrgentHigh", default, with = "de::flag")]
    pub alarm_urgent_high: Option<bool>,
    /// High alarm enabled.
    #[serde(rename = "alarmHigh", default, with = "de::flag")]
    pub alarm_high: Option<bool>,
    /// Low alarm enabled.
    #[serde(rename = "alarmLow", default, with = "de::flag")]
    pub alarm_low: Option<bool>,
    /// Urgent-low alarm enabled.
    #[serde(rename = "alarmUrgentLow", default, with = "de::flag")]
    pub alarm_urgent_low: Option<bool>,
    /// Stale-data warning enabled.
    #[serde(rename = "alarmTimeagoWarn", default, with = "de::flag")]
    pub alarm_timeago_warn: Option<bool>,
    /// Minutes before the stale-data warning.
    #[serde(rename = "alarmTimeagoWarnMins", default, with = "de::int")]
    pub alarm_timeago_warn_mins: Option<i64>,
    /// Stale-data urgent alarm enabled.
    #[serde(rename = "alarmTimeagoUrgent", default, with = "de::flag")]
    pub alarm_timeago_urgent: Option<bool>,
    /// Minutes before the stale-data urgent alarm.
    #[serde(rename = "alarmTimeagoUrgentMins", default, with = "de::int")]
    pub alarm_timeago_urgent_mins: Option<i64>,
    /// Glucose thresholds.
    #[serde(default, deserialize_with = "de::tolerant")]
    pub thresholds: Option<Thresholds>,
    /// Hours shown in the main chart.
    #[serde(rename = "focusHours", default, with = "de::int")]
    pub focus_hours: Option<i64>,
    /// Client heartbeat, seconds.
    #[serde(default, with = "de::int")]
    pub heartbeat: Option<i64>,
    /// Public base URL.
    #[serde(rename = "baseURL", default, with = "de::text")]
    pub base_url: Option<String>,
    /// Seconds added per failed authentication.
    #[serde(rename = "authFailDelay", default, with = "de::int")]
    pub auth_fail_delay: Option<i64>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Settings {
    /// Parsed display units.
    #[must_use]
    pub fn display_units(&self) -> Option<Units> {
        self.units.as_deref().and_then(Units::parse)
    }

    /// Whether a plugin or feature is enabled (e.g. `"iob"`, `"careportal"`).
    #[must_use]
    pub fn is_enabled(&self, feature: &str) -> bool {
        self.enable.iter().any(|f| f == feature)
    }
}

/// Alarm thresholds (mg/dL).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Thresholds {
    /// Urgent-high threshold.
    #[serde(rename = "bgHigh", default, with = "de::num")]
    pub bg_high: Option<f64>,
    /// Top of the target range.
    #[serde(rename = "bgTargetTop", default, with = "de::num")]
    pub bg_target_top: Option<f64>,
    /// Bottom of the target range.
    #[serde(rename = "bgTargetBottom", default, with = "de::num")]
    pub bg_target_bottom: Option<f64>,
    /// Urgent-low threshold.
    #[serde(rename = "bgLow", default, with = "de::num")]
    pub bg_low: Option<f64>,
}

/// `GET /api/v1/verifyauth`: what the current credentials can do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VerifyAuth {
    /// Read access.
    #[serde(rename = "canRead", default)]
    pub can_read: bool,
    /// Write access.
    #[serde(rename = "canWrite", default)]
    pub can_write: bool,
    /// Admin access (API secret or admin role).
    #[serde(rename = "isAdmin", default)]
    pub is_admin: bool,
    /// `OK` or `UNAUTHORIZED`.
    #[serde(default)]
    pub message: String,
    /// `FOUND` when an access token matched a subject.
    #[serde(default)]
    pub rolefound: String,
    /// `DEFAULT` (anonymous) or `ROLE`.
    #[serde(default)]
    pub permissions: String,
}

/// `GET /api/v1/adminnotifies`: messages Nightscout shows to administrators.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AdminNotifies {
    /// Notifications (only returned to admins).
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub notifies: Vec<AdminNotify>,
    /// Total count.
    #[serde(rename = "notifyCount", default)]
    pub notify_count: u64,
}

/// An admin notification.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AdminNotify {
    /// Title.
    #[serde(default, with = "de::text")]
    pub title: Option<String>,
    /// Message.
    #[serde(default, with = "de::text")]
    pub message: Option<String>,
    /// Occurrences.
    #[serde(default, with = "de::int")]
    pub count: Option<i64>,
    /// Last occurrence.
    #[serde(rename = "lastRecorded", default, with = "time::millis")]
    pub last_recorded: Option<Timestamp>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}
