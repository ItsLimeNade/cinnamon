//! Logs a carb entry and reads it back. The token needs the `careportal` (or `admin`) role.
//!
//! ```sh
//! NS_URL=https://my-ns.example.com NS_TOKEN=myapp-0123456789abcdef cargo run --example log_treatment
//! ```
#![allow(clippy::print_stdout)]

use std::env;

use cinnamon::model::Treatment;
use cinnamon::{Client, Credentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("NS_URL")?;
    let token = env::var("NS_TOKEN")?;
    let ns = Client::builder(&url)
        .credentials(Credentials::access_token(token))
        .app_name("cinnamon-example")
        .connect()
        .await?;

    let snack = Treatment::carbs(15.0)
        .with_notes("Mid-afternoon snack")
        .with_entered_by("cinnamon-example");
    let created = ns.treatments().create(&snack).await?;
    println!(
        "stored as {:?}{}",
        created.identifier,
        if created.deduplicated {
            " (already existed)"
        } else {
            ""
        }
    );

    if let Some(id) = created.identifier {
        let stored = ns.treatments().get(&id).await?;
        println!("{:?}", stored.kind());
    }
    Ok(())
}
