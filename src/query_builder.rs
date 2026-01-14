use crate::error::NightscoutError;

use super::client::NightscoutClient;
use crate::endpoints::Endpoint;

use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use reqwest::Method;
use futures::stream::{self, StreamExt};

pub struct QueryBuilder<T> {
    client: NightscoutClient,
    endpoint: Endpoint,
    from_date: Option<DateTime<Utc>>,
    to_date: Option<DateTime<Utc>>,
    count: usize,
    method: Method,
    id: Option<String>,
    _marker: PhantomData<T>,
}

impl<T> QueryBuilder<T> {
    pub fn new(client: NightscoutClient, endpoint: Endpoint, method: Method) -> Self {
        Self {
            client,
            endpoint,
            from_date: None,
            to_date: None,
            count: 10,
            method,
            id: None, 
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

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl<T> IntoFuture for QueryBuilder<T>
where
    T: DeserializeOwned + Send + Sync + 'static, // Added Sync mostly for safety in async iterators
{   
    type Output = Result<Vec<T>, NightscoutError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;
    
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let path = if let Some(id) = &self.id {
                format!("{}/{}", self.endpoint.as_path(), id)
            } else {
                self.endpoint.as_path().to_string()
            };

            let mut url = self.client.base_url.join(&path)?;

            {
                let mut query = url.query_pairs_mut();
                
                if self.id.is_none() {
                    query.append_pair("count", &self.count.to_string());

                    if let Some(from) = self.from_date {
                        query.append_pair("find[dateString][$gte]", &from.to_rfc3339());
                    }

                    if let Some(to) = self.to_date {
                        query.append_pair("find[dateString][$lte]", &to.to_rfc3339());
                    }
                }
            }

            match self.method {
                Method::GET => {
                    let mut request = self.client.http.get(url);
                    request = self.client.auth(request);
                    
                    let response = self.client.send_checked(request).await?;

                    if self.id.is_some() {
                        let item = response.json::<Vec<T>>().await?;
                        Ok(item)
                    } else {
                        Ok(response.json::<Vec<T>>().await?)
                    }
                }
                Method::DELETE => {
                    if self.id.is_some() {
                        // Single ID delete logic
                        let mut get_req = self.client.http.get(url.clone());
                        get_req = self.client.auth(get_req);
                        let item = self.client.send_checked(get_req).await?.json::<Vec<T>>().await?;

                        let mut del_req = self.client.http.delete(url);
                        del_req = self.client.auth(del_req);
                        self.client.send_checked(del_req).await?;

                        Ok(item)
                    } else {
                        let mut get_request = self.client.http.get(url.clone());
                        get_request = self.client.auth(get_request);

                        let get_response = self.client.send_checked(get_request).await?;
                        let items: Vec<serde_json::Value> = get_response.json().await?;

                        let delete_urls: Vec<reqwest::Url> = items.iter()
                            .filter_map(|item| {
                                let id = item.get("_id")?.as_str()?;
                                let delete_path = format!("{}/{}", self.endpoint.as_path(), id);
                                self.client.base_url.join(&delete_path).ok()
                            })
                            .collect();

                        let delete_tasks = delete_urls.into_iter().map(|url| {
                            let client = self.client.clone();
                            async move {
                                let mut req = client.http.delete(url);
                                req = client.auth(req);
                                client.send_checked(req).await
                            }
                        });

                        stream::iter(delete_tasks)
                            .buffer_unordered(10)
                            .collect::<Vec<_>>()
                            .await;

                        let t_items: Vec<T> = serde_json::from_value(serde_json::Value::Array(items))?;
                        Ok(t_items)
                    }
                }
                _ => Err(NightscoutError::Unknown),
            }
        })
    }
}
