use serde::{Deserialize, Serialize};
use reqwest::Method;

use crate::client::NightscoutClient;
use crate::query_builder::QueryBuilder;
use crate::endpoints::Endpoint;
use crate::error::NightscoutError;


#[derive(Debug, Deserialize)]
pub struct IobWrapper {
    pub iob: IobData,
}

/// IOB (Insulin On Board)
/// This struct represents the current amount of insulin inside the user's system.
#[derive(Debug, Serialize, Deserialize)]
pub struct IobData {
    pub iob: f64,
    #[serde(rename = "displayLine")]
    pub display_line: String,
}

/// Treatment
/// Represents a care event (bolus, carb correction, temp basal, etc.)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Treatment {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    
    #[serde(rename = "eventType")]
    pub event_type: String,

    #[serde(rename = "created_at")]
    pub created_at: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub glucose: Option<f64>,

    #[serde(rename = "glucoseType", skip_serializing_if = "Option::is_none")]
    pub glucose_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub carbs: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub insulin: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(rename = "enteredBy", skip_serializing_if = "Option::is_none")]
    pub entered_by: Option<String>,
}
pub struct TreatmentsService {
    pub client: NightscoutClient,
}

impl TreatmentsService {
    /// Returns a query builder to list treatments
    pub fn list(&self) -> QueryBuilder<Treatment> {
        QueryBuilder::<Treatment>::new(self.client.clone(), Endpoint::Treatments, Method::GET)
    }

    /// Returns a query builder to delete treatments
    pub fn delete(&self) -> QueryBuilder<Treatment> {
        QueryBuilder::<Treatment>::new(self.client.clone(), Endpoint::Treatments, Method::DELETE)
    }

    /// Creates new treatments
    pub async fn create(&self, treatments: Vec<Treatment>) -> Result<Vec<Treatment>, NightscoutError> {
        let url = self.client.base_url.join(Endpoint::Treatments.as_path())?;

        let mut request = self.client.http.post(url);
        request = self.client.auth(request);

        let response = self.client.send_checked(request.json(&treatments)).await?;

        Ok(response.json::<Vec<Treatment>>().await?)
    }
}