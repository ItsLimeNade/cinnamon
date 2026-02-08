use super::error::NightscoutError;

use anyhow::Result;
use reqwest::{Client as HttpClient, Response};
use sha1::{Digest, Sha1};
use url::Url;

use crate::models::devicestatus::DeviceStatusService;
use crate::models::entries::{MbgService, SgvService};
use crate::models::profile::ProfileService;
use crate::models::properties::PropertiesService;
use crate::models::status::StatusService;
use crate::models::treatments::TreatmentsService;

#[derive(Clone)]
pub struct NightscoutClient {
    pub base_url: Url,
    pub api_secret: Option<String>,
    pub http: HttpClient,
    pub api_secret_hash: Option<String>,
}

impl NightscoutClient {
    pub fn new(base_url: &str, api_secret: Option<String>) -> Result<Self, NightscoutError> {
        let hash = api_secret.clone().map(|secret| {
            let mut hasher = Sha1::new();
            hasher.update(secret.as_bytes());
            let result = hasher.finalize();
            format!("{:x}", result)
        });

        Ok(Self {
            base_url: Url::parse(base_url)?,
            http: HttpClient::new(),
            api_secret,
            api_secret_hash: hash,
        })
    }

    pub fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(hash) = &self.api_secret_hash {
            request.header("api-secret", hash)
        } else {
            request
        }
    }

    pub fn treatments(&self) -> TreatmentsService {
        TreatmentsService {
            client: self.clone(),
        }
    }

    pub fn sgv(&self) -> SgvService {
        SgvService {
            client: self.clone(),
        }
    }

    pub fn mbg(&self) -> MbgService {
        MbgService {
            client: self.clone(),
        }
    }

    pub fn properties(&self) -> PropertiesService {
        PropertiesService {
            client: self.clone(),
        }
    }

    pub fn devicestatus(&self) -> DeviceStatusService {
        DeviceStatusService {
            client: self.clone(),
        }
    }

    pub fn profiles(&self) -> ProfileService {
        ProfileService {
            client: self.clone(),
        }
    }

    pub fn status(&self) -> StatusService {
        StatusService {
            client: self.clone(),
        }
    }

    pub async fn send_checked(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<Response, NightscoutError> {
        let response = request.send().await?;

        if response.status().is_success() {
            Ok(response)
        } else {
            let status = response.status();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown API error".to_string());

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(NightscoutError::AuthError);
            }

            Err(NightscoutError::ApiError { status, message })
        }
    }

    pub async fn fetch<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T, NightscoutError> {
        let req = self.auth(self.http.get(url));
        let res  = self.send_checked(req).await?;
        let data = res.json::<T>().await?;
        Ok(data)
    }
}
