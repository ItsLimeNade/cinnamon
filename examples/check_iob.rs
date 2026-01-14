use cinnamon::client::NightscoutClient;
use cinnamon::models::properties::PropertyType;
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = env::var("NS_URL").expect("NS_URL not set");
    let client = NightscoutClient::new(&url, None)?;

    // Request specific properties (faster than fetching everything)
    let stats = client
        .properties()
        .get()
        .only(&[PropertyType::Iob, PropertyType::Cob])
        .send()
        .await?;

    if let Some(iob) = stats.iob {
        println!("IOB: {} U", iob.iob);
    }

    if let Some(cob) = stats.cob {
        println!("COB: {} g", cob.cob);
    }

    Ok(())
}
