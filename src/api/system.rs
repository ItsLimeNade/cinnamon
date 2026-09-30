//! Server-level endpoints: status, versions, permissions checks and admin notifications.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::client::transport::Request;
use crate::client::{Client, ServerInfo, Support};
use crate::error::Result;
use crate::model::system::{
    AdminNotifies, ApiVersionInfo, LastModified, Status, V3Status, V3Version, VerifyAuth,
};

/// Server-level endpoints: `ns.server()`.
#[derive(Debug, Clone)]
pub struct Server {
    client: Client,
}

/// `{"status": 200, "message": {...}}`, the envelope of `verifyauth` and `adminnotifies`.
#[derive(Deserialize)]
struct MessageEnvelope<T> {
    message: T,
}

impl Server {
    pub(crate) const fn new(client: Client) -> Self {
        Self { client }
    }

    async fn get<T: DeserializeOwned>(&self, path: &'static str) -> Result<T> {
        self.client
            .inner
            .execute(Request::get(path, path))
            .await?
            .json()
    }

    /// Discovered capabilities (cached after the first call).
    ///
    /// # Errors
    ///
    /// Transport errors while probing.
    pub async fn info(&self) -> Result<ServerInfo> {
        self.client.server_info().await
    }

    /// `GET /api/v1/status.json`: version, enabled features and public settings.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`](crate::Error::Unauthorized) when anonymous access is
    /// disabled and no credentials were given.
    pub async fn status(&self) -> Result<Status> {
        self.get("api/v1/status.json").await
    }

    /// `GET /api/versions`: the API generations the server mounts.
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn versions(&self) -> Result<Vec<ApiVersionInfo>> {
        self.client
            .inner
            .execute(Request::get("api/versions", "api/versions").anonymous())
            .await?
            .json()
    }

    /// `GET /api/v3/version` (public).
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`](crate::Error::NotFound) on servers without API v3.
    pub async fn v3_version(&self) -> Result<V3Version> {
        self.client
            .inner
            .execute(Request::get("api/v3/version", "api/v3/version").anonymous())
            .await?
            .v3()
    }

    /// `GET /api/v3/status`: version plus the caller's permissions per collection.
    ///
    /// # Errors
    ///
    /// [`Error::Unsupported`](crate::Error::Unsupported) without API v3 or an access token.
    pub async fn v3_status(&self) -> Result<V3Status> {
        self.client
            .inner
            .pick_api(Support::V3Only, "API v3 status")
            .await?;
        self.client
            .inner
            .execute(Request::get("api/v3/status", "api/v3/status"))
            .await?
            .v3()
    }

    /// `GET /api/v3/lastModified`: newest change per collection, the cheap poll that
    /// drives incremental sync.
    ///
    /// # Errors
    ///
    /// [`Error::Unsupported`](crate::Error::Unsupported) without API v3 or an access token.
    pub async fn last_modified(&self) -> Result<LastModified> {
        self.client
            .inner
            .pick_api(Support::V3Only, "lastModified")
            .await?;
        self.client
            .inner
            .execute(Request::get("api/v3/lastModified", "api/v3/lastModified"))
            .await?
            .v3()
    }

    /// `GET /api/v1/verifyauth`: what the current credentials may do. Never fails with 401;
    /// check [`VerifyAuth::message`].
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn verify_auth(&self) -> Result<VerifyAuth> {
        let env: MessageEnvelope<VerifyAuth> = self.get("api/v1/verifyauth").await?;
        Ok(env.message)
    }

    /// `GET /api/v1/adminnotifies`: messages shown to administrators (e.g. failed
    /// authentication attempts).
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn admin_notifies(&self) -> Result<AdminNotifies> {
        let env: MessageEnvelope<AdminNotifies> = self.get("api/v1/adminnotifies").await?;
        Ok(env.message)
    }
}
