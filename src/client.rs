use super::structs;
use sha1::{Digest, Sha1};
use structs::endpoints::Endpoint;
use structs::treatments::{IobData, IobWrapper};

use anyhow::Result;
use reqwest::{Client as HttpClient};
use url::Url;

#[derive(Clone)]
pub struct NightscoutClient {
    pub base_url: Url,
    pub api_secret: Option<String>,
    pub http: HttpClient,
    pub api_secret_hash: Option<String>
}

impl NightscoutClient {
    pub fn new(base_url: &str, api_secret: Option<String>) -> Result<Self> {
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
            api_secret_hash: hash
        })
    }

    pub fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(hash) = &self.api_secret_hash {
            request.header("api-secret", hash)
        } else {
            request
        }
    }

    pub async fn iob(&self) -> reqwest::Result<IobData> {
        let url = self
            .base_url
            .join(Endpoint::Iob.as_path())
            .expect("Error building the URL");

        let mut request = self.http.get(url);

        request = self.auth(request);

        let res = request.send().await?;
        let wrapper = res.json::<IobWrapper>().await?;

        Ok(wrapper.iob)
    }
}
