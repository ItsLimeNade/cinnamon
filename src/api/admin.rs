//! Authorization administration: subjects, roles and permissions (admin token required).

use serde_json::Value;

use crate::client::Client;
use crate::client::transport::{Request, encode_segment};
use crate::error::{Error, Result};
use crate::model::admin::{Role, Subject};

/// Authorization administration: `ns.admin()`.
#[derive(Debug, Clone)]
pub struct Admin {
    client: Client,
}

impl Admin {
    pub(crate) const fn new(client: Client) -> Self {
        Self { client }
    }

    /// All subjects (token owners), including their access tokens.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:subjects:read`.
    pub async fn subjects(&self) -> Result<Vec<Subject>> {
        self.client
            .inner
            .execute(Request::get(
                "api/v2/authorization/subjects",
                "api/v2/authorization/subjects",
            ))
            .await?
            .json()
    }

    /// Creates a subject; Nightscout generates its access token (read it back with
    /// [`Admin::subjects`]).
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:subjects:create`.
    pub async fn create_subject(&self, subject: &Subject) -> Result<()> {
        self.client
            .inner
            .execute(
                Request::post(
                    "api/v2/authorization/subjects",
                    "api/v2/authorization/subjects",
                )
                .json(subject)?,
            )
            .await
            .map(|_| ())
    }

    /// Replaces a subject (matched by `_id`).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] without an `_id`, [`Error::Unauthorized`] without permission.
    pub async fn update_subject(&self, subject: &Subject) -> Result<()> {
        if subject.id.is_none() {
            return Err(Error::InvalidInput("the subject has no _id".into()));
        }
        self.client
            .inner
            .execute(
                Request::put(
                    "api/v2/authorization/subjects",
                    "api/v2/authorization/subjects",
                )
                .json(subject)?,
            )
            .await
            .map(|_| ())
    }

    /// Deletes a subject, revoking its token.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:subjects:delete`.
    pub async fn delete_subject(&self, id: &str) -> Result<()> {
        self.client
            .inner
            .execute(Request::delete(
                format!("api/v2/authorization/subjects/{}", encode_segment(id)),
                "api/v2/authorization/subjects/{id}",
            ))
            .await
            .map(|_| ())
    }

    /// All roles: the stored ones plus Nightscout's built-ins (`admin`, `readable`,
    /// `careportal`, …).
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:roles:list`.
    pub async fn roles(&self) -> Result<Vec<Role>> {
        self.client
            .inner
            .execute(Request::get(
                "api/v2/authorization/roles",
                "api/v2/authorization/roles",
            ))
            .await?
            .json()
    }

    /// Creates a role.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:roles:create`.
    pub async fn create_role(&self, role: &Role) -> Result<()> {
        self.client
            .inner
            .execute(
                Request::post("api/v2/authorization/roles", "api/v2/authorization/roles")
                    .json(role)?,
            )
            .await
            .map(|_| ())
    }

    /// Replaces a role (matched by `_id`).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] without an `_id`, [`Error::Unauthorized`] without permission.
    pub async fn update_role(&self, role: &Role) -> Result<()> {
        if role.id.is_none() {
            return Err(Error::InvalidInput("the role has no _id".into()));
        }
        self.client
            .inner
            .execute(
                Request::put("api/v2/authorization/roles", "api/v2/authorization/roles")
                    .json(role)?,
            )
            .await
            .map(|_| ())
    }

    /// Deletes a role.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:roles:delete`.
    pub async fn delete_role(&self, id: &str) -> Result<()> {
        self.client
            .inner
            .execute(Request::delete(
                format!("api/v2/authorization/roles/{}", encode_segment(id)),
                "api/v2/authorization/roles/{id}",
            ))
            .await
            .map(|_| ())
    }

    /// Every permission string the server has seen checked since it started.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] without `admin:api:permissions:read`.
    pub async fn permissions(&self) -> Result<Vec<String>> {
        let value: Value = self
            .client
            .inner
            .execute(Request::get(
                "api/v2/authorization/permissions",
                "api/v2/authorization/permissions",
            ))
            .await?
            .json()?;
        Ok(match value {
            Value::Array(items) => items
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            Value::Object(map) => map.keys().cloned().collect(),
            _ => Vec::new(),
        })
    }

    /// Whether the current credentials hold `permission` (e.g. `api:treatments:create`).
    ///
    /// A `false` answer counts as a failed authentication on the server, which briefly
    /// delays later requests from this IP.
    ///
    /// # Errors
    ///
    /// Transport errors.
    pub async fn check(&self, permission: &str) -> Result<bool> {
        let req = Request::get(
            format!(
                "api/v2/authorization/debug/check/{}",
                encode_segment(permission)
            ),
            "api/v2/authorization/debug/check/{permission}",
        );
        match self.client.inner.execute(req).await {
            Ok(_) => Ok(true),
            Err(Error::Unauthorized { .. } | Error::Forbidden { .. }) => Ok(false),
            Err(err) => Err(err),
        }
    }
}
