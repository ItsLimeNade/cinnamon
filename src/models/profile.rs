use crate::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;
use napi_derive::napi;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct ProfileService {
    pub client: NightscoutClient,
}

impl ProfileService {
    pub async fn get(&self) -> Result<Vec<ProfileSet>, NightscoutError> {
        let url = self.client.base_url.join(Endpoint::Profile.as_path())?;

        let mut request = self.client.http.get(url);
        request = self.client.auth(request);

        let response = self.client.send_checked(request).await?;

        Ok(response.json::<Vec<ProfileSet>>().await?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]

pub struct ProfileSet {
    #[serde(rename = "_id")]
    pub id: String,

    #[napi(js_name = "defaultProfile")]
    #[serde(rename = "defaultProfile")]
    pub default_profile_name: String,

    #[napi(js_name = "startDate")]
    #[serde(rename = "startDate")]
    pub start_date: String,

    pub store: HashMap<String, ProfileConfig>,

    #[serde(rename = "mills")]
    pub mills: Option<i64>,

    pub units: Option<String>,

    #[napi(js_name = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[napi(object)]
pub struct ProfileConfig {
    pub dia: f64,
    pub carbs_hr: Option<f64>,
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
#[napi(object)]
pub struct TimeSchedule {
    pub time: String,
    pub value: f64,
    #[serde(rename = "timeAsSeconds")]
    pub time_as_seconds: Option<i64>,
}
