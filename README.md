# cinnamon

A type-safe, asynchronous Rust client for the Nightscout API (v1 & v2).

Cinnamon aims to simplify interactions with Nightscout by providing strongly-typed structs for entries, treatments, profiles, and device status, handling authentication and error propagation automatically.

## Installation

Add this to your Cargo.toml:

```toml
[dependencies]
cinnamon = "0.1.0"
tokio = { version = "1", features = ["full"] }
chrono = "0.4"

```

## Usage

### Setup the Client

Initialize the client with your Nightscout URL and optional API Secret.

```rust
use cinnamon::client::NightscoutClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://my-cgm.herokuapp.com";
    let token = Some("my-api-secret-123".to_string());

    let client = NightscoutClient::new(url, token)?;
    
    println!("Successfully connected to {}", client.base_url);
    Ok(())
}

```

### Fetching Glucose Data (SGV)

Retrieve the latest Sensor Glucose Value (SGV) or a list of historical entries.

```rust
use cinnamon::models::entries::SgvEntry;

// ... inside main ...

// Get the single latest entry
match client.entries().sgv().latest().await {
    Ok(entry) => {
        println!("Latest BG: {} mg/dl", entry.sgv);
        println!("Time: {}", entry.date_string);
        println!("Trend: {:?}", entry.direction);
    },
    Err(e) => eprintln!("Error fetching SGV: {}", e),
}

// Fetch the last 10 entries
let history = client.entries().sgv().list().limit(10).await?;
for entry in history {
    println!("[{}] {}", entry.date_string, entry.sgv);
}

```

### Fetching System State (Properties)

Use the V2 Properties API to check system states like Insulin On Board (IOB), Carbs On Board (COB), or Pump Status.

```rust
use cinnamon::models::properties::PropertyType;

// ... inside main ...

let stats = client.properties()
    .get()
    .only(&[PropertyType::Iob, PropertyType::Cob, PropertyType::Pump])
    .send()
    .await?;

if let Some(iob_data) = stats.iob {
    println!("IOB: {} U (Source: {})", iob_data.iob, iob_data.source);
}

if let Some(cob_data) = stats.cob {
    println!("COB: {} g", cob_data.cob);
}

```

### Uploading Treatments

Upload insulin boluses, carb corrections, or other care events to Nightscout.

```rust
use cinnamon::models::treatments::Treatment;
use chrono::Utc;

// ... inside main ...

let correction = Treatment {
    id: None,
    event_type: "Correction Bolus".to_string(),
    created_at: Utc::now().to_rfc3339(),
    insulin: Some(2.5),
    notes: Some("Correction for high BG".to_string()),
    entered_by: Some("Cinnamon-Rust".to_string()),
    // Fill unused fields with None
    glucose: None, glucose_type: None, carbs: None, units: None,
};

match client.treatments().create(vec![correction]).await {
    Ok(_) => println!("Treatment uploaded successfully."),
    Err(e) => eprintln!("Failed to upload: {}", e),
}

```

## Disclaimer

NO MEDICAL ADVICE: This library is for educational and informational purposes only. It is not intended to be relied upon for medical decisions, insulin dosing, or treatment adjustments. Always consult with a qualified healthcare professional.

**If you have any concerns regarding your health or diabetes management, please contact your healthcare provider immediately.**

**NO WARRANTY**: This software is provided "as is", without warranty of any kind. The data retrieved may be inaccurate, delayed, or incomplete. The authors and contributors explicitly disclaim any liability for any direct or indirect damage or health consequences resulting from the use of this code.
