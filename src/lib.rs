use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client as HttpClient;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;
use url::Url;

/// SGV (Sensor Glucose Value)
/// This struct represents blood glucose values automatically entered by a CGM (continuous glucose monitor)
#[derive(Debug, Serialize, Deserialize)]
pub struct SgvEntry {
    #[serde(rename = "_id")]
    id: String,
    sgv: u16,
    date: u64,
    #[serde(rename = "dateString")]
    date_string: String,
    direction: String,
    #[serde(rename = "type")]
    _type: String,
}

/// MBG (Meter Blood Glucose)
/// This struct represents blood glucose data manually entered by the user, often obtained via a fingerprick.
/// https://en.wikipedia.org/wiki/Fingerstick
#[derive(Debug, Serialize, Deserialize)]
pub struct MbgEntry {
    #[serde(rename = "_id")]
    id: String,
    mbg: u16,
    date: u64,
    #[serde(rename = "dateString")]
    date_string: String,
    #[serde(rename = "type")]
    _type: String,
}

#[derive(Debug, Deserialize)]
pub struct IobWrapper {
    iob: IobData,
}

/// IOB (Insulin On Board)
/// This struct represents the current ammount of insulin inside the user's system.
#[derive(Debug, Serialize, Deserialize)]
pub struct IobData {
    iob: f64,
    #[serde(rename = "displayLine")]
    display_line: String,
}

pub enum Endpoint {
    Svg,
    Mbg,
    Iob,
}

impl Endpoint {
    pub fn as_path(&self) -> &'static str {
        match self {
            Endpoint::Svg => "api/v2/entries/sgv.json",
            Endpoint::Mbg => "api/v2/entries/mbg.json",
            Endpoint::Iob => "api/v2/properties/iob.json",
        }
    }
}

#[derive(Clone)]
pub struct NightscoutClient {
    base_url: Url,
    api_secret: Option<String>,
    http: HttpClient,
}

impl NightscoutClient {
    pub fn new(base_url: &str, api_secret: Option<String>) -> Result<Self> {
        Ok(Self {
            base_url: Url::parse(base_url)?,
            http: HttpClient::new(),
            api_secret,
        })
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

pub struct QueryBuilder<T> {
    client: NightscoutClient,
    endpoint: Endpoint,
    from_date: Option<DateTime<Utc>>,
    to_date: Option<DateTime<Utc>>,
    count: usize,
    _marker: PhantomData<T>,
}

impl<T> QueryBuilder<T> {
    pub fn new(client: NightscoutClient, endpoint: Endpoint) -> Self {
        Self {
            client,
            endpoint,
            from_date: None,
            to_date: None,
            count: 10,
            _marker: PhantomData,
        }
    }

    pub fn from(mut self, date: DateTime<Utc>) -> Self {
        self.from_date = Some(date);
        self
    }

    pub fn to(mut self, date: DateTime<Utc>) -> Self {
        self.to_date = Some(date);
        self
    }

    pub fn limit(mut self, count: usize) -> Self {
        self.count = count;
        self
    }
}

impl<T> IntoFuture for QueryBuilder<T>
where
    T: DeserializeOwned + Send + 'static,
{
    type Output = Result<Vec<T>, reqwest::Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let mut url = self
                .client
                .base_url
                .join(self.endpoint.as_path())
                .expect("Error building the URL");

            {
                let mut query = url.query_pairs_mut();

                query.append_pair("count", &self.count.to_string());

                if let Some(from) = self.from_date {
                    query.append_pair("find[dateString][$gte]", &from.to_rfc3339());
                }

                if let Some(to) = self.to_date {
                    query.append_pair("find[dateString][$lte]", &to.to_rfc3339());
                }
            }

            let mut request = self.client.http.get(url);

            if let Some(secret) = &self.client.api_secret {
                request = request.header("api-secret", secret);
            }

            let response = request.send().await?;
            response.json::<Vec<T>>().await
        })
    }
}
