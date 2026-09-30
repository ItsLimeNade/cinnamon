//! Follows glucose and alarms live over socket.io.
//!
//! ```sh
//! NS_URL=https://my-ns.example.com NS_TOKEN=myapp-0123456789abcdef \
//!   cargo run --example realtime_alarms --features realtime
//! ```
#![allow(clippy::print_stdout)]

use std::env;

use cinnamon::realtime::AlarmEvent;
use cinnamon::{Client, Credentials};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("NS_URL")?;
    let token = env::var("NS_TOKEN")?;
    let ns = Client::connect(&url, Credentials::access_token(token)).await?;

    let mut glucose = ns.realtime().glucose().await?;
    let mut alarms = ns.realtime().alarms().await?;
    println!("listening… (Ctrl-C to stop)");

    loop {
        tokio::select! {
            Some(reading) = glucose.next() => {
                let reading = reading?;
                println!(
                    "BG {} {}",
                    reading.mgdl.unwrap_or_default(),
                    reading.direction.as_deref().unwrap_or("")
                );
            }
            Some(event) = alarms.next() => match event? {
                AlarmEvent::Urgent(a) | AlarmEvent::Alarm(a) => {
                    println!("ALARM {}: {}", a.title.unwrap_or_default(), a.message.unwrap_or_default());
                }
                AlarmEvent::Cleared(_) => println!("alarm cleared"),
                AlarmEvent::Reconnected => println!("reconnected (run a sync to catch up)"),
                _ => {}
            },
            else => break,
        }
    }
    Ok(())
}
