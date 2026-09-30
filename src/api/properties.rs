//! API v2 computed state: properties, ddata and summary.

use std::future::IntoFuture;

use futures_util::future::BoxFuture;

use crate::client::Client;
use crate::client::transport::{Request, encode_segment};
use crate::error::Result;
use crate::model::Timestamp;
use crate::model::ddata::DData;
use crate::model::properties::{Properties, Property};
use crate::model::summary::Summary;

/// A request for `/api/v2/properties`; `.await` it.
///
/// ```no_run
/// # async fn run(ns: cinnamon::Client) -> cinnamon::Result<()> {
/// use cinnamon::model::properties::Property;
///
/// let props = ns.properties().only([Property::Iob, Property::Cob]).await?;
/// if let Some(iob) = props.iob.and_then(|p| p.iob) {
///     println!("IOB {iob:.2} U");
/// }
/// # Ok(()) }
/// ```
#[derive(Debug)]
#[must_use = "a request does nothing until awaited"]
pub struct PropertiesRequest {
    client: Client,
    only: Vec<Property>,
}

impl PropertiesRequest {
    pub(crate) const fn new(client: Client) -> Self {
        Self {
            client,
            only: Vec::new(),
        }
    }

    /// Restricts the response to these properties (smaller payload, less server work).
    pub fn only<I: IntoIterator<Item = Property>>(mut self, properties: I) -> Self {
        self.only.extend(properties);
        self
    }

    /// Runs the request.
    ///
    /// # Errors
    ///
    /// Transport and authorization errors (needs `api:entries:read` and
    /// `api:treatments:read`).
    pub async fn send(self) -> Result<Properties> {
        let path = if self.only.is_empty() {
            "api/v2/properties".to_owned()
        } else {
            let names: Vec<String> = self
                .only
                .iter()
                .map(|p| encode_segment(p.as_str()))
                .collect();
            format!("api/v2/properties/{}", names.join(","))
        };
        self.client
            .inner
            .execute(Request::get(path, "api/v2/properties/{names}"))
            .await?
            .json()
    }
}

impl IntoFuture for PropertiesRequest {
    type Output = Result<Properties>;
    type IntoFuture = BoxFuture<'static, Result<Properties>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

pub(crate) async fn ddata(client: &Client, at: Option<Timestamp>) -> Result<DData> {
    let path = match at {
        Some(at) => format!("api/v2/ddata/at/{}", encode_segment(&at.to_iso())),
        None => "api/v2/ddata/at".to_owned(),
    };
    client
        .inner
        .execute(Request::get(path, "api/v2/ddata/at/{at}"))
        .await?
        .json()
}

pub(crate) async fn summary(client: &Client, hours: u32) -> Result<Summary> {
    client
        .inner
        .execute(Request::get("api/v2/summary", "api/v2/summary").query("hours", hours.to_string()))
        .await?
        .json()
}
