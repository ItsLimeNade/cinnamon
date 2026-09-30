//! Prints the latest glucose, the trend and today's readings.
//!
//! ```sh
//! NS_URL=https://my-ns.example.com NS_TOKEN=myapp-0123456789abcdef cargo run --example read_bg
//! ```
#![allow(clippy::print_stdout)]

use std::env;

use chrono::{Duration, Utc};
use cinnamon::model::Units;
use cinnamon::{Client, Credentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("NS_URL")?;
    let token = env::var("NS_TOKEN")?;
    let ns = Client::connect(&url, Credentials::access_token(token)).await?;

    let units = ns
        .server()
        .status()
        .await?
        .settings
        .and_then(|s| s.display_units())
        .unwrap_or(Units::MgDl);

    match ns.entries().sgv().latest().await? {
        Some(bg) => {
            let arrow = bg.direction.as_ref().map_or("", |d| d.arrow());
            let minutes = (Utc::now() - bg.date.to_datetime()).num_minutes();
            println!(
                "{} {units} {arrow} ({minutes} min ago)",
                bg.sgv.in_units(units)
            );
        }
        None => println!("no readings yet"),
    }

    let today = ns
        .entries()
        .sgv()
        .list()
        .since(Utc::now() - Duration::hours(24))
        .limit(288)
        .await?;
    let in_range = today
        .iter()
        .filter(|s| (70.0..=180.0).contains(&s.sgv.as_mgdl()))
        .count();
    if !today.is_empty() {
        println!(
            "{} readings in 24 h, {:.0}% in 70–180 mg/dL",
            today.len(),
            100.0 * in_range as f64 / today.len() as f64
        );
    }
    Ok(())
}
