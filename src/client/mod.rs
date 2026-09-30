//! The Nightscout client, its builder and credentials.

mod auth;
mod builder;
mod discovery;
pub(crate) mod transport;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use url::Url;

pub use auth::{Credentials, Session};
pub use builder::ClientBuilder;
pub use discovery::ServerInfo;
pub(crate) use discovery::{Api, Support};

use crate::error::Result;

/// Which Nightscout API generation the client uses for operations both APIs implement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApiVersion {
    /// API v3 when the server has it and the client holds an access token, otherwise v1/v2.
    #[default]
    Auto,
    /// Always API v3 (requires an access token).
    V3,
    /// Always API v1/v2. v3-only operations (settings, history, patch, soft delete) fail
    /// with [`Error::Unsupported`](crate::Error::Unsupported).
    V1,
}

/// Retry behavior for transient failures (timeouts, connection errors, 408/429/5xx).
///
/// Only idempotent requests are retried: reads, `PUT`, `DELETE`, and API v3 creates, which
/// carry a client-computed `identifier` so a replay deduplicates server-side. A 401 is never
/// retried, because each failed authentication makes Nightscout delay every later request
/// from the same IP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl RetryPolicy {
    /// Never retry.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            max_retries: 0,
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        }
    }

    /// Up to `max_retries` retries with exponential backoff between `base_delay` and
    /// `max_delay` (with jitter).
    #[must_use]
    pub const fn new(max_retries: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
            max_delay,
        }
    }

    pub(crate) fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        if let Some(wait) = retry_after {
            return wait;
        }
        let exp = self
            .base_delay
            .saturating_mul(1u32 << attempt.min(16))
            .min(self.max_delay);
        // "Equal jitter": half fixed, half random.
        exp / 2 + exp.mul_f64(fastrand::f64() / 2.0)
    }
}

impl Default for RetryPolicy {
    /// Two retries, 300 ms base delay, 5 s cap.
    fn default() -> Self {
        Self::new(2, Duration::from_millis(300), Duration::from_secs(5))
    }
}

/// An async Nightscout client.
///
/// Cheap to clone: clones share the connection pool, the JWT session and discovery results.
///
/// ```no_run
/// # async fn run() -> cinnamon::Result<()> {
/// use cinnamon::{Client, Credentials};
///
/// let ns = Client::connect("https://my-ns.example.com", Credentials::access_token("app-0123456789abcdef")).await?;
/// if let Some(latest) = ns.entries().sgv().latest().await? {
///     println!("{} {}", latest.sgv, latest.direction.as_ref().map_or("", |d| d.arrow()));
/// }
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) base: Url,
    pub(crate) http: reqwest::Client,
    pub(crate) credentials: Credentials,
    pub(crate) jwt: tokio::sync::Mutex<Option<auth::Jwt>>,
    pub(crate) app: String,
    pub(crate) api: ApiVersion,
    pub(crate) retry: RetryPolicy,
    pub(crate) server: tokio::sync::OnceCell<ServerInfo>,
}

impl Client {
    /// Starts configuring a client for the Nightscout at `url`.
    pub fn builder(url: impl AsRef<str>) -> ClientBuilder {
        ClientBuilder::new(url.as_ref())
    }

    /// Builds a client and verifies the credentials against the server before returning, so
    /// a wrong token fails here instead of silently falling back to anonymous access.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`](crate::Error::Unauthorized) when the credentials are rejected,
    /// plus any URL, transport or TLS error.
    pub async fn connect(url: impl AsRef<str>, credentials: Credentials) -> Result<Self> {
        Self::builder(url).credentials(credentials).connect().await
    }

    /// The base URL requests are resolved against (always ends with `/`).
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.inner.base
    }

    /// The `app` name sent with API v3 writes and used as the default `device`.
    #[must_use]
    pub fn app_name(&self) -> &str {
        &self.inner.app
    }

    /// Server version and API v3 availability (discovered once, then cached).
    ///
    /// # Errors
    ///
    /// Transport errors while probing `/api/v3/version`.
    pub async fn server_info(&self) -> Result<ServerInfo> {
        self.inner.server_info().await.cloned()
    }

    /// The authenticated subject and its permissions. `None` unless the client uses an
    /// access token.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`](crate::Error::Unauthorized) when the token is rejected.
    pub async fn session(&self) -> Result<Option<Session>> {
        self.inner.session().await
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.base.as_str())
            .field("credentials", &self.inner.credentials)
            .field("app", &self.inner.app)
            .field("api", &self.inner.api)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_bounded_and_jittered() {
        let policy = RetryPolicy::new(5, Duration::from_millis(100), Duration::from_secs(1));
        for attempt in 0..10 {
            let d = policy.delay(attempt, None);
            assert!(d <= Duration::from_secs(1), "{d:?}");
        }
        assert!(policy.delay(0, None) >= Duration::from_millis(50));
        assert_eq!(
            policy.delay(3, Some(Duration::from_secs(7))),
            Duration::from_secs(7)
        );
    }
}
