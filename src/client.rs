use super::query_builder::QueryBuilder;
use super::structs;
use sha1::{Digest, Sha1};
use structs::endpoints::Endpoint;
use structs::entries::{MbgEntry, SgvEntry};
use structs::treatments::{IobData, IobWrapper};

use anyhow::Result;
use reqwest::Client as HttpClient;
use url::Url;

#[derive(Clone)]
pub struct NightscoutClient {
    pub base_url: Url,
    pub api_secret: Option<String>,
    pub http: HttpClient,
}

impl NightscoutClient {
    pub fn new(base_url: &str, api_secret: Option<String>) -> Result<Self> {
        Ok(Self {
            base_url: Url::parse(base_url)?,
            http: HttpClient::new(),
            api_secret,
        })
    }

    pub async fn upload_sgv(&self, entries: Vec<SgvEntry>) -> reqwest::Result<Vec<SgvEntry>> {
        let url = self
            .base_url
            .join(Endpoint::Entries.as_path())
            .expect("URL Error");

        let mut request = self.http.post(url);

        if let Some(secret) = &self.api_secret {
            let mut hasher = Sha1::new();
            hasher.update(secret.as_bytes());

            let result = hasher.finalize();
            request = request.header("api-secret", format!("{:x}", result));
        }

        let response = request.json(&entries).send().await?;

        response.json::<Vec<SgvEntry>>().await
    }

    pub fn get<T>(&self, endpoint: Endpoint) -> QueryBuilder<T> {
        QueryBuilder::<T>::new(self.clone(), endpoint)
    }

    pub fn sgv(&self) -> QueryBuilder<SgvEntry> {
        QueryBuilder::<SgvEntry>::new(self.clone(), Endpoint::Svg)
    }

    pub fn mbg(&self) -> QueryBuilder<MbgEntry> {
        QueryBuilder::<MbgEntry>::new(self.clone(), Endpoint::Mbg)
    }

    pub async fn iob(&self) -> reqwest::Result<IobData> {
        let url = self
            .base_url
            .join(Endpoint::Iob.as_path())
            .expect("Error building the URL");

        let mut request = self.http.get(url);

        if let Some(secret) = &self.api_secret {
            request = request.header("api-secret", secret);
        }

        let res = request.send().await?;
        let wrapper = res.json::<IobWrapper>().await?;

        Ok(wrapper.iob)
    }
}
