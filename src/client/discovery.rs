//! Server capability discovery and per-operation API selection.

use super::{ApiVersion, Inner, transport::Request};
use crate::error::{Error, Result};
use crate::model::system::V3Version;

/// The minimum API v3 version cinnamon speaks: 3.0.3 wraps responses in `{status, result}`
/// and no longer requires a `Date` header.
const MIN_V3: (u32, u32, u32) = (3, 0, 3);

/// What cinnamon learned about the server during discovery.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerInfo {
    /// Nightscout release, e.g. `15.0.8`, when the server reports it.
    pub version: Option<String>,
    /// API v3 version, e.g. `3.0.5`; `None` on servers without API v3.
    pub api_v3_version: Option<String>,
    /// Storage backend and version, e.g. `mongodb 7.0.14`.
    pub storage: Option<String>,
}

impl ServerInfo {
    /// Whether the server's API v3 is recent enough for cinnamon to use.
    #[must_use]
    pub fn supports_v3(&self) -> bool {
        self.api_v3_version
            .as_deref()
            .and_then(parse_semver)
            .is_some_and(|v| v >= MIN_V3)
    }
}

fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.trim().parse::<u32>().ok());
    Some((
        parts.next()??,
        parts.next().flatten().unwrap_or(0),
        parts.next().flatten().unwrap_or(0),
    ))
}

/// The wire API an operation is sent through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Api {
    V1,
    V3,
}

/// Which APIs implement an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Support {
    Both,
    V1Only,
    V3Only,
}

impl Inner {
    /// Discovers (once) and returns the server's capabilities.
    pub(crate) async fn server_info(&self) -> Result<&ServerInfo> {
        self.server.get_or_try_init(|| self.discover()).await
    }

    async fn discover(&self) -> Result<ServerInfo> {
        let probe = Request::get("api/v3/version", "api/v3/version").anonymous();
        match self.execute(probe).await {
            Ok(resp) => {
                let v: V3Version = resp.v3()?;
                Ok(ServerInfo {
                    version: v.version,
                    api_v3_version: v.api_version,
                    storage: v.storage.map(|s| {
                        [s.storage, s.version]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(" ")
                    }),
                })
            }
            // Pre-v3 Nightscout: ask the v1 status endpoint for the version (best effort).
            Err(Error::NotFound) => {
                let status = Request::get("api/v1/status.json", "api/v1/status.json");
                let version = match self.execute(status).await {
                    Ok(resp) => resp
                        .json::<serde_json::Value>()
                        .ok()
                        .and_then(|v| v.get("version")?.as_str().map(str::to_owned)),
                    Err(_) => None,
                };
                Ok(ServerInfo {
                    version,
                    ..ServerInfo::default()
                })
            }
            Err(err) => Err(err),
        }
    }

    /// Chooses the API for an operation, honoring the client's [`ApiVersion`] preference.
    pub(crate) async fn pick_api(&self, support: Support, feature: &'static str) -> Result<Api> {
        let unsupported = |reason: &str| Error::Unsupported {
            feature,
            reason: reason.to_owned(),
        };
        match support {
            Support::V1Only => Ok(Api::V1),
            Support::V3Only => {
                if self.credentials.is_api_secret() {
                    return Err(unsupported(
                        "API v3 does not accept API_SECRET; use an access token",
                    ));
                }
                if self.api == ApiVersion::V1 {
                    return Err(unsupported("the client is pinned to API v1"));
                }
                if self.server_info().await?.supports_v3() {
                    Ok(Api::V3)
                } else {
                    Err(unsupported(
                        "this Nightscout has no API v3 (upgrade to 14.1 or later)",
                    ))
                }
            }
            Support::Both => match self.api {
                ApiVersion::V1 => Ok(Api::V1),
                ApiVersion::V3 if self.credentials.is_api_secret() => Err(unsupported(
                    "API v3 does not accept API_SECRET; use an access token",
                )),
                ApiVersion::V3 => Ok(Api::V3),
                ApiVersion::Auto => {
                    if self.credentials.is_access_token() && self.server_info().await?.supports_v3()
                    {
                        Ok(Api::V3)
                    } else {
                        Ok(Api::V1)
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_version_gate() {
        let info = |v: &str| ServerInfo {
            api_v3_version: Some(v.into()),
            ..ServerInfo::default()
        };
        assert!(info("3.0.5").supports_v3());
        assert!(info("3.0.3").supports_v3());
        assert!(!info("3.0.2-alpha").supports_v3());
        assert!(info("3.1").supports_v3());
        assert!(!ServerInfo::default().supports_v3());
    }
}
