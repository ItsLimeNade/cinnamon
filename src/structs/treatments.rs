use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct IobWrapper {
    pub iob: IobData,
}

/// IOB (Insulin On Board)
/// This struct represents the current ammount of insulin inside the user's system.
#[derive(Debug, Serialize, Deserialize)]
pub struct IobData {
    pub iob: f64,
    #[serde(rename = "displayLine")]
    pub display_line: String,
}
