//! Request builders for every Nightscout endpoint, reached through [`Client`].

mod admin;
pub(crate) mod collection;
mod entries;
mod notifications;
mod properties;
mod system;

pub use admin::Admin;
pub use collection::{
    Collection, Created, Delete, DeleteFilter, DeleteMany, DeleteReport, HistoryPage, List,
};
pub use entries::Entries;
pub use notifications::Notifications;
pub use properties::PropertiesRequest;
pub use system::Server;

use crate::client::Client;
use crate::error::Result;
use crate::model::ddata::DData;
use crate::model::summary::Summary;
use crate::model::{Activity, DeviceStatus, Food, ProfileStore, Setting, Timestamp, Treatment};
use crate::query::Filter;

impl Client {
    /// Glucose entries (`sgv`, `mbg`, `cal`).
    #[must_use]
    pub fn entries(&self) -> Entries {
        Entries::new(self.clone())
    }

    /// Treatments (boluses, carbs, temp basals, targets, site changes, …).
    #[must_use]
    pub fn treatments(&self) -> Collection<Treatment> {
        Collection::new(self.clone(), Filter::new())
    }

    /// Uploader, pump and closed-loop status reports.
    #[must_use]
    pub fn devicestatus(&self) -> Collection<DeviceStatus> {
        Collection::new(self.clone(), Filter::new())
    }

    /// Profile documents; see also [`Collection::current`].
    #[must_use]
    pub fn profiles(&self) -> Collection<ProfileStore> {
        Collection::new(self.clone(), Filter::new())
    }

    /// The food database and quick picks; see also [`Collection::quickpicks`].
    #[must_use]
    pub fn food(&self) -> Collection<Food> {
        Collection::new(self.clone(), Filter::new())
    }

    /// Activity records (API v1 only).
    #[must_use]
    pub fn activity(&self) -> Collection<Activity> {
        Collection::new(self.clone(), Filter::new())
    }

    /// Per-application settings documents (API v3 only; needs `api:settings:admin` to read).
    #[must_use]
    pub fn settings(&self) -> Collection<Setting> {
        Collection::new(self.clone(), Filter::new())
    }

    /// Live plugin state (IOB, COB, delta, device ages, pump and loop status).
    pub fn properties(&self) -> PropertiesRequest {
        PropertiesRequest::new(self.clone())
    }

    /// The server's in-memory data window (`/api/v2/ddata/at`), now or as of `at`.
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn ddata(&self, at: Option<Timestamp>) -> Result<DData> {
        properties::ddata(self, at).await
    }

    /// A compact summary of the last `hours` hours (`/api/v2/summary`).
    ///
    /// # Errors
    ///
    /// Transport, authorization and decoding errors.
    pub async fn summary(&self, hours: u32) -> Result<Summary> {
        properties::summary(self, hours).await
    }

    /// Server-level endpoints (status, versions, permissions checks).
    #[must_use]
    pub fn server(&self) -> Server {
        Server::new(self.clone())
    }

    /// Alarm acknowledgements and Loop remote commands.
    #[must_use]
    pub fn notifications(&self) -> Notifications {
        Notifications::new(self.clone())
    }

    /// Subjects, roles and permissions administration.
    #[must_use]
    pub fn admin(&self) -> Admin {
        Admin::new(self.clone())
    }
}
