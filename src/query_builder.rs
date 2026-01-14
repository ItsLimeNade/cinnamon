use super::client::NightscoutClient;
use super::structs::endpoints::Endpoint;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;
use reqwest::Method;

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
    T: DeserializeOwned + Send + 'static,
{   
    type Output = Result<Vec<T>, reqwest::Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;
    
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let path = if let Some(id) = &self.id {
                format!("{}/{}", self.endpoint.as_path(), id)
            } else {
                self.endpoint.as_path().to_string()
            };

            let mut url = self
                .client
                .base_url
                .join(&path)
                .expect("Error building the URL");

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
                    
                    let response = request.send().await?;

                    if self.id.is_some() {
                        let item = response.json::<Vec<T>>().await?;
                        Ok(item)
                    } else {
                        response.json::<Vec<T>>().await
                    }
                }
                Method::DELETE => {
                    if self.id.is_some() {
                        let mut get_req = self.client.http.get(url.clone());
                        get_req = self.client.auth(get_req);

                        let item = get_req.send().await?.json::<Vec<T>>().await?;

                        let mut del_req = self.client.http.delete(url);
                        
                        del_req = self.client.auth(del_req);

                        del_req.send().await?;

                        Ok(item)
                    } else {
                        let mut get_request = self.client.http.get(url.clone());
                        get_request = self.client.auth(get_request);

                        let get_response = get_request.send().await?;
                        let items: Vec<serde_json::Value> = get_response.json().await?;

                        for item in &items {
                            if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                                let delete_path = format!("{}/{}", self.endpoint.as_path(), id);
                                let delete_url = self.client.base_url.join(&delete_path)
                                    .expect("Error building ID-based delete URL");

                                let mut delete_req = self.client.http.delete(delete_url);
                                
                                delete_req = self.client.auth(delete_req);
                                
                                let _ = delete_req.send().await;
                            }
                        }

                        let t_items: Vec<T> = serde_json::from_value(serde_json::Value::Array(items))
                            .expect("Data format mismatch: Could not deserialize deleted items into T");

                        Ok(t_items)
                    }
                }
                _ => panic!("Method not supported by the QueryBuilder!"),
            }
        })
    }
}