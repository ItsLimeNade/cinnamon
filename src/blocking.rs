//! A synchronous facade over the async [`Client`](crate::Client) (feature `blocking`).
//!
//! Every async request builder can be run to completion with [`Client::run`]; the async API
//! stays the single source of truth, so nothing is duplicated or missing.
//!
//! ```no_run
//! use cinnamon::blocking::Client;
//! use cinnamon::Credentials;
//!
//! let ns = Client::connect("https://my-ns.example.com", Credentials::access_token("app-0123456789abcdef"))?;
//! let latest = ns.run(ns.entries().sgv().latest())?;
//! let week = ns.run(ns.treatments().list().limit(100))?;
//! # Ok::<(), cinnamon::Error>(())
//! ```
//!
//! Do not use it from inside an async runtime: the wrapper owns a runtime of its own and
//! `run` panics when called from async code.

use std::future::{Future, IntoFuture};
use std::ops::Deref;
use std::sync::Arc;

use crate::client::{ClientBuilder, Credentials};
use crate::error::{Error, Result};

/// A blocking Nightscout client: the async [`crate::Client`] plus a private runtime.
///
/// Dereferences to the async client, so every builder is available; pass the builder (or
/// any future) to [`Client::run`].
#[derive(Clone, Debug)]
pub struct Client {
    inner: crate::Client,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Client {
    /// Builds a client and verifies the credentials (see [`crate::Client::connect`]).
    ///
    /// # Errors
    ///
    /// Same as [`crate::Client::connect`], plus [`Error::Transport`] if the runtime cannot
    /// start.
    pub fn connect(url: impl AsRef<str>, credentials: Credentials) -> Result<Self> {
        Self::from_builder(crate::Client::builder(url).credentials(credentials))
    }

    /// Finishes a [`ClientBuilder`] (running discovery and credential checks).
    ///
    /// # Errors
    ///
    /// Same as [`ClientBuilder::connect`].
    pub fn from_builder(builder: ClientBuilder) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::Transport(Box::new(e)))?;
        let inner = runtime.block_on(builder.connect())?;
        Ok(Self {
            inner,
            runtime: Arc::new(runtime),
        })
    }

    /// Runs an async request builder or future to completion.
    ///
    /// # Panics
    ///
    /// When called from within an async runtime.
    pub fn run<F: IntoFuture>(&self, request: F) -> F::Output {
        self.runtime.block_on(request.into_future())
    }

    /// Runs any future to completion (alias of [`Client::run`] for plain futures).
    ///
    /// # Panics
    ///
    /// When called from within an async runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    /// The underlying async client.
    #[must_use]
    pub const fn as_async(&self) -> &crate::Client {
        &self.inner
    }
}

impl Deref for Client {
    type Target = crate::Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
