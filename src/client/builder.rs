use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use url::{Host, Url};

use super::{ApiVersion, Client, Credentials, Inner, RetryPolicy, transport::Request};
use crate::error::{Error, Result};

/// Configures and builds a [`Client`].
///
/// ```no_run
/// # async fn run() -> cinnamon::Result<()> {
/// use std::time::Duration;
/// use cinnamon::{ApiVersion, Client, Credentials};
///
/// let ns = Client::builder("https://my-ns.example.com")
///     .credentials(Credentials::access_token("app-0123456789abcdef"))
///     .app_name("my-dashboard")
///     .timeout(Duration::from_secs(20))
///     .api(ApiVersion::Auto)
///     .connect()
///     .await?;
/// # Ok(()) }
/// ```
#[derive(Debug)]
#[must_use]
pub struct ClientBuilder {
    url: String,
    credentials: Credentials,
    http: Option<reqwest::Client>,
    timeout: Duration,
    connect_timeout: Duration,
    user_agent: Option<String>,
    app: String,
    api: ApiVersion,
    retry: RetryPolicy,
    allow_insecure_http: bool,
}

impl ClientBuilder {
    pub(crate) fn new(url: &str) -> Self {
        Self {
            url: url.trim().to_owned(),
            credentials: Credentials::none(),
            http: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            user_agent: None,
            app: "cinnamon".to_owned(),
            api: ApiVersion::Auto,
            retry: RetryPolicy::default(),
            allow_insecure_http: false,
        }
    }

    /// How to authenticate. Defaults to [`Credentials::none`].
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Shorthand for `.credentials(Credentials::access_token(token))`.
    pub fn access_token(self, token: impl Into<String>) -> Self {
        self.credentials(Credentials::access_token(token))
    }

    /// Shorthand for `.credentials(Credentials::api_secret(secret))`.
    pub fn api_secret(self, secret: impl Into<String>) -> Self {
        self.credentials(Credentials::api_secret(secret))
    }

    /// The application name recorded as `app` on API v3 writes and used as the default
    /// `device` of documents this client creates. Defaults to `"cinnamon"`.
    pub fn app_name(mut self, app: impl Into<String>) -> Self {
        self.app = app.into();
        self
    }

    /// Total time allowed per HTTP attempt (default 30 s). Ignored with [`Self::http_client`].
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Time allowed to establish a connection (default 10 s). Ignored with
    /// [`Self::http_client`].
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Overrides the `User-Agent` (default `cinnamon/<version>`). Ignored with
    /// [`Self::http_client`].
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Uses a preconfigured `reqwest::Client` (for proxies, custom roots, shared pools).
    ///
    /// Configure it with `redirect(reqwest::redirect::Policy::none())`: a client that follows
    /// redirects can forward the `api-secret` header to another host.
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http = Some(client);
        self
    }

    /// Which API generation to use (default [`ApiVersion::Auto`]).
    pub const fn api(mut self, api: ApiVersion) -> Self {
        self.api = api;
        self
    }

    /// Retry behavior for transient failures (default: 2 retries with backoff).
    pub const fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Allows plain `http://` for non-local hosts. Credentials and health data then travel
    /// unencrypted; only use this on a trusted network.
    pub const fn allow_insecure_http(mut self) -> Self {
        self.allow_insecure_http = true;
        self
    }

    /// Builds the client without contacting the server.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidUrl`] for malformed URLs, [`Error::InsecureTransport`] for `http://`
    /// URLs to non-local hosts, and [`Error::Transport`] if the HTTP client cannot be built.
    pub fn build(self) -> Result<Client> {
        let base = normalize_base_url(&self.url)?;
        if base.scheme() == "http" && !self.allow_insecure_http && !is_local(&base) {
            return Err(Error::InsecureTransport {
                host: base.host_str().unwrap_or_default().to_owned(),
            });
        }

        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(self.timeout)
                .connect_timeout(self.connect_timeout)
                .user_agent(
                    self.user_agent
                        .unwrap_or_else(|| format!("cinnamon/{}", env!("CARGO_PKG_VERSION"))),
                )
                .build()
                .map_err(|e| Error::Transport(Box::new(e)))?,
        };

        Ok(Client {
            inner: Arc::new(Inner {
                base,
                http,
                credentials: self.credentials,
                jwt: tokio::sync::Mutex::new(None),
                app: self.app,
                api: self.api,
                retry: self.retry,
                server: tokio::sync::OnceCell::new(),
            }),
        })
    }

    /// Builds the client, runs discovery and verifies the credentials.
    ///
    /// # Errors
    ///
    /// Everything [`Self::build`] returns, plus [`Error::Unauthorized`] when the server
    /// rejects the credentials.
    pub async fn connect(self) -> Result<Client> {
        let client = self.build()?;
        let inner = &client.inner;
        if inner.credentials.is_access_token() {
            inner.bearer().await?;
        } else if inner.credentials.is_api_secret() {
            let resp = inner
                .execute(Request::get("api/v1/verifyauth", "api/v1/verifyauth"))
                .await?;
            let body: serde_json::Value = resp.json()?;
            let message = &body["message"];
            if message["message"] != "OK" || message["isAdmin"] != true {
                return Err(Error::Unauthorized {
                    message: "Nightscout rejected the API secret".into(),
                });
            }
        }
        inner.server_info().await?;
        Ok(client)
    }
}

fn normalize_base_url(raw: &str) -> Result<Url> {
    let mut url = Url::parse(raw).map_err(|e| Error::InvalidUrl(format!("{raw}: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::InvalidUrl(format!(
            "{raw}: only http and https URLs are supported"
        )));
    }
    if url.host().is_none() {
        return Err(Error::InvalidUrl(format!("{raw}: missing host")));
    }
    url.set_query(None);
    url.set_fragment(None);
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn is_local(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(d)) => d == "localhost" || d.ends_with(".localhost"),
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_is_normalized() {
        let url = normalize_base_url("https://ns.example.com/sub?x=1#frag")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(url.as_str(), "https://ns.example.com/sub/");
        assert!(normalize_base_url("ftp://ns.example.com").is_err());
        assert!(normalize_base_url("not a url").is_err());
    }

    #[test]
    fn plain_http_is_refused_for_remote_hosts() {
        let err = Client::builder("http://ns.example.com").build().map(|_| ());
        assert!(matches!(err, Err(Error::InsecureTransport { .. })));
        assert!(Client::builder("http://localhost:1337").build().is_ok());
        assert!(Client::builder("http://127.0.0.1:1337").build().is_ok());
        assert!(
            Client::builder("http://ns.example.com")
                .allow_insecure_http()
                .build()
                .is_ok()
        );
    }
}
