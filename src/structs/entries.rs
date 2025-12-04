use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::structs::trends::Trend;

/// SGV (Sensor Glucose Value)
/// This struct represents blood glucose values automatically entered by a CGM (continuous glucose monitor)
#[derive(Debug, Serialize, Deserialize)]
pub struct SgvEntry {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub sgv: i64,
    pub date: i64,
    #[serde(rename = "dateString")]
    pub date_string: String,
    pub direction: Trend,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl SgvEntry {
    pub fn new(sgv: i64, direction: Trend, date: DateTime<Utc>) -> Self {
        SgvEntry {
            id: None,
            sgv,
            date: date.timestamp_millis(),
            date_string: date.to_rfc3339(),
            direction,
            type_: "sgv".to_string(),
            device: Some("cinnamon".to_string()),
        }
    }

    pub fn device(mut self, name: String) -> Self {
        self.device = Some(name);
        self
    }
}

/// MBG (Meter Blood Glucose)
/// This struct represents blood glucose data manually entered by the user, often obtained via a fingerprick.
/// https://en.wikipedia.org/wiki/Fingerstick
#[derive(Debug, Serialize, Deserialize)]
pub struct MbgEntry {
    #[serde(rename = "_id")]
    pub id: String,
    pub mbg: u16,
    pub date: u64,
    #[serde(rename = "dateString")]
    pub date_string: String,
    #[serde(rename = "type")]
    pub type_: String,
}
