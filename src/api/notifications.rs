//! Alarm acknowledgements and Loop remote commands.

use std::time::Duration;

use crate::client::Client;
use crate::client::transport::Request;
use crate::error::Result;
use crate::model::notification::{Level, LoopNotification};

/// Notification endpoints: `ns.notifications()`.
#[derive(Debug, Clone)]
pub struct Notifications {
    client: Client,
}

impl Notifications {
    pub(crate) const fn new(client: Client) -> Self {
        Self { client }
    }

    /// Silences alarms of `level` (and lower) in `group` for `silence`
    /// (`GET /api/v1/notifications/ack`). Nightscout's default group is `"default"`.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`](crate::Error::Unauthorized) without the
    /// `notifications:*:ack` permission.
    pub async fn ack(&self, level: Level, group: &str, silence: Duration) -> Result<()> {
        let millis = u64::try_from(silence.as_millis()).unwrap_or(u64::MAX);
        self.client
            .inner
            .execute(
                Request::get("api/v1/notifications/ack", "api/v1/notifications/ack")
                    .query("level", level.as_i8().to_string())
                    .query("group", group)
                    .query("time", millis.to_string()),
            )
            .await
            .map(|_| ())
    }

    /// Sends a remote command to Loop through Apple push (`POST /api/v2/notifications/loop`).
    /// Requires Loop's APNs settings on the server and, by default, an admin token.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`](crate::Error::Unauthorized) without
    /// `notifications:loop:push`, [`Error::Server`](crate::Error::Server) when the push fails.
    pub async fn push_to_loop(&self, notification: &LoopNotification) -> Result<()> {
        self.client
            .inner
            .execute(
                Request::post("api/v2/notifications/loop", "api/v2/notifications/loop")
                    .json(notification)?,
            )
            .await
            .map(|_| ())
    }
}
