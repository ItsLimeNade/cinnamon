//! The `food` collection: the food database and quick picks.

use serde::{Deserialize, Serialize};

use super::{CollectionName, Extra, Meta, Timestamp, de, impl_document, time};

/// A food item (`type: "food"`) or a quick pick (`type: "quickpick"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Food {
    /// Bookkeeping fields.
    #[serde(flatten)]
    pub meta: Meta,
    /// `food` or `quickpick`.
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "Option::is_none",
        with = "de::text"
    )]
    pub food_type: Option<String>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub name: Option<String>,
    /// Category.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub category: Option<String>,
    /// Subcategory.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub subcategory: Option<String>,
    /// Portion size.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub portion: Option<f64>,
    /// Portion unit (`g`, `ml`, `pcs`, …).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::text")]
    pub unit: Option<String>,
    /// Carbs per portion, in grams.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub carbs: Option<f64>,
    /// Fat per portion, in grams.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub fat: Option<f64>,
    /// Protein per portion, in grams.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub protein: Option<f64>,
    /// Energy per portion, in kJ.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub energy: Option<f64>,
    /// Glycemic index class (1 = low … 3 = high).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub gi: Option<f64>,
    /// Hidden from the quick-pick list (Nightscout stores this as a string).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::flag")]
    pub hidden: Option<bool>,
    /// Hide the quick pick after it was used once.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::flag")]
    pub hideafteruse: Option<bool>,
    /// Position in the quick-pick list.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "de::num")]
    pub position: Option<f64>,
    /// Foods making up a quick pick.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "de::null_as_empty"
    )]
    pub foods: Vec<serde_json::Value>,
    /// When the item was created.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "time::iso")]
    pub created_at: Option<Timestamp>,
    /// Creation time as epoch millis (API v3).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::millis"
    )]
    pub date: Option<Timestamp>,
    /// Fields cinnamon does not model.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Food {
    /// A food-database item with `carbs` grams per `portion` `unit`.
    #[must_use]
    pub fn new(name: impl Into<String>, carbs: f64, portion: f64, unit: impl Into<String>) -> Self {
        Self {
            meta: Meta::default(),
            food_type: Some("food".into()),
            name: Some(name.into()),
            category: None,
            subcategory: None,
            portion: Some(portion),
            unit: Some(unit.into()),
            carbs: Some(carbs),
            fat: None,
            protein: None,
            energy: None,
            gi: None,
            hidden: None,
            hideafteruse: None,
            position: None,
            foods: Vec::new(),
            created_at: Some(Timestamp::now()),
            date: None,
            extra: Extra::new(),
        }
    }

    fn fill_defaults(&mut self) {
        let t = self.created_at.or(self.date).unwrap_or_else(Timestamp::now);
        self.created_at.get_or_insert(t);
        self.date.get_or_insert(t);
    }
}

impl_document!(
    Food,
    CollectionName::Food,
    timestamp = |s| s.created_at.or(s.date),
    device = |_s| None
);
