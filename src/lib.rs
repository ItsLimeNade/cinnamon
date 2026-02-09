//! # Cinnamon
//!
//! `cinnamon` is a type-safe, asynchronous Rust client for the Nightscout API (v1 & v2).
//!
//! It provides strongly-typed interfaces for interacting with Nightscout entries, treatments,
//! profiles, and device status updates. The library handles authentication, URL construction,
//! and error propagation, allowing you to focus on the data.
//!
//! ## Usage Pattern: Builder vs. Direct Fetch
//!
//! This library uses two distinct patterns depending on the complexity of the endpoint:
//!
//! 1.  **Query Builder Pattern**: For endpoints that support filtering, pagination, or large datasets (e.g., Entries, Treatments),
//!     the `.get()` method returns a `QueryBuilder`. You must chain methods to configure the query and finally call `.send().await` to execute it.
//! 2.  **Direct Fetch Pattern**: For simpler endpoints that return a single system state (e.g., Status, Profile),
//!     the `.get()` method is asynchronous and returns the data immediately. No `.send()` is required.
//!
//! ## Example
//!
//! ```rust,no_run
//! use cinnamon::client::NightscoutClient;
//! use cinnamon::models::properties::PropertyType;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = NightscoutClient::new("https://my-cgm.herokuapp.com")?
//!     .with_secret("my_secret");
//!
//!     // Pattern 1: Query Builder (needs .send())
//!     let entries = client.entries().sgv().get()
//!         .limit(5)
//!         .send()
//!         .await?;
//!
//!     // Pattern 2: Direct Fetch (returns data immediately)
//!     let status = client.status().get().await?;
//!
//!     Ok(())
//! }
//! ```
pub mod client;
pub mod endpoints;
pub mod error;
pub mod models;
pub mod query_builder;
