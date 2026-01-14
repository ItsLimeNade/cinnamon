use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use crate::client::NightscoutClient;
use crate::error::NightscoutError;
use crate::structs::trends::Trend;
use crate::query_builder::QueryBuilder;
use crate::structs::endpoints::Endpoint;

pub struct EntriesService {
    pub client: NightscoutClient
}

pub struct SgvService {
    pub client: NightscoutClient
}

impl EntriesService {
    pub fn sgv(&self) -> SgvService {
        SgvService { client: self.client.clone() }
    }
}

impl SgvService {
    /// Returns a query builder used to create your request
    /// 
    /// # Examples
    /// 
    /// ```
    /// use cinnamon::client::NightscoutClient;
    /// 
    /// let URL = "https://www.example_url.com/";
    /// let SECRET = "SecretPasss";
    /// 
    /// let client = NightscoutClient::new(URL, SECRET);
    /// let entries: Vec<SgvEntry> = client.entries().sgv()
    ///                 .list()
    ///                 .from(Utc::now() - Duration::hours(24))
    ///                 .to(Utc::now() - Duration::hours(20)) 
    ///                 .limit(10)
    ///                 .await?;
    pub fn list(&self) -> QueryBuilder<SgvEntry> {
        QueryBuilder::<SgvEntry>::new(self.client.clone(), Endpoint::Sgv, Method::GET)
    }

    pub fn delete(&self) -> QueryBuilder<SgvEntry> {
        QueryBuilder::<SgvEntry>::new(self.client.clone(), Endpoint::Sgv, Method::DELETE)
    }

    /// Fetches the latest available SGV entry.
    pub async fn latest(&self) -> Result<SgvEntry, NightscoutError> {
        let url = self
            .client
            .base_url
            .join(Endpoint::Current.as_path())?;

        let mut request = self.client.http.get(url);

        request = self.client.auth(request);

        let res = self.client.send_checked(request).await?;

        let resp = res.json::<Vec<SgvEntry>>().await?;
        resp.first()
            .cloned()
            .ok_or(NightscoutError::NotFound)
    }

    pub async fn create(&self, entries: Vec<SgvEntry>) -> Result<Vec<SgvEntry>, NightscoutError> {
        let url = self
            .client
            .base_url
            .join(Endpoint::Entries.as_path())
            .expect("URL Error");

        let mut request = self.client.http.post(url);

        request = self.client.auth(request);

        let response = self.client.send_checked(request.json(&entries)).await?;

        Ok(response.json::<Vec<SgvEntry>>().await?)
    }

}


/// SGV (Sensor Glucose Value)
/// 
/// This struct represents blood glucose values automatically entered by a CGM (continuous glucose monitor)
#[derive(Debug, Serialize, Deserialize, Clone)]
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
/// 
/// This struct represents blood glucose data manually entered by the user, often obtained via a fingerprick.
/// 
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
