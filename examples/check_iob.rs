//! Shows insulin and carbs on board, the loop state and device ages from the server's
//! computed properties.
//!
//! ```sh
//! NS_URL=https://my-ns.example.com NS_TOKEN=myapp-0123456789abcdef cargo run --example check_iob
//! ```
#![allow(clippy::print_stdout)]

use std::env;

use cinnamon::model::properties::Property;
use cinnamon::{Client, Credentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("NS_URL")?;
    let token = env::var("NS_TOKEN")?;
    let ns = Client::connect(&url, Credentials::access_token(token)).await?;

    let props = ns
        .properties()
        .only([
            Property::Iob,
            Property::Cob,
            Property::Basal,
            Property::Loop,
            Property::OpenAps,
            Property::Cage,
            Property::Sage,
        ])
        .await?;

    if let Some(iob) = props.iob.and_then(|i| i.iob) {
        println!("IOB    {iob:.2} U");
    }
    if let Some(cob) = props.cob.and_then(|c| c.cob) {
        println!("COB    {cob:.0} g");
    }
    if let Some(basal) = props.basal.and_then(|b| b.display) {
        println!("Basal  {basal}");
    }
    let status = props
        .loop_status
        .or(props.openaps)
        .and_then(|l| l.status)
        .and_then(|s| s.label.or(s.code));
    if let Some(status) = status {
        println!("Loop   {status}");
    }
    if let Some(cage) = props.cage.and_then(|c| c.display) {
        println!("Site   {cage}");
    }
    if let Some(sage) = props
        .sage
        .as_ref()
        .and_then(|s| s.current())
        .and_then(|a| a.display.clone())
    {
        println!("Sensor {sage}");
    }
    Ok(())
}
