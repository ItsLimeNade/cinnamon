use crate::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;
use crate::query_builder::{HasDevice, QueryBuilder};

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct DeviceStatusService {
    pub client: NightscoutClient,
}

impl DeviceStatusService {
    pub fn get(&self) -> QueryBuilder<DeviceStatus> {
        QueryBuilder::<DeviceStatus>::new(self.client.clone(), Endpoint::DeviceStatus, Method::GET)
    }

    pub fn delete(&self) -> QueryBuilder<DeviceStatus> {
        QueryBuilder::<DeviceStatus>::new(
            self.client.clone(),
            Endpoint::DeviceStatus,
            Method::DELETE,
        )
    }

    pub async fn create(
        &self,
        entries: Vec<DeviceStatus>,
    ) -> Result<Vec<DeviceStatus>, NightscoutError> {
        let url = self
            .client
            .base_url
            .join(Endpoint::DeviceStatus.as_path())?;
        let mut request = self.client.http.post(url);
        request = self.client.auth(request);
        let response = self.client.send_checked(request.json(&entries)).await?;
        Ok(response.json::<Vec<DeviceStatus>>().await?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceStatus {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,

    #[serde(rename = "created_at")]
    pub created_at: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pump: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub openaps: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploader: Option<Value>,

    #[serde(flatten)]
    pub extra: Value,
}

impl HasDevice for DeviceStatus {
    fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }
}
