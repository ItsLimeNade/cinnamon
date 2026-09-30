//! Authorization administration: subjects (token owners) and roles.

use serde::{Deserialize, Serialize};

use super::{Extra, de};

/// A subject: a named owner of an access token with a set of roles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Subject {
    /// Document id.
    #[serde(
        rename = "_id",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub id: Option<String>,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Role names (e.g. `readable`, `careportal`, `admin`).
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub roles: Vec<String>,
    /// Free-text notes.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub notes: Option<String>,
    /// The subject's access token (returned by the server; never sent back).
    #[serde(rename = "accessToken", default, skip_serializing)]
    pub access_token: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Subject {
    /// A new subject named `name` with `roles`.
    #[must_use]
    pub fn new<I, S>(name: impl Into<String>, roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            roles: roles.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }
}

/// A role: a named set of Shiro-style permissions (`api:treatments:create`, `*:*:read`, …).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Role {
    /// Document id (absent for built-in roles).
    #[serde(
        rename = "_id",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub id: Option<String>,
    /// Role name.
    #[serde(default)]
    pub name: String,
    /// Granted permissions.
    #[serde(default, deserialize_with = "de::null_as_empty")]
    pub permissions: Vec<String>,
    /// Free-text notes.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub notes: Option<String>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Role {
    /// A new role named `name` with `permissions`.
    #[must_use]
    pub fn new<I, S>(name: impl Into<String>, permissions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            permissions: permissions.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }
}
