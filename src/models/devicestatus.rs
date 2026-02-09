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
    /// Initiates a query for Device Status entries.
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
    /// let entries = client.devicestatus()
    ///     .get()
    ///     .limit(10)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self) -> QueryBuilder<DeviceStatus> {
        QueryBuilder::<DeviceStatus>::new(self.client.clone(), Endpoint::DeviceStatus, Method::GET)
    }

    /// Initiates a delete request for Device Status entries.
    ///
    /// Use the builder to specify which entries to delete (e.g. by ID or date range).
    pub fn delete(&self) -> QueryBuilder<DeviceStatus> {
        QueryBuilder::<DeviceStatus>::new(
            self.client.clone(),
            Endpoint::DeviceStatus,
            Method::DELETE,
        )
    }

    /// Uploads new Device Status entries to Nightscout.
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
