# cinnamon
A type-safe Nightscout client for Rust, aiming to simplify the interactions with the confusing Nightscout API.


## Basic usage:
### Installation:
Add this to your `Cargo.toml`
```TOML
[dependencies]
cinnamon = "0.0.1"
tokio = { version = "1", features = ["full"] }
chrono = "0.4"
```

### Quick start:

```rs
use nightscout_rs::NightscoutClient;
use chrono::{Duration, Utc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the client
    let url = "https://www.your-nightscout-url.com";
    let token = Some("your-api-secret".to_string());
    
    let client = NightscoutClient::new(url, token).expect("Invalid URL");

    // Fetch SGV Data
    // Get the last 10 entries from the last 24 hours
    println!("Fetching SGV entries...");
    let entries = client.sgv()
        .from(Utc::now() - Duration::hours(24))
        .limit(10)
        .await?;

    for entry in entries {
        println!("-> {} mg/dl at {}", entry.sgv, entry.date_string);
    }

    // Fetch IOB
    println!("\nFetching IOB...");
    let iob_data = client.iob().await?;
    println!("-> Current IOB: {} U ({})", iob_data.iob, iob_data.display_line);

    Ok(())
}

```

## Important Disclaimer
### This software is a Work In Progress (WIP) and is not production-ready.

**NO MEDICAL ADVICE**: This library is for educational and informational purposes only. It is not intended to be relied upon for medical decisions, insulin dosing, or treatment adjustments. Always consult your primary glucose monitoring device (CGM/BGM) and official medical equipment before taking any action. 

**If you have any concerns regarding your health or diabetes management, please contact your healthcare provider immediately.**

**NO WARRANTY**: This software is provided "as is", without warranty of any kind. The data retrieved may be inaccurate, delayed, or incomplete. The authors and contributors explicitly disclaim any liability for any direct or indirect damage or health consequences resulting from the use of this code.