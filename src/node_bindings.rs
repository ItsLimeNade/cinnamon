use crate::client::NightscoutClient;
use crate::models::devicestatus::DeviceStatus;
use crate::models::entries::{MbgEntry, SgvEntry};
use crate::models::profile::{ProfileService, ProfileSet};
use crate::models::properties::{Properties, PropertiesService, PropertyType};
use crate::models::status::Status;
use crate::models::treatments::Treatment;
use crate::query_builder::Device;

use chrono::{DateTime, Utc};
use napi::bindgen_prelude::*;
use napi_derive::napi;

// -----------------------------
// Client

#[napi]
pub struct Cinnamon {
    client: NightscoutClient,
}

#[napi(string_enum)]
pub enum DeviceType {
    Auto,
    All,
    Custom,
}

#[napi]
impl Cinnamon {
    #[napi(constructor)]
    pub fn new(url: String, api_secret: Option<String>) -> Result<Self> {
        let client = NightscoutClient::new(&url, api_secret)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Cinnamon { client })
    }

    #[napi]
    pub fn sgv(&self) -> JsSgvQuery {
        JsSgvQuery {
            client: self.client.clone(),
            limit: 10,
            from: None,
            to: None,
            device: Device::All,
        }
    }

    #[napi]
    pub fn mbg(&self) -> JsMbgQuery {
        JsMbgQuery {
            client: self.client.clone(),
            limit: 10,
            from: None,
            to: None,
            device: Device::All,
        }
    }

    #[napi]
    pub fn treatments(&self) -> JsTreatmentQuery {
        JsTreatmentQuery {
            client: self.client.clone(),
            limit: 10,
            from: None,
            to: None,
            device: Device::All,
        }
    }

    #[napi]
    pub fn device_status(&self) -> JsDeviceStatusQuery {
        JsDeviceStatusQuery {
            client: self.client.clone(),
            limit: 10,
            from: None,
            to: None,
            device: Device::All,
        }
    }

    #[napi]
    pub fn profile(&self) -> JsProfileQuery {
        JsProfileQuery {
            client: self.client.clone(),
        }
    }

    #[napi]
    pub fn properties(&self) -> JsPropertiesQuery {
        JsPropertiesQuery {
            client: self.client.clone(),
            enabled: vec![],
            at: None,
        }
    }

    #[napi]
    pub fn status(&self) -> JsStatusQuery {
        JsStatusQuery {
            client: self.client.clone(),
        }
    }
}

// -----------------------------
// Status

#[napi]
pub struct JsStatusQuery {
    client: NightscoutClient,
}

#[napi]
impl JsStatusQuery {
    #[napi]
    pub async fn fetch(&self) -> Result<Status> {
        self.client
            .status()
            .get()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}

// -----------------------------
// SGV

#[napi]
pub struct JsSgvQuery {
    client: NightscoutClient,
    limit: usize,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    device: Device,
}

#[napi]
impl JsSgvQuery {
    #[napi]
    pub fn limit(&mut self, count: u32) -> &Self {
        self.limit = count as usize;
        self
    }

    #[napi]
    pub fn from_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.from = Some(date);
        self
    }

    #[napi]
    pub fn to_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.to = Some(date);
        self
    }

    #[napi]
    pub fn filter_device(&mut self, mode: DeviceType, name: Option<String>) -> &Self {
        self.device = match mode {
            DeviceType::Auto => Device::Auto,
            DeviceType::Custom => Device::Custom(name.unwrap_or_default()),
            DeviceType::All => Device::All,
        };
        self
    }

    #[napi]
    pub async fn fetch(&self) -> Result<Vec<SgvEntry>> {
        let mut builder = self.client.entries().sgv().list();
        builder = builder.limit(self.limit);
        builder = builder.device(self.device.clone());
        if let Some(f) = self.from {
            builder = builder.from(f);
        }
        if let Some(t) = self.to {
            builder = builder.to(t);
        }

        builder.await.map_err(|e| Error::from_reason(e.to_string()))
    }
}

// -----------------------------
// MBG

#[napi]
pub struct JsMbgQuery {
    client: NightscoutClient,
    limit: usize,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    device: Device,
}

#[napi]
impl JsMbgQuery {
    #[napi]
    pub fn limit(&mut self, count: u32) -> &Self {
        self.limit = count as usize;
        self
    }

    #[napi]
    pub fn from_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.from = Some(date);
        self
    }

    #[napi]
    pub fn to_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.to = Some(date);
        self
    }

    #[napi]
    pub fn filter_device(&mut self, mode: DeviceType, name: Option<String>) -> &Self {
        self.device = match mode {
            DeviceType::Auto => Device::Auto,
            DeviceType::Custom => Device::Custom(name.unwrap_or_default()),
            DeviceType::All => Device::All,
        };
        self
    }

    #[napi]
    pub async fn fetch(&self) -> Result<Vec<MbgEntry>> {
        let mut builder = self.client.entries().mbg().list();
        builder = builder.limit(self.limit);
        builder = builder.device(self.device.clone());
        if let Some(f) = self.from {
            builder = builder.from(f);
        }
        if let Some(t) = self.to {
            builder = builder.to(t);
        }

        builder.await.map_err(|e| Error::from_reason(e.to_string()))
    }
}

// -----------------------------
// Treatments

#[napi]
pub struct JsTreatmentQuery {
    client: NightscoutClient,
    limit: usize,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    device: Device,
}

#[napi]
impl JsTreatmentQuery {
    #[napi]
    pub fn limit(&mut self, count: u32) -> &Self {
        self.limit = count as usize;
        self
    }

    #[napi]
    pub fn from_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.from = Some(date);
        self
    }

    #[napi]
    pub fn to_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.to = Some(date);
        self
    }

    #[napi]
    pub fn filter_device(&mut self, mode: DeviceType, name: Option<String>) -> &Self {
        self.device = match mode {
            DeviceType::Auto => Device::Auto,
            DeviceType::Custom => Device::Custom(name.unwrap_or_default()),
            DeviceType::All => Device::All,
        };
        self
    }

    #[napi]
    pub async fn fetch(&self) -> Result<Vec<Treatment>> {
        let mut builder = self.client.treatments().list();
        builder = builder.limit(self.limit);
        builder = builder.device(self.device.clone());
        if let Some(f) = self.from {
            builder = builder.from(f);
        }
        if let Some(t) = self.to {
            builder = builder.to(t);
        }

        builder.await.map_err(|e| Error::from_reason(e.to_string()))
    }
}

// -----------------------------
// DeviceStatus

#[napi]
pub struct JsDeviceStatusQuery {
    client: NightscoutClient,
    limit: usize,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    device: Device,
}

#[napi]
impl JsDeviceStatusQuery {
    #[napi]
    pub fn limit(&mut self, count: u32) -> &Self {
        self.limit = count as usize;
        self
    }

    #[napi]
    pub fn from_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.from = Some(date);
        self
    }

    #[napi]
    pub fn to_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.to = Some(date);
        self
    }

    #[napi]
    pub fn filter_device(&mut self, mode: DeviceType, name: Option<String>) -> &Self {
        self.device = match mode {
            DeviceType::Auto => Device::Auto,
            DeviceType::Custom => Device::Custom(name.unwrap_or_default()),
            DeviceType::All => Device::All,
        };
        self
    }

    #[napi]
    pub async fn fetch(&self) -> Result<Vec<DeviceStatus>> {
        let mut builder = self.client.devicestatus().list();
        builder = builder.limit(self.limit);
        builder = builder.device(self.device.clone());
        if let Some(f) = self.from {
            builder = builder.from(f);
        }
        if let Some(t) = self.to {
            builder = builder.to(t);
        }

        builder.await.map_err(|e| Error::from_reason(e.to_string()))
    }
}

// -----------------------------
// Profile

#[napi]
pub struct JsProfileQuery {
    client: NightscoutClient,
}

#[napi]
impl JsProfileQuery {
    #[napi]
    pub async fn fetch(&self) -> Result<Vec<ProfileSet>> {
        let service = ProfileService {
            client: self.client.clone(),
        };

        service
            .get()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}

// -----------------------------
// Properties

#[napi]
pub struct JsPropertiesQuery {
    client: NightscoutClient,
    enabled: Vec<String>,
    at: Option<DateTime<Utc>>,
}

#[napi]
impl JsPropertiesQuery {
    #[napi]
    pub fn enable(&mut self, properties: Vec<String>) -> &Self {
        self.enabled = properties;
        self
    }

    #[napi]
    pub fn at_date(&mut self, date: DateTime<Utc>) -> &Self {
        self.at = Some(date);
        self
    }

    #[napi]
    pub async fn fetch(&self) -> Result<Properties> {
        let service = PropertiesService {
            client: self.client.clone(),
        };
        let mut request = service.get();

        if let Some(date) = self.at {
            request = request.at(date);
        }

        if !self.enabled.is_empty() {
            let props: Vec<PropertyType> = self
                .enabled
                .iter()
                .map(|s| match s.to_lowercase().as_str() {
                    "iob" => PropertyType::Iob,
                    "cob" => PropertyType::Cob,
                    "pump" => PropertyType::Pump,
                    "basal" => PropertyType::Basal,
                    "profile" => PropertyType::Profile,
                    "bage" => PropertyType::Bage,
                    "cage" => PropertyType::Cage,
                    "iage" => PropertyType::Iage,
                    "sage" => PropertyType::Sage,
                    "upbat" => PropertyType::Upbat,
                    "rawbg" => PropertyType::Rawbg,
                    "delta" => PropertyType::Delta,
                    "direction" => PropertyType::Direction,
                    "ar2" => PropertyType::Ar2,
                    "devicestatus" => PropertyType::Devicestatus,
                    "openaps" => PropertyType::Openaps,
                    "loop" => PropertyType::Loop,
                    "bgnow" => PropertyType::BgNow,
                    "buckets" => PropertyType::Buckets,
                    "dbsize" => PropertyType::DbSize,
                    "runtimestate" => PropertyType::RuntimeState,
                    _ => PropertyType::Custom(s.to_string()),
                })
                .collect();
            request = request.only(&props);
        }

        request
            .send()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}
