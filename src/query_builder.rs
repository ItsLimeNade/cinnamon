use super::client::NightscoutClient;
use super::structs::endpoints::Endpoint;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;

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
