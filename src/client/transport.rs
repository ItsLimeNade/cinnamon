//! The single request path every API call goes through.

use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, LOCATION, RETRY_AFTER};
use reqwest::{Method, RequestBuilder, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::Inner;
use super::auth::{Jwt, JwtResponse, Kind};
use crate::error::{Error, Result};

/// Emits a `tracing` debug event when the `tracing` feature is on; compiles to nothing otherwise.
macro_rules! trace_event {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            tracing::debug!(target: "cinnamon", $($arg)*);
        }
    };
}

/// A cached JWT older than this is refreshed once on a 401 (the server may have been
/// redeployed, which regenerates its signing key). Younger ones are trusted: a 401 then
/// means "insufficient permission", and retrying would only extend Nightscout's
/// per-IP auth-failure delay.
const JWT_REFRESH_ON_401_AGE: Duration = Duration::from_secs(60);

/// Upper bound for honoring a server's `Retry-After`.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// A request relative to the client's base URL.
#[derive(Debug)]
pub(crate) struct Request {
    method: Method,
    path: String,
    /// Path template used in traces, so ids and tokens never reach logs.
    #[cfg_attr(not(feature = "tracing"), allow(dead_code))]
    route: &'static str,
    query: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    authenticated: bool,
    idempotent: bool,
}

impl Request {
    pub(crate) fn new(method: Method, path: impl Into<String>, route: &'static str) -> Self {
        let idempotent = !matches!(method, Method::POST | Method::PATCH);
        Self {
            method,
            path: path.into(),
            route,
            query: Vec::new(),
            body: None,
            authenticated: true,
            idempotent,
        }
    }

    pub(crate) fn get(path: impl Into<String>, route: &'static str) -> Self {
        Self::new(Method::GET, path, route)
    }

    pub(crate) fn post(path: impl Into<String>, route: &'static str) -> Self {
        Self::new(Method::POST, path, route)
    }

    pub(crate) fn put(path: impl Into<String>, route: &'static str) -> Self {
        Self::new(Method::PUT, path, route)
    }

    pub(crate) fn patch(path: impl Into<String>, route: &'static str) -> Self {
        Self::new(Method::PATCH, path, route)
    }

    pub(crate) fn delete(path: impl Into<String>, route: &'static str) -> Self {
        Self::new(Method::DELETE, path, route)
    }

    /// Attaches a JSON body.
    pub(crate) fn json<T: Serialize + ?Sized>(mut self, body: &T) -> Result<Self> {
        let bytes = serde_json::to_vec(body)
            .map_err(|e| Error::InvalidInput(format!("could not serialize request body: {e}")))?;
        self.body = Some(bytes);
        Ok(self)
    }

    pub(crate) fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((key.into(), value.into()));
        self
    }

    pub(crate) fn queries<I>(mut self, pairs: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.query.extend(pairs);
        self
    }

    /// Sends the request without credentials.
    pub(crate) const fn anonymous(mut self) -> Self {
        self.authenticated = false;
        self
    }

    /// Overrides whether the request may be retried after a transient failure.
    pub(crate) const fn idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent = idempotent;
        self
    }

    fn is_write(&self) -> bool {
        !matches!(self.method, Method::GET | Method::HEAD)
    }
}

/// A buffered successful (2xx or 304) response.
#[derive(Debug)]
pub(crate) struct Response {
    pub(crate) body: bytes::Bytes,
}

impl Response {
    /// Decodes the whole body.
    pub(crate) fn json<T: DeserializeOwned>(&self) -> Result<T> {
        decode(&self.body)
    }

    /// Decodes the `result` member of an API v3 envelope (`{"status":200,"result":…}`).
    pub(crate) fn v3<T: DeserializeOwned>(&self) -> Result<T> {
        #[derive(serde::Deserialize)]
        struct Envelope<T> {
            result: T,
        }
        decode::<Envelope<T>>(&self.body).map(|e| e.result)
    }
}

/// Deserializes JSON, reporting the path of the first field that fails.
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let de = &mut serde_json::Deserializer::from_slice(bytes);
    serde_path_to_error::deserialize(de).map_err(Error::decode)
}

/// Characters escaped in a single URL path segment.
const SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Percent-encodes one path segment (ids, names, patterns).
pub(crate) fn encode_segment(segment: &str) -> String {
    percent_encoding::utf8_percent_encode(segment, SEGMENT).to_string()
}

/// Converts a JSON value, reporting the path of the first field that fails.
pub(crate) fn decode_value<T: DeserializeOwned>(value: serde_json::Value) -> Result<T> {
    serde_path_to_error::deserialize(value).map_err(Error::decode)
}

impl Inner {
    /// Sends a request, applying credentials, retries and error mapping.
    pub(crate) async fn execute(&self, req: Request) -> Result<Response> {
        let mut url = self.base.join(&req.path)?;
        if !req.query.is_empty() {
            url.query_pairs_mut().extend_pairs(&req.query);
        }

        let mut attempt: u32 = 0;
        let mut refreshed_jwt = false;
        loop {
            let started = Instant::now();
            let mut builder = self
                .http
                .request(req.method.clone(), url.clone())
                .header(ACCEPT, "application/json");
            if let Some(body) = &req.body {
                builder = builder
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone());
            }
            let mut stale_jwt = false;
            if req.authenticated {
                (builder, stale_jwt) = self.authorize(builder).await?;
            }

            let response = match builder.send().await {
                Ok(response) => response,
                Err(err) => {
                    let err = Error::from_reqwest(err);
                    trace_event!(
                        method = %req.method,
                        route = req.route,
                        attempt,
                        error = %err,
                        "request failed"
                    );
                    if req.idempotent && err.is_transient() && attempt < self.retry.max_retries {
                        tokio::time::sleep(self.retry.delay(attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
            };

            let status = response.status();
            trace_event!(
                method = %req.method,
                route = req.route,
                status = status.as_u16(),
                attempt,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "request completed"
            );
            let _ = started;

            if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_owned();
                return Err(Error::Redirected {
                    status: status.as_u16(),
                    location,
                });
            }

            if status == StatusCode::UNAUTHORIZED && stale_jwt && !refreshed_jwt {
                self.invalidate_jwt().await;
                refreshed_jwt = true;
                continue;
            }

            let headers = response.headers().clone();
            let body = response.bytes().await.map_err(Error::from_reqwest)?;
            if status.is_success() || status == StatusCode::NOT_MODIFIED {
                return Ok(Response { body });
            }

            let err = Error::from_response(status, &body, req.is_write());
            if req.idempotent && err.is_transient() && attempt < self.retry.max_retries {
                tokio::time::sleep(self.retry.delay(attempt, retry_after(&headers))).await;
                attempt += 1;
                continue;
            }
            return Err(err);
        }
    }

    /// Adds credentials. Returns whether a JWT old enough to be refreshed on 401 was used.
    async fn authorize(&self, builder: RequestBuilder) -> Result<(RequestBuilder, bool)> {
        match &self.credentials.0 {
            Kind::None => Ok((builder, false)),
            Kind::ApiSecret(hash) => {
                Ok((builder.header("api-secret", hash.expose_secret()), false))
            }
            Kind::AccessToken(_) => {
                let (jwt, age) = self.bearer().await?;
                Ok((
                    builder.bearer_auth(jwt.expose_secret()),
                    age >= JWT_REFRESH_ON_401_AGE,
                ))
            }
        }
    }

    /// Returns a valid JWT, exchanging the access token when there is none or it is about to
    /// expire. Concurrent callers wait on the same exchange (single flight).
    pub(crate) async fn bearer(&self) -> Result<(SecretString, Duration)> {
        let mut slot = self.jwt.lock().await;
        if let Some(jwt) = slot.as_ref().filter(|jwt| jwt.is_fresh()) {
            return Ok((jwt.token.clone(), jwt.obtained.elapsed()));
        }
        let jwt = self.exchange_token().await?;
        let token = jwt.token.clone();
        *slot = Some(jwt);
        Ok((token, Duration::ZERO))
    }

    pub(crate) async fn invalidate_jwt(&self) {
        *self.jwt.lock().await = None;
    }

    pub(crate) async fn session(&self) -> Result<Option<super::Session>> {
        if !self.credentials.is_access_token() {
            return Ok(None);
        }
        self.bearer().await?;
        Ok(self
            .jwt
            .lock()
            .await
            .as_ref()
            .map(|jwt| jwt.session.clone()))
    }

    async fn exchange_token(&self) -> Result<Jwt> {
        let Some(token) = self.credentials.access_token_secret() else {
            return Err(Error::Unsupported {
                feature: "JWT exchange",
                reason: "the client has no access token".into(),
            });
        };
        let encoded = encode_segment(token.expose_secret());
        let req = Request::get(
            format!("api/v2/authorization/request/{encoded}"),
            "api/v2/authorization/request/{token}",
        )
        .anonymous();
        match Box::pin(self.execute(req)).await {
            Ok(resp) => resp.json::<JwtResponse>().map(Jwt::from_response),
            Err(Error::Unauthorized { .. }) => Err(Error::Unauthorized {
                message: "Nightscout rejected the access token (tokens are listed under Admin \
                          Tools; changing API_SECRET rotates every token)"
                    .into(),
            }),
            Err(err) => Err(err),
        }
    }
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let secs: u64 = headers
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(secs).min(MAX_RETRY_AFTER))
}
