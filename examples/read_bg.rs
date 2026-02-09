use cinnamon::client::NightscoutClient;
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize (Read-only doesn't strictly need a token, but good practice)
    let url = env::var("NS_URL").expect("NS_URL not set");
    let token = env::var("NS_TOKEN").expect("NS_TOKEN not set");
    let client = NightscoutClient::new(&url)?
    .with_secret(token);

    println!("Fetching latest glucose data.");

    // Fetch last 5 SGV entries
    let entries = client
        .sgv()
        .get()
        .limit(5)
        .send()
        .await?;

    for entry in entries {
        println!(
            "[{}] {} mg/dl ({:?})",
            entry.date_string, entry.sgv, entry.direction
        );
    }

    Ok(())
}
