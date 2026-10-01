//! Typed socket.io events.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::ddata::tolerant_vec;
use crate::model::notification::Level;
use crate::model::properties::PropertySgv;
use crate::model::{
    CollectionName, DeviceStatus, Extra, ProfileStore, Timestamp, Treatment, de, time,
};
use crate::sync::SyncDoc;

/// A change broadcast on the `/storage` channel.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum StorageEvent {
    /// A document was created through API v3.
    Created(SyncDoc),
    /// A document was replaced or patched through API v3.
    Updated(SyncDoc),
    /// A document was deleted through API v3.
    Deleted {
        /// Its collection.
        collection: CollectionName,
        /// Its identifier.
        identifier: String,
    },
    /// The connection dropped and was re-established; events may have been missed.
    Reconnected,
}

impl StorageEvent {
    pub(crate) fn decode(name: &str, mut args: Vec<Value>) -> Option<Self> {
        let payload = if args.is_empty() {
            return None;
        } else {
            args.swap_remove(0)
        };
        let collection: CollectionName =
            serde_json::from_value(payload.get("colName")?.clone()).ok()?;
        match name {
            "create" | "update" => {
                let doc = SyncDoc {
                    collection,
                    doc: payload.get("doc")?.clone(),
                };
                Some(if name == "create" {
                    Self::Created(doc)
                } else {
                    Self::Updated(doc)
                })
            }
            "delete" => Some(Self::Deleted {
                collection,
                identifier: payload.get("identifier")?.as_str()?.to_owned(),
            }),
            _ => None,
        }
    }
}

/// An alarm or notification from the `/alarm` channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Alarm {
    /// Severity.
    #[serde(default = "info")]
    pub level: Level,
    /// Title, e.g. `Urgent LOW`.
    #[serde(default, with = "de::text")]
    pub title: Option<String>,
    /// Message body.
    #[serde(default, with = "de::text")]
    pub message: Option<String>,
    /// Alarm group (needed to acknowledge it).
    #[serde(default, with = "de::text")]
    pub group: Option<String>,
    /// Event name (`high`, `low`, …).
    #[serde(rename = "eventName", default, with = "de::text")]
    pub event_name: Option<String>,
    /// Whether this is an announcement.
    #[serde(rename = "isAnnouncement", default, with = "de::flag")]
    pub is_announcement: Option<bool>,
    /// The plugin that raised it.
    #[serde(default)]
    pub plugin: Option<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

const fn info() -> Level {
    Level::Info
}

/// An event from the `/alarm` channel.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AlarmEvent {
    /// A warning (`alarm`).
    Alarm(Alarm),
    /// An urgent alarm (`urgent_alarm`).
    Urgent(Alarm),
    /// An announcement.
    Announcement(Alarm),
    /// A lower-level notification.
    Notification(Alarm),
    /// A previous alarm was cleared (`clear_alarm`).
    Cleared(Alarm),
    /// The connection dropped and was re-established; alarms may have been missed.
    Reconnected,
}

impl AlarmEvent {
    pub(crate) fn decode(name: &str, mut args: Vec<Value>) -> Option<Self> {
        let (wrap, level): (fn(Alarm) -> Self, Level) = match name {
            "alarm" => (Self::Alarm, Level::Warn),
            "urgent_alarm" => (Self::Urgent, Level::Urgent),
            "announcement" => (Self::Announcement, Level::Info),
            "notification" => (Self::Notification, Level::Info),
            "clear_alarm" => (Self::Cleared, Level::Info),
            _ => return None,
        };
        let payload = if args.is_empty() {
            Value::Null
        } else {
            args.swap_remove(0)
        };
        // Never drop an alarm over its payload: fall back to the event's own severity.
        let has_level = payload.get("level").is_some();
        let mut alarm = serde_json::from_value::<Alarm>(payload.clone())
            .unwrap_or_else(|_| Alarm::bare(level, payload));
        if !has_level {
            alarm.level = level;
        }
        Some(wrap(alarm))
    }
}

impl Alarm {
    /// An alarm built from whatever a payload that does not decode still offers.
    fn bare(level: Level, payload: Value) -> Self {
        let text = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_owned);
        Self {
            level,
            title: text("title"),
            message: text("message"),
            group: text("group"),
            event_name: text("eventName"),
            is_announcement: None,
            plugin: None,
            extra: match payload {
                Value::Object(map) => map,
                _ => Extra::new(),
            },
        }
    }
}

/// A `dataUpdate` from the root namespace: the full recent data on connect, deltas after.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DataUpdate {
    /// `true` for deltas, absent/`false` for the initial full payload.
    #[serde(default, with = "de::flag")]
    pub delta: Option<bool>,
    /// Server time of the update.
    #[serde(rename = "lastUpdated", default, with = "time::millis")]
    pub last_updated: Option<Timestamp>,
    /// Sensor glucose values.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub sgvs: Vec<PropertySgv>,
    /// Meter glucose values.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub mbgs: Vec<PropertySgv>,
    /// Calibrations.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub cals: Vec<Value>,
    /// Treatments (deltas may carry `action: "update" | "remove"` in `extra`).
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub treatments: Vec<Treatment>,
    /// Device status reports.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub devicestatus: Vec<DeviceStatus>,
    /// Profile documents.
    #[serde(default, deserialize_with = "tolerant_vec")]
    pub profiles: Vec<ProfileStore>,
    /// Food.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub food: Vec<Value>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl DataUpdate {
    /// Whether this is a delta rather than the initial full payload.
    #[must_use]
    pub fn is_delta(&self) -> bool {
        self.delta == Some(true)
    }
}

/// An event from the root namespace.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum UpdateEvent {
    /// Data changed.
    Data(Box<DataUpdate>),
    /// The connection dropped and was re-established (a full payload follows).
    Reconnected,
}

impl UpdateEvent {
    pub(crate) fn decode(name: &str, mut args: Vec<Value>) -> Option<Self> {
        if name != "dataUpdate" || args.is_empty() {
            return None;
        }
        serde_json::from_value(args.swap_remove(0))
            .ok()
            .map(|u| Self::Data(Box::new(u)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn storage_events() {
        let created = StorageEvent::decode(
            "create",
            vec![json!({"colName": "treatments", "doc": {"identifier": "x", "carbs": 5}})],
        );
        assert!(
            matches!(created, Some(StorageEvent::Created(ref d)) if d.identifier() == Some("x"))
        );
        let deleted = StorageEvent::decode(
            "delete",
            vec![json!({"colName": "devicestatus", "identifier": "y"})],
        );
        assert_eq!(
            deleted,
            Some(StorageEvent::Deleted {
                collection: CollectionName::DeviceStatus,
                identifier: "y".into()
            })
        );
    }

    #[test]
    fn alarm_events() {
        let urgent = AlarmEvent::decode(
            "urgent_alarm",
            vec![
                json!({"level": 2, "title": "Urgent LOW", "message": "BG 39", "group": "default"}),
            ],
        );
        assert!(matches!(urgent, Some(AlarmEvent::Urgent(ref a)) if a.level == Level::Urgent));
    }

    #[test]
    fn alarms_are_never_dropped_over_their_payload() {
        let warn = AlarmEvent::decode("alarm", vec![json!({"level": "1", "title": "High"})]);
        assert!(matches!(warn, Some(AlarmEvent::Alarm(ref a)) if a.level == Level::Warn));

        let unreadable = AlarmEvent::decode(
            "urgent_alarm",
            vec![json!({"level": "loud", "title": "Urgent LOW"})],
        );
        assert!(matches!(
            unreadable,
            Some(AlarmEvent::Urgent(ref a))
                if a.level == Level::Urgent && a.title.as_deref() == Some("Urgent LOW")
        ));

        let empty = AlarmEvent::decode("urgent_alarm", Vec::new());
        assert!(matches!(empty, Some(AlarmEvent::Urgent(ref a)) if a.level == Level::Urgent));
    }
}
