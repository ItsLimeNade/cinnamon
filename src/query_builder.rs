use super::client::NightscoutClient;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;

use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use reqwest::Method;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug, PartialEq)]
/// Specifies target device filtering behavior.
pub enum Device {
    /// Automatically attempts to determine the primary device name from recent data.
    /// Performs an extra HTTP request (pre-flight) to find the device name.
    Auto,
    /// Fetches data from all devices.
    All,
    /// Fetches data only from a specific device name (e.g., "bubble").
    Custom(String),
}

/// Trait for models that contain a device name field.
pub trait HasDevice {
    fn device(&self) -> Option<&str>;
}

pub struct QueryBuilder<T> {
    client: NightscoutClient,
    endpoint: Endpoint,
    from_date: Option<DateTime<Utc>>,
    to_date: Option<DateTime<Utc>>,
    count: usize,
    method: Method,
    id: Option<String>,
    device: Device,
    date_field: String,
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
            device: Device::All,
            date_field: "dateString".to_string(),
            _marker: PhantomData,
        }
    }

    /// Filters results to entries occurring on or after this date.
    pub fn from(mut self, date: DateTime<Utc>) -> Self {
        self.from_date = Some(date);
        self
    }

    /// Filters results to entries occurring on or before this date.
    pub fn to(mut self, date: DateTime<Utc>) -> Self {
        self.to_date = Some(date);
        self
    }

    /// Limits the number of results returned. Default is 10.
    pub fn limit(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// targets a specific resource ID.
    ///
    /// When used with `GET`, this fetches a single item.
    /// When used with `DELETE`, this deletes a single item.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Some nightscout entries use different date filter names
    ///
    /// This function allows to override the default dateString date field query
    /// param name.
    pub(crate) fn with_date_field(mut self, field: impl Into<String>) -> Self {
        self.date_field = field.into();
        self
    }

    /// Filters results by device name.
    pub fn device(mut self, device: Device) -> Self {
        self.device = device;
        self
    }
}

impl<T> QueryBuilder<T>
where
    T: DeserializeOwned + Send + Sync + 'static + HasDevice,
{
    /// Executes the built query.
    ///
    /// This method sends the HTTP request to Nightscout constructed by the builder methods.
    pub async fn send(self) -> Result<Vec<T>, NightscoutError> {
        // For Device::Auto, it is needed to do a pre-flight to determine which device to use.
        // While it has performance impact, it's a good tradeoff if you do not know the device
        // names on the server and only want data from one device.
        let resolved_device_name: Option<String> = match &self.device {
            Device::Custom(name) => Some(name.clone()),
            Device::Auto => {
                let mut probe_url = self.client.base_url.join(self.endpoint.as_path())?;
                {
                    let mut query = probe_url.query_pairs_mut();
                    query.append_pair("count", "1");

                    // We still need to access the data at the interval which the user wants us to get data
                    // if we didn't the device name could be (and probably will be) total wrong.
                    if let Some(from) = self.from_date {
                        let key = format!("find[{}][$gte]", self.date_field);
                        query.append_pair(&key, &from.to_rfc3339());
                    }

                    if let Some(to) = self.to_date {
                        let key = format!("find[{}][$lte]", self.date_field);
                        query.append_pair(&key, &to.to_rfc3339());
                    }
                }
                let probe_result: Result<Vec<T>, _> = self.client.fetch(probe_url).await;

                match probe_result {
                    Ok(items) => items
                        .first()
                        .and_then(|item| item.device())
                        .map(|s| s.to_string()),
                    Err(_) => None,
                }
            }
            Device::All => None,
        };

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
                    let key = format!("find[{}][$gte]", self.date_field);
                    query.append_pair(&key, &from.to_rfc3339());
                }

                if let Some(to) = self.to_date {
                    let key = format!("find[{}][$lte]", self.date_field);
                    query.append_pair(&key, &to.to_rfc3339());
                }

                if let Some(name) = &resolved_device_name {
                    query.append_pair("find[device]", name);
                }
            }
        }

        match self.method {
            Method::GET => {
                let items: Vec<T> = self.client.fetch(url).await?;
                Ok(items)
            }
            Method::DELETE => {
                if self.id.is_some() {
                    let item: Vec<T> = self.client.fetch(url.clone()).await?;

                    let mut del_req = self.client.http.delete(url);
                    del_req = self.client.auth(del_req);
                    self.client.send_checked(del_req).await?;

                    Ok(item)
                } else {
                    let items: Vec<serde_json::Value> = self.client.fetch(url.clone()).await?;

                    let delete_urls: Vec<reqwest::Url> = items
                        .iter()
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
    }
}
