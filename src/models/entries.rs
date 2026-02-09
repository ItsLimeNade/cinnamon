use crate::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;
use crate::models::trends::Trend;
use crate::query_builder::{HasDevice, QueryBuilder};

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};

pub struct SgvService {
    pub client: NightscoutClient,
}

pub struct MbgService {
    pub client: NightscoutClient,
}

impl SgvService {
    /// Initiates a query for SGV entries.
    ///
    /// This returns a `QueryBuilder`. You can chain methods like `.limit()`, `.from()`, and `.to()`
    /// before calling `.send()` to execute the request.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use cinnamon::client::NightscoutClient;
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NightscoutClient::new("https://ns.example.com")?;
    /// let entries = client.sgv()
    ///     .get()
    ///     .limit(10)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self) -> QueryBuilder<SgvEntry> {
        QueryBuilder::<SgvEntry>::new(self.client.clone(), Endpoint::Sgv, Method::GET)
    }

    /// Initiates a delete request for SGV entries.
    ///
    /// Use the builder to specify which entries to delete (e.g. by ID or date range).
    pub fn delete(&self) -> QueryBuilder<SgvEntry> {
        QueryBuilder::<SgvEntry>::new(self.client.clone(), Endpoint::Sgv, Method::DELETE)
    }

    /// Fetches the single latest available SGV entry.
    ///
    /// This is a convenience wrapper around `.get().limit(1)`.
    pub async fn latest(&self) -> Result<SgvEntry, NightscoutError> {
        let builder = self.get().limit(1);
        let result = builder.send().await?;

        result.first().cloned().ok_or(NightscoutError::NotFound)
    }

    /// Uploads new SGV entries to Nightscout.
    pub async fn create(&self, entries: Vec<SgvEntry>) -> Result<Vec<SgvEntry>, NightscoutError> {
        let url = self
            .client
            .base_url
            .join(Endpoint::Entries.as_path())?;

        let mut request = self.client.http.post(url);

        request = self.client.auth(request);

        let response = self.client.send_checked(request.json(&entries)).await?;

        Ok(response.json::<Vec<SgvEntry>>().await?)
    }
}

impl MbgService {
    /// Initiates a query for MBG entries.
    ///
    /// This returns a `QueryBuilder`. You can chain methods like `.limit()`, `.from()`, and `.to()`
    /// before calling `.send()` to execute the request.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use cinnamon::client::NightscoutClient;
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NightscoutClient::new("https://ns.example.com")?;
    /// let entries = client.mbg()
    ///     .get()
    ///     .limit(10)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self) -> QueryBuilder<MbgEntry> {
        QueryBuilder::<MbgEntry>::new(self.client.clone(), Endpoint::Mbg, Method::GET)
    }

    /// Initiates a delete request for MBG entries.
    ///
    /// Use the builder to specify which entries to delete (e.g. by ID or date range).
    pub fn delete(&self) -> QueryBuilder<MbgEntry> {
        QueryBuilder::<MbgEntry>::new(self.client.clone(), Endpoint::Mbg, Method::DELETE)
    }
    
    /// Fetches the single latest available MBG entry.
    ///
    /// This is a convenience wrapper around `.get().limit(1)`.
    pub async fn latest(&self) -> Result<MbgEntry, NightscoutError> {
        let builder = self.get().limit(1);
        let result = builder.send().await?;

        result.first().cloned().ok_or(NightscoutError::NotFound)
    }

    /// Uploads new MBG entries to Nightscout.
    pub async fn create(&self, entries: Vec<MbgEntry>) -> Result<Vec<MbgEntry>, NightscoutError> {
        let url = self.client.base_url.join(Endpoint::Entries.as_path())?;

        let mut request = self.client.http.post(url);
        request = self.client.auth(request);

        let response = self.client.send_checked(request.json(&entries)).await?;

        Ok(response.json::<Vec<MbgEntry>>().await?)
    }
}

/// SGV (Sensor Glucose Value)
///
/// This struct represents blood glucose values automatically entered by a CGM (continuous glucose monitor)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SgvEntry {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub sgv: i32,
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
    pub fn new(sgv: i32, direction: Trend, date: DateTime<Utc>) -> Self {
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

impl HasDevice for SgvEntry {
    fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }
}

/// MBG (Meter Blood Glucose)
///
/// This struct represents blood glucose data manually entered by the user, often obtained via a fingerprick.
///
/// https://en.wikipedia.org/wiki/Fingerstick
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MbgEntry {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub mbg: i32,
    pub date: i64,
    #[serde(rename = "dateString")]
    pub date_string: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl MbgEntry {
    pub fn new(mbg: i32, date: DateTime<Utc>) -> Self {
        MbgEntry {
            id: None,
            mbg,
            date: date.timestamp_millis(),
            date_string: date.to_rfc3339(),
            type_: "mbg".to_string(),
            device: Some("cinnamon".to_string()),
        }
    }

    pub fn device(mut self, name: String) -> Self {
        self.device = Some(name);
        self
    }
}

impl HasDevice for MbgEntry {
    fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }
}
