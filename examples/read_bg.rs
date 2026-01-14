use cinnamon::client::NightscoutClient;
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize (Read-only doesn't strictly need a token, but good practice)
    let url = env::var("NS_URL").expect("NS_URL not set");
    let token = env::var("NS_TOKEN").ok();
    let client = NightscoutClient::new(&url, token)?;

    println!("Fetching latest glucose data.");

    // Fetch last 5 SGV entries
    let entries = client.entries().sgv().list().limit(5).await?;

    for entry in entries {
        println!(
            "[{}] {} mg/dl ({:?})",
            entry.date_string, entry.sgv, entry.direction
        );
    }

    Ok(())
}