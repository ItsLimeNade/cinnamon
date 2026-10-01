//! Credentials and the cached JWT session.

use std::fmt;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sha1::{Digest, Sha1};

/// How cinnamon authenticates against Nightscout.
///
/// Prefer [`Credentials::access_token`]: tokens are scoped to roles (for example `readable`
/// or `careportal`), work with API v3, and are exchanged for short-lived JWTs so the token
/// itself is only sent once per session. [`Credentials::api_secret`] grants full admin
/// access, only works with API v1/v2, and is sent on every request.
///
/// `Debug` never prints the secret material.
#[derive(Clone)]
pub struct Credentials(pub(crate) Kind);

#[derive(Clone)]
pub(crate) enum Kind {
    None,
    AccessToken(SecretString),
    /// Holds the lowercase SHA-1 hex digest; the plain secret is dropped immediately.
    ApiSecret(SecretString),
}

impl Credentials {
    /// Anonymous access, limited to what `AUTH_DEFAULT_ROLES` allows (API v1/v2 only).
    #[must_use]
    pub const fn none() -> Self {
        Self(Kind::None)
    }

    /// An access token created in Nightscout's Admin Tools, e.g. `myapp-1a2b3c4d5e6f7a8b`.
    #[must_use]
    pub fn access_token(token: impl Into<String>) -> Self {
        Self(Kind::AccessToken(SecretString::from(
            token.into().trim().to_owned(),
        )))
    }

    /// The server's `API_SECRET`. It is hashed with SHA-1 right away; only the digest is
    /// kept and sent in the `api-secret` header.
    ///
    /// This grants admin rights and is rejected by API v3. Use an access token when you can.
    #[must_use]
    pub fn api_secret(secret: impl Into<String>) -> Self {
        let secret = SecretString::from(secret.into());
        let digest = Sha1::digest(secret.expose_secret().as_bytes());
        let hex = digest.iter().fold(String::with_capacity(40), |mut s, b| {
            use fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        });
        Self(Kind::ApiSecret(SecretString::from(hex)))
    }

    /// Whether these are anonymous credentials.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self.0, Kind::None)
    }

    pub(crate) const fn is_access_token(&self) -> bool {
        matches!(self.0, Kind::AccessToken(_))
    }

    pub(crate) const fn is_api_secret(&self) -> bool {
        matches!(self.0, Kind::ApiSecret(_))
    }

    pub(crate) fn access_token_secret(&self) -> Option<&SecretString> {
        match &self.0 {
            Kind::AccessToken(t) => Some(t),
            _ => None,
        }
    }

    #[cfg_attr(not(feature = "realtime"), allow(dead_code))]
    pub(crate) fn api_secret_hash(&self) -> Option<&SecretString> {
        match &self.0 {
            Kind::ApiSecret(h) => Some(h),
            _ => None,
        }
    }
}

impl Default for Credentials {
    fn default() -> Self {
        Self::none()
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            Kind::None => "Credentials::None",
            Kind::AccessToken(_) => "Credentials::AccessToken(<redacted>)",
            Kind::ApiSecret(_) => "Credentials::ApiSecret(<redacted>)",
        })
    }
}

/// Who the client is authenticated as, from the JWT exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Session {
    /// The subject (token owner) name.
    pub subject: Option<String>,
    /// Permission groups granted to the subject, e.g. `[["*"]]` for admin.
    pub permission_groups: Vec<Vec<String>>,
    /// When the JWT was issued.
    pub issued_at: DateTime<Utc>,
    /// When the JWT expires (Nightscout issues 8-hour tokens).
    pub expires_at: DateTime<Utc>,
}

impl Session {
    /// Whether any granted permission matches `wanted` (Shiro-style, `*` wildcards).
    #[must_use]
    pub fn has_permission(&self, wanted: &str) -> bool {
        self.permission_groups
            .iter()
            .flatten()
            .any(|granted| shiro_match(granted, wanted))
    }
}

/// Minimal Shiro permission matching as used by Nightscout (`domain:collection:action`).
fn shiro_match(granted: &str, wanted: &str) -> bool {
    let mut g = granted.split(':');
    let mut w = wanted.split(':');
    loop {
        match (g.next(), w.next()) {
            (None, _) => return true,
            (Some(gp), Some(wp)) => {
                if gp != "*" && !gp.split(',').any(|alt| alt == wp) {
                    return false;
                }
            }
            (Some(gp), None) => return gp == "*",
        }
    }
}

/// Raw response of `GET /api/v2/authorization/request/{token}`.
#[derive(Deserialize)]
pub(crate) struct JwtResponse {
    pub(crate) token: String,
    #[serde(default)]
    pub(crate) sub: Option<String>,
    #[serde(default, rename = "permissionGroups")]
    pub(crate) permission_groups: Vec<Vec<String>>,
    pub(crate) iat: i64,
    pub(crate) exp: i64,
}

/// A JWT held in memory.
pub(crate) struct Jwt {
    pub(crate) token: SecretString,
    pub(crate) session: Session,
    pub(crate) obtained: Instant,
}

impl Jwt {
    /// Refresh this long before `exp` so in-flight requests never race the expiry.
    const REFRESH_MARGIN: Duration = Duration::from_secs(5 * 60);

    pub(crate) fn from_response(resp: JwtResponse) -> Self {
        let ts = |secs| DateTime::from_timestamp(secs, 0).unwrap_or_default();
        Self {
            token: SecretString::from(resp.token),
            session: Session {
                subject: resp.sub,
                permission_groups: resp.permission_groups,
                issued_at: ts(resp.iat),
                expires_at: ts(resp.exp),
            },
            obtained: Instant::now(),
        }
    }

    /// Measured on the local monotonic clock against the token's own lifetime (`exp - iat`),
    /// so a skewed system clock can neither keep an expired JWT nor force an exchange on
    /// every request.
    pub(crate) fn is_fresh(&self) -> bool {
        let lifetime = (self.session.expires_at - self.session.issued_at)
            .to_std()
            .unwrap_or_default();
        self.obtained.elapsed() + Self::REFRESH_MARGIN < lifetime
    }
}

impl fmt::Debug for Jwt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Jwt")
            .field("token", &"<redacted>")
            .field("session", &self.session)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_secret_is_hashed_and_redacted() {
        let creds = Credentials::api_secret("cinnamon-e2e-secret");
        let hash = creds
            .api_secret_hash()
            .map(|h| h.expose_secret().to_owned())
            .unwrap_or_default();
        assert_eq!(hash, "30143ac058893ef38d5e090c9719103474b9ec19");
        assert_eq!(format!("{creds:?}"), "Credentials::ApiSecret(<redacted>)");
    }

    #[test]
    fn jwt_freshness_ignores_clock_skew() {
        let jwt = |iat: i64, exp: i64| {
            Jwt::from_response(JwtResponse {
                token: "jwt".into(),
                sub: None,
                permission_groups: Vec::new(),
                iat,
                exp,
            })
        };
        // An 8-hour token from a server whose clock says 1990 is still fresh here...
        assert!(jwt(631_152_000, 631_152_000 + 8 * 3600).is_fresh());
        // ...and a token living less than the refresh margin never is.
        assert!(!jwt(631_152_000, 631_152_000 + 60).is_fresh());
    }

    #[test]
    fn shiro_matching() {
        assert!(shiro_match("*", "api:treatments:create"));
        assert!(shiro_match("*:*:read", "api:entries:read"));
        assert!(!shiro_match("*:*:read", "api:entries:create"));
        assert!(shiro_match(
            "api:treatments:create",
            "api:treatments:create"
        ));
        assert!(shiro_match("api:pebble,entries:read", "api:entries:read"));
        assert!(!shiro_match("api:treatments", "api"));
        assert!(shiro_match("api", "api:treatments:read"));
    }
}
