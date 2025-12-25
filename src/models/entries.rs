use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use crate::client::NightscoutClient;
use crate::structs::trends::Trend;
use crate::query_builder::QueryBuilder;
use crate::structs::endpoints::Endpoint;
use sha1::{Digest, Sha1};

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
    pub async fn latest(&self) -> reqwest::Result<SgvEntry> {
        let url = self
            .client
            .base_url
            .join(Endpoint::Current.as_path())
            .expect("Error building the URL");

        let mut request = self.client.http.get(url);

        if let Some(secret) = &self.client.api_secret {
            request = request.header("api-secret", secret);
        }

        let res = request.send().await?;

        let resp = res.json::<Vec<SgvEntry>>().await?;
        let data = resp.first().expect("No data was found");
        Ok(data.clone())
    }

    pub async fn create(&self, entries: Vec<SgvEntry>) -> reqwest::Result<Vec<SgvEntry>> {
        let url = self
            .client
            .base_url
            .join(Endpoint::Entries.as_path())
            .expect("URL Error");

        let mut request = self.client.http.post(url);

        if let Some(secret) = &self.client.api_secret {
            let mut hasher = Sha1::new();
            hasher.update(secret.as_bytes());

            let result = hasher.finalize();
            request = request.header("api-secret", format!("{:x}", result));
        }

        let response = request.json(&entries).send().await?;

        response.json::<Vec<SgvEntry>>().await
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
