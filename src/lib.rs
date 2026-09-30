#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod api;
#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub mod blocking;
pub mod client;
pub mod error;
pub mod model;
pub mod query;
#[cfg(feature = "realtime")]
#[cfg_attr(docsrs, doc(cfg(feature = "realtime")))]
pub mod realtime;
pub mod sync;

pub use client::{
    ApiVersion, Client, ClientBuilder, Credentials, RetryPolicy, ServerInfo, Session,
};
pub use error::{Error, Result};

/// The most common imports: `use cinnamon::prelude::*;`.
pub mod prelude {
    pub use crate::api::{DeleteFilter, List};
    pub use crate::model::{
        Direction, Document, Entry, EventType, Glucose, Profile, ProfileStore, Sgv, Timestamp,
        Treatment, TreatmentKind, Units,
    };
    pub use crate::query::Filter;
    pub use crate::sync::{SyncCursor, SyncEvent};
    pub use crate::{ApiVersion, Client, Credentials, Error, Result};
}
