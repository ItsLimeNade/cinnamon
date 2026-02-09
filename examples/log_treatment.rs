use chrono::Utc;
use cinnamon::client::NightscoutClient;
use cinnamon::models::treatments::Treatment;
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = env::var("NS_URL").expect("NS_URL not set");
    let token = env::var("NS_TOKEN").expect("NS_TOKEN not set"); // Token IS required for writing
    let client = NightscoutClient::new(&url)?
    .with_secret(token);

    let snack = Treatment {
        id: None,
        event_type: "Carb Correction".to_string(),
        created_at: Utc::now().to_rfc3339(),
        carbs: Some(15.0),
        notes: Some("Mid-afternoon snack via Cinnamon".to_string()),
        entered_by: Some("Cinnamon-Rust".to_string()),
        // Fill other fields with None
        glucose: None,
        glucose_type: None,
        insulin: None,
        units: None,
    };

    println!("Uploading treatment.");

    // Send it
    let result = client.treatments().create(vec![snack]).await?;

    println!("Success! Created treatment with ID: {:?}", result[0].id);

    Ok(())
}
