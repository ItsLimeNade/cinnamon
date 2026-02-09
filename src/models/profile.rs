use crate::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct ProfileService {
    pub client: NightscoutClient,
}

impl ProfileService {
    /// Retrieves the Nightscout profile data.
    ///
    /// This is a "Direct Fetch" method. It does not use a query builder; calling this
    /// method immediately initiates the HTTP request.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cinnamon::client::NightscoutClient;
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NightscoutClient::new("https://ns.example.com")?;
    /// let status = client.profiles().get().await?;
    /// println!("Nightscout Name: {}", status.default_profile_name);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get(&self) -> Result<Vec<ProfileSet>, NightscoutError> {
        let url = self.client.base_url.join(Endpoint::Profile.as_path())?;
        self.client.fetch::<Vec<ProfileSet>>(url).await
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]

pub struct ProfileSet {
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(rename = "defaultProfile")]
    pub default_profile_name: String,

    #[serde(rename = "startDate")]
    pub start_date: String,

    pub store: HashMap<String, ProfileConfig>,

    #[serde(rename = "mills")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mills: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileConfig {
    pub dia: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carbs_hr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
    pub timezone: String,
    pub units: String,
    pub carbratio: Vec<TimeSchedule>,
    pub sens: Vec<TimeSchedule>,
    pub basal: Vec<TimeSchedule>,
    pub target_low: Vec<TimeSchedule>,
    pub target_high: Vec<TimeSchedule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimeSchedule {
    pub time: String,
    pub value: f64,
    #[serde(rename = "timeAsSeconds")]
    pub time_as_seconds: Option<i64>,
}
