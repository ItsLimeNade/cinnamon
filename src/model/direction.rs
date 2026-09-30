//! CGM trend directions.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The trend arrow reported with a sensor glucose value.
///
/// Covers every value Nightscout knows; anything else is preserved verbatim in
/// [`Direction::Unknown`] so documents round-trip unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Direction {
    /// No trend available (`NONE`).
    None,
    /// Rising very fast (`TripleUp`).
    TripleUp,
    /// Rising fast (`DoubleUp`).
    DoubleUp,
    /// Rising (`SingleUp`).
    SingleUp,
    /// Rising slowly (`FortyFiveUp`).
    FortyFiveUp,
    /// Steady (`Flat`).
    Flat,
    /// Falling slowly (`FortyFiveDown`).
    FortyFiveDown,
    /// Falling (`SingleDown`).
    SingleDown,
    /// Falling fast (`DoubleDown`).
    DoubleDown,
    /// Falling very fast (`TripleDown`).
    TripleDown,
    /// The uploader could not compute a trend (`NOT COMPUTABLE`).
    NotComputable,
    /// The rate is outside the sensor's range (`RATE OUT OF RANGE`).
    RateOutOfRange,
    /// Any other value, kept as-is.
    Unknown(String),
}

impl Direction {
    /// Parses a direction string; unknown values become [`Direction::Unknown`].
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "NONE" => Self::None,
            "TripleUp" => Self::TripleUp,
            "DoubleUp" => Self::DoubleUp,
            "SingleUp" => Self::SingleUp,
            "FortyFiveUp" => Self::FortyFiveUp,
            "Flat" => Self::Flat,
            "FortyFiveDown" => Self::FortyFiveDown,
            "SingleDown" => Self::SingleDown,
            "DoubleDown" => Self::DoubleDown,
            "TripleDown" => Self::TripleDown,
            "NOT COMPUTABLE" => Self::NotComputable,
            "RATE OUT OF RANGE" => Self::RateOutOfRange,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// The exact string Nightscout stores.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "NONE",
            Self::TripleUp => "TripleUp",
            Self::DoubleUp => "DoubleUp",
            Self::SingleUp => "SingleUp",
            Self::FortyFiveUp => "FortyFiveUp",
            Self::Flat => "Flat",
            Self::FortyFiveDown => "FortyFiveDown",
            Self::SingleDown => "SingleDown",
            Self::DoubleDown => "DoubleDown",
            Self::TripleDown => "TripleDown",
            Self::NotComputable => "NOT COMPUTABLE",
            Self::RateOutOfRange => "RATE OUT OF RANGE",
            Self::Unknown(s) => s,
        }
    }

    /// The arrow Nightscout's UI shows.
    #[must_use]
    pub const fn arrow(&self) -> &'static str {
        match self {
            Self::TripleUp => "⤊",
            Self::DoubleUp => "⇈",
            Self::SingleUp => "↑",
            Self::FortyFiveUp => "↗",
            Self::Flat => "→",
            Self::FortyFiveDown => "↘",
            Self::SingleDown => "↓",
            Self::DoubleDown => "⇊",
            Self::TripleDown => "⤋",
            Self::None => "⇼",
            Self::NotComputable | Self::Unknown(_) => "-",
            Self::RateOutOfRange => "⇕",
        }
    }

    /// The numeric trend code used by Pebble/watch APIs (0 = NONE … 9 = RATE OUT OF RANGE).
    /// Triple arrows map to 8 like Nightscout does.
    #[must_use]
    pub const fn trend_code(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::DoubleUp => 1,
            Self::SingleUp => 2,
            Self::FortyFiveUp => 3,
            Self::Flat => 4,
            Self::FortyFiveDown => 5,
            Self::SingleDown => 6,
            Self::DoubleDown => 7,
            Self::TripleUp | Self::TripleDown | Self::NotComputable | Self::Unknown(_) => 8,
            Self::RateOutOfRange => 9,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Direction {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(|s| Self::parse(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_value() {
        for s in [
            "NONE",
            "TripleUp",
            "DoubleUp",
            "SingleUp",
            "FortyFiveUp",
            "Flat",
            "FortyFiveDown",
            "SingleDown",
            "DoubleDown",
            "TripleDown",
            "NOT COMPUTABLE",
            "RATE OUT OF RANGE",
            "SomethingNew",
        ] {
            assert_eq!(Direction::parse(s).as_str(), s);
        }
        assert_eq!(
            Direction::parse("SomethingNew"),
            Direction::Unknown("SomethingNew".into())
        );
    }
}
