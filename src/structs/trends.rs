use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
pub enum Trend {
    DoubleUp,
    SingleUp,
    FortyFiveUp,
    Flat,
    FortyFiveDown,
    SingleDown,
    DoubleDown,
    #[serde(other)]
    Else,
}

impl Trend {
    pub fn as_arrow(&self) -> &str {
        match self {
            Self::DoubleUp => "↑↑",
            Self::SingleUp => "↑",
            Self::FortyFiveUp => "↗",
            Self::Flat => "→",
            Self::FortyFiveDown => "↘",
            Self::SingleDown => "↓",
            Self::DoubleDown => "↓↓",
            Self::Else => "↮",
        }
    }
}

impl fmt::Display for Trend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_arrow())
    }
}
