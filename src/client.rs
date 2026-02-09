use super::error::NightscoutError;

use reqwest::{Client as HttpClient, Response};
use sha1::{Digest, Sha1};
use url::Url;

use crate::models::devicestatus::DeviceStatusService;
use crate::models::entries::{MbgService, SgvService};
use crate::models::profile::ProfileService;
use crate::models::properties::PropertiesService;
use crate::models::status::StatusService;
use crate::models::treatments::TreatmentsService;

use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct NightscoutClient {
    pub inner: Arc<NightscoutClientInner>,
}

#[derive(Clone)]
pub struct NightscoutClientInner {
    /// The base URL of the Nightscout instance.
    pub base_url: Url,
    /// The internal HTTP client used for requests.
    pub http: HttpClient,
    /// The SHA1 hash of the API secret, used for authentication headers.
    pub api_secret_hash: Option<String>,
}

impl Deref for NightscoutClient {
    type Target = NightscoutClientInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl NightscoutClient {
    /// Creates a new `NightscoutClient` without an API secret.
    ///
    /// This client will only be able to access public endpoints. To perform write operations
    /// or access protected data, chain `with_secret`.
    ///
    /// ## Arguments
    ///
    /// * `base_url` - The full URL to the Nightscout instance (e.g., `https://my-site.herokuapp.com`).
    ///
    /// ## Errors
    ///
    /// Returns a `NightscoutError` if the URL is invalid.
    pub fn new(base_url: &str) -> Result<Self, NightscoutError> {
        let inner = NightscoutClientInner {
            base_url:Url::parse(base_url)?,
            http: HttpClient::new(),
            api_secret_hash: None, 
        };
        let client = Self { inner: Arc::new(inner) };
        Ok(client)
    }

    /// Appends an API secret to the client for authentication.
    ///
    /// The secret is automatically hashed using SHA1 as required by Nightscout headers.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use cinnamon::client::NightscoutClient;
    /// let client = NightscoutClient::new("https://example.com").unwrap()
    ///     .with_secret("my-password-123");
    /// ```
    pub fn with_secret(self, api_secret: impl Into<String>) -> Self {
        let secret = api_secret.into();
        
        let mut hasher = Sha1::new();
        hasher.update(secret.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let inner = NightscoutClientInner {
            base_url: self.base_url.clone(),
            http: self.http.clone(),
            api_secret_hash: Some(hash), 
        };
        let client = Self { inner: Arc::new(inner) };
        client
    }

    /// Adds authentication headers to a request if a secret is present.
    pub fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(hash) = &self.api_secret_hash {
            request.header("api-secret", hash)
        } else {
            request
        }
    }

    /// Access the Treatments service for managing care events (boluses, carbs, etc.).
    pub fn treatments(&self) -> TreatmentsService {
        TreatmentsService {
            client: self.clone(),
        }
    }

    /// Access the Sensor Glucose Value (SGV) service.
    pub fn sgv(&self) -> SgvService {
        SgvService {
            client: self.clone(),
        }
    }

    /// Access the Meter Blood Glucose (MBG) service.
    pub fn mbg(&self) -> MbgService {
        MbgService {
            client: self.clone(),
        }
    }

    /// Access the Properties service for system status (IOB, COB, Pump).
    pub fn properties(&self) -> PropertiesService {
        PropertiesService {
            client: self.clone(),
        }
    }

    /// Access the Device Status service for uploader and pump status updates.
    pub fn devicestatus(&self) -> DeviceStatusService {
        DeviceStatusService {
            client: self.clone(),
        }
    }

    /// Access the Profile service for basal rates, ISF, and carb ratios.
    pub fn profiles(&self) -> ProfileService {
        ProfileService {
            client: self.clone(),
        }
    }

    /// Access the server status service (version, settings, capabilities).
    pub fn status(&self) -> StatusService {
        StatusService {
            client: self.clone(),
        }
    }

    /// Sends a request and checks the response status.
    ///
    /// Returns `NightscoutError::AuthError` if the server returns 401 Unauthorized,
    /// or `NightscoutError::ApiError` for other non-success codes.
    pub(crate) async fn send_checked(
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

    /// Helper to fetch and deserialize a JSON response from a URL.
    pub(crate) async fn fetch<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T, NightscoutError> {
        let req = self.auth(self.http.get(url));
        let res  = self.send_checked(req).await?;
        let data = res.json::<T>().await?;
        Ok(data)
    }
}
