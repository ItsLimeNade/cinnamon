use crate::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;
use crate::models::treatments::Treatment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyType {
    Iob,
    Cob,
    Pump,
    Basal,
    Profile,
    Bage,
    Cage,
    Iage,
    Sage,
    Upbat,
    Rawbg,
    Delta,
    Direction,
    Ar2,
    Devicestatus,
    Openaps,
    Loop,
    BgNow,
    Buckets,
    DbSize,
    RuntimeState,
    Custom(String),
}

impl fmt::Display for PropertyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PropertyType::Iob => "iob",
            PropertyType::Cob => "cob",
            PropertyType::Pump => "pump",
            PropertyType::Basal => "basal",
            PropertyType::Profile => "profile",
            PropertyType::Bage => "bage",
            PropertyType::Cage => "cage",
            PropertyType::Iage => "iage",
            PropertyType::Sage => "sage",
            PropertyType::Upbat => "upbat",
            PropertyType::Rawbg => "rawbg",
            PropertyType::Delta => "delta",
            PropertyType::Direction => "direction",
            PropertyType::Ar2 => "ar2",
            PropertyType::Devicestatus => "devicestatus",
            PropertyType::Openaps => "openaps",
            PropertyType::Loop => "loop",
            PropertyType::BgNow => "bgnow",
            PropertyType::Buckets => "buckets",
            PropertyType::DbSize => "dbsize",
            PropertyType::RuntimeState => "runtimestate",
            PropertyType::Custom(s) => s.as_str(),
        };
        write!(f, "{}", s)
    }
}

/// The main response object for /api/v2/properties
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Properties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bgnow: Option<BgNow>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub buckets: Option<Vec<Bucket>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Delta>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub iob: Option<IobProperty>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cob: Option<Cob>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub basal: Option<Basal>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub upbat: Option<Upbat>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dbsize: Option<DbSize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtimestate: Option<RuntimeState>,

    /// Captures any other fields (like "pump" or custom plugins) generically
    #[serde(flatten)]
    pub unknown: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BgNow {
    pub mean: f64,
    pub last: f64,
    pub mills: i64,
    pub sgvs: Vec<PropertySgv>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Delta {
    pub absolute: f64,
    #[serde(rename = "elapsedMins")]
    pub elapsed_mins: f64,
    pub interpolated: bool,
    #[serde(rename = "mean5MinsAgo")]
    pub mean_5_mins_ago: f64,
    pub mgdl: f64,
    pub scaled: f64,
    pub display: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bucket {
    pub mean: f64,
    pub last: f64,
    pub mills: i64,
    pub index: i32,
    #[serde(rename = "fromMills")]
    pub from_mills: i64,
    #[serde(rename = "toMills")]
    pub to_mills: i64,
    pub sgvs: Vec<PropertySgv>,
}

/// A simplified SGV used inside properties (slightly different from main Entries)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PropertySgv {
    #[serde(rename = "_id")]
    pub id: String,
    pub mgdl: f64,
    pub mills: i64,
    pub device: String,
    pub direction: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub scaled: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Direction {
    pub display: Option<String>,
    pub value: String,
    pub label: String,
    pub entity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Upbat {
    pub display: String,
    // devices is sometimes a Map, sometimes empty. Value is safest.
    pub devices: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IobProperty {
    pub iob: f64,
    pub activity: f64,
    pub source: String,
    pub display: String,
    #[serde(rename = "displayLine")]
    pub display_line: String,
    #[serde(rename = "lastBolus", skip_serializing_if = "Option::is_none")]
    pub last_bolus: Option<Treatment>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cob {
    pub cob: f64,
    #[serde(rename = "isDecaying")]
    pub is_decaying: i32,
    #[serde(rename = "decayedBy")]
    pub decayed_by: String,
    pub source: String,
    pub display: Value,
    #[serde(rename = "displayLine")]
    pub display_line: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Basal {
    pub display: String,
    pub current: Option<BasalCurrent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BasalCurrent {
    pub basal: f64,
    #[serde(rename = "tempbasal")]
    pub temp_basal: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DbSize {
    pub display: String,
    pub status: String,
    #[serde(rename = "totalDataSize")]
    pub total_data_size: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeState {
    pub state: String,
}

pub struct PropertiesService {
    pub client: NightscoutClient,
}

impl PropertiesService {
    /// Begins building a request for Nightscout system properties.
    ///
    /// System properties include Insulin on Board (IOB), Carbs on Board (COB), Pump Status, etc.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use cinnamon::client::NightscoutClient;
    /// # use cinnamon::models::properties::PropertyType;
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NightscoutClient::new("https://ns.example.com")?;
    ///
    /// // Fetch only IOB and COB
    /// let props = client.properties()
    ///     .get()
    ///     .only(&[PropertyType::Iob, PropertyType::Cob])
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self) -> PropertiesRequest {
        PropertiesRequest::new(self.client.clone())
    }
}

/// A builder for constructing a properties request.
pub struct PropertiesRequest {
    client: NightscoutClient,
    requested_properties: Vec<PropertyType>,
    at_time: Option<DateTime<Utc>>,
}

impl PropertiesRequest {
    pub fn new(client: NightscoutClient) -> Self {
        Self {
            client,
            requested_properties: Vec::new(),
            at_time: None,
        }
    }

    /// Specifies which properties to retrieve.
    ///
    /// By default, Nightscout returns all available system properties. Using this method
    /// allows you to filter the response to specific fields (e.g., IOB, COB), which reduces
    /// payload size and processing time.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use cinnamon::client::NightscoutClient;
    /// # use cinnamon::models::properties::PropertyType;
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NightscoutClient::new("https://ns.example.com")?;
    ///
    /// let properties = client.properties()
    ///     .get()
    ///     // Request only Insulin on Board and Carbs on Board
    ///     .only(&[PropertyType::Iob, PropertyType::Cob])
    ///     .send()
    ///     .await?;
    ///
    /// if let Some(iob) = properties.iob {
    ///     println!("IOB: {}", iob.iob);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn only(mut self, properties: &[PropertyType]) -> Self {
        self.requested_properties.extend_from_slice(properties);
        self
    }

    /// Requests the system state as it was at a specific time.
    ///
    /// If omitted, the current system state is returned.
    pub fn at(mut self, time: DateTime<Utc>) -> Self {
        self.at_time = Some(time);
        self
    }

    /// Executes the request.
    pub async fn send(self) -> Result<Properties, NightscoutError> {
        let base_path = Endpoint::Properties.as_path();

        let path = if self.requested_properties.is_empty() {
            format!("{}.json", base_path)
        } else {
            let joined = self
                .requested_properties
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<String>>()
                .join(",");
            format!("{}/{}", base_path, joined)
        };

        let mut url = self.client.base_url.join(&path)?;

        if let Some(time) = self.at_time {
            url.query_pairs_mut()
                .append_pair("time", &time.to_rfc3339());
        }

        let data = self.client.fetch::<Properties>(url).await?;
        Ok(data)
    }
}
