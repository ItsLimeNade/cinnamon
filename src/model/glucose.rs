//! Glucose values and units.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Nightscout's mg/dL per mmol/L factor (`MMOL_TO_MGDL` in `lib/constants.json`).
pub const MMOL_TO_MGDL: f64 = 18.015_59;

/// Blood glucose display units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Units {
    /// Milligrams per deciliter.
    #[default]
    MgDl,
    /// Millimoles per liter.
    MmolL,
}

impl Units {
    /// Parses Nightscout's many spellings (`mg/dl`, `mg/dL`, `mgdl`, `mmol`, `mmol/L`, …).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.trim().to_ascii_lowercase();
        if lower.starts_with("mg") {
            Some(Self::MgDl)
        } else if lower.starts_with("mmol") {
            Some(Self::MmolL)
        } else {
            None
        }
    }

    /// The canonical Nightscout spelling (`mg/dl` or `mmol`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MgDl => "mg/dl",
            Self::MmolL => "mmol",
        }
    }
}

impl fmt::Display for Units {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MgDl => "mg/dL",
            Self::MmolL => "mmol/L",
        })
    }
}

/// A glucose value, stored in mg/dL as Nightscout does.
///
/// Serializes as an integer when the value is whole (so `120` round-trips as `120`, not
/// `120.0`), and deserializes from numbers or numeric strings.
#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Glucose(f64);

impl Glucose {
    /// From mg/dL.
    #[must_use]
    pub const fn mgdl(value: f64) -> Self {
        Self(value)
    }

    /// From mmol/L.
    #[must_use]
    pub fn mmol(value: f64) -> Self {
        Self(value * MMOL_TO_MGDL)
    }

    /// From a value in `units`.
    #[must_use]
    pub fn new(value: f64, units: Units) -> Self {
        match units {
            Units::MgDl => Self::mgdl(value),
            Units::MmolL => Self::mmol(value),
        }
    }

    /// The value in mg/dL.
    #[must_use]
    pub const fn as_mgdl(self) -> f64 {
        self.0
    }

    /// The value in mmol/L, rounded to one decimal like Nightscout displays it.
    #[must_use]
    pub fn as_mmol(self) -> f64 {
        (self.0 / MMOL_TO_MGDL * 10.0).round() / 10.0
    }

    /// The value in `units` (mmol/L rounded to one decimal).
    #[must_use]
    pub fn in_units(self, units: Units) -> f64 {
        match units {
            Units::MgDl => self.0,
            Units::MmolL => self.as_mmol(),
        }
    }
}

impl fmt::Debug for Glucose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Glucose({} mg/dL)", self.0)
    }
}

impl fmt::Display for Glucose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<f64> for Glucose {
    fn from(mgdl: f64) -> Self {
        Self(mgdl)
    }
}

impl Serialize for Glucose {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0.fract() == 0.0 && self.0.abs() < 1e15 {
            s.serialize_i64(self.0 as i64)
        } else {
            s.serialize_f64(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Glucose {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        let number = match &value {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        };
        number
            .filter(|f: &f64| f.is_finite())
            .map(Self)
            .ok_or_else(|| {
                serde::de::Error::custom(format!("expected a glucose value, found {value}"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert!((Glucose::mmol(5.5).as_mgdl() - 99.085_745).abs() < 1e-6);
        assert!((Glucose::mgdl(180.0).as_mmol() - 10.0).abs() < 1e-9);
        assert_eq!(Units::parse("mg/dL"), Some(Units::MgDl));
        assert_eq!(Units::parse("mmol/L"), Some(Units::MmolL));
    }

    #[test]
    fn whole_values_serialize_as_integers() {
        assert_eq!(
            serde_json::to_string(&Glucose::mgdl(120.0)).ok().as_deref(),
            Some("120")
        );
        assert_eq!(
            serde_json::to_string(&Glucose::mgdl(120.5)).ok().as_deref(),
            Some("120.5")
        );
        let g: Glucose = serde_json::from_str("\"171\"").unwrap_or_default();
        assert!((g.as_mgdl() - 171.0).abs() < f64::EPSILON);
    }
}
