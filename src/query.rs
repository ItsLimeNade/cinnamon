//! A typed filter language, encoded for whichever API the request goes through.
//!
//! ```
//! use cinnamon::query::Filter;
//!
//! let filter = Filter::new()
//!     .eq("device", "xDrip-DexcomG6")
//!     .gte("sgv", 180)
//!     .one_of("direction", ["SingleUp", "DoubleUp"]);
//! assert!(!filter.is_empty());
//! ```
//!
//! API v3 receives `field$op=value` parameters; API v1 receives `find[field][$op]=value`.
//! Values are typed so numbers, booleans and times are encoded the way each API expects
//! (for example epoch milliseconds for `date`, ISO-8601 for `created_at`).

use chrono::{DateTime, TimeZone};

use crate::model::Timestamp;

/// A comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Op {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Any of a list.
    In,
    /// None of a list.
    Nin,
    /// Matches a regular expression.
    Regex,
}

impl Op {
    const fn v3(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::In => "in",
            Self::Nin => "nin",
            Self::Regex => "re",
        }
    }

    const fn v1(self) -> &'static str {
        match self {
            Self::Eq => "$eq",
            Self::Ne => "$ne",
            Self::Gt => "$gt",
            Self::Gte => "$gte",
            Self::Lt => "$lt",
            Self::Lte => "$lte",
            Self::In => "$in",
            Self::Nin => "$nin",
            Self::Regex => "$regex",
        }
    }
}

/// A filter value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// A string.
    Text(String),
    /// A number.
    Number(f64),
    /// A boolean.
    Bool(bool),
    /// A point in time; encoded as millis or ISO depending on the field.
    Time(Timestamp),
    /// A list of strings (for [`Op::In`] and [`Op::Nin`]).
    List(Vec<String>),
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&String> for Value {
    fn from(v: &String) -> Self {
        Self::Text(v.clone())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<Timestamp> for Value {
    fn from(v: Timestamp) -> Self {
        Self::Time(v)
    }
}

impl<Tz: TimeZone> From<DateTime<Tz>> for Value {
    fn from(v: DateTime<Tz>) -> Self {
        Self::Time(v.into())
    }
}

macro_rules! numeric_value {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(v: $t) -> Self {
                Self::Number(f64::from(v))
            }
        }
    )*};
}
numeric_value!(f64, f32, i32, u32, i16, u16, i8, u8);

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Number(v as f64)
    }
}

/// One `field op value` condition.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Condition {
    pub(crate) field: String,
    pub(crate) op: Op,
    pub(crate) value: Value,
}

/// A conjunction (AND) of conditions.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct Filter {
    pub(crate) conditions: Vec<Condition>,
}

impl Filter {
    /// An empty filter (matches everything).
    pub const fn new() -> Self {
        Self {
            conditions: Vec::new(),
        }
    }

    /// Adds a condition.
    pub fn with(mut self, field: impl Into<String>, op: Op, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition {
            field: field.into(),
            op,
            value: value.into(),
        });
        self
    }

    /// `field == value`.
    pub fn eq(self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(field, Op::Eq, value)
    }

    /// `field != value`.
    pub fn ne(self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(field, Op::Ne, value)
    }

    /// `field > value`.
    pub fn gt(self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(field, Op::Gt, value)
    }

    /// `field >= value`.
    pub fn gte(self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(field, Op::Gte, value)
    }

    /// `field < value`.
    pub fn lt(self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(field, Op::Lt, value)
    }

    /// `field <= value`.
    pub fn lte(self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(field, Op::Lte, value)
    }

    /// `field` is one of `values`.
    pub fn one_of<I, S>(self, field: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let list = values.into_iter().map(Into::into).collect();
        self.with(field, Op::In, Value::List(list))
    }

    /// `field` is none of `values`.
    pub fn none_of<I, S>(self, field: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let list = values.into_iter().map(Into::into).collect();
        self.with(field, Op::Nin, Value::List(list))
    }

    /// `field` matches the regular expression `pattern` (case-sensitive).
    pub fn matches(self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.with(field, Op::Regex, Value::Text(pattern.into()))
    }

    /// Adds every condition of `other`.
    pub fn and(mut self, other: Self) -> Self {
        self.conditions.extend(other.conditions);
        self
    }

    /// Whether the filter has no conditions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    /// Whether any condition constrains `field`.
    pub(crate) fn mentions(&self, field: &str) -> bool {
        self.conditions.iter().any(|c| c.field == field)
    }

    /// API v3 query parameters: `field$op=value`.
    pub(crate) fn v3_pairs(&self) -> Vec<(String, String)> {
        self.conditions
            .iter()
            .map(|c| {
                let value = match &c.value {
                    Value::Text(s) if c.op != Op::Regex && needs_v3_quotes(s) => format!("'{s}'"),
                    Value::List(items) => items.join("|"),
                    other => encode_scalar(&c.field, other),
                };
                (format!("{}${}", c.field, c.op.v3()), value)
            })
            .collect()
    }

    /// API v1 query parameters: `find[field][$op]=value`.
    pub(crate) fn v1_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for c in &self.conditions {
            match (&c.op, &c.value) {
                (Op::In | Op::Nin, Value::List(items)) => {
                    let key = format!("find[{}][{}][]", c.field, c.op.v1());
                    pairs.extend(items.iter().map(|item| (key.clone(), item.clone())));
                }
                (Op::Eq, value) => {
                    pairs.push((format!("find[{}]", c.field), encode_scalar(&c.field, value)));
                }
                (op, value) => pairs.push((
                    format!("find[{}][{}]", c.field, op.v1()),
                    encode_scalar(&c.field, value),
                )),
            }
        }
        pairs
    }
}

/// Fields Nightscout stores as epoch milliseconds; every other time field is an ISO string.
fn is_millis_field(field: &str) -> bool {
    matches!(field, "date" | "mills" | "srvModified" | "srvCreated")
}

fn encode_scalar(field: &str, value: &Value) -> String {
    match value {
        Value::Text(s) => s.clone(),
        Value::Number(n) => format_number(*n),
        Value::Bool(b) => b.to_string(),
        Value::Time(t) if is_millis_field(field) => t.as_millis().to_string(),
        Value::Time(t) => t.to_iso(),
        Value::List(items) => items.join("|"),
    }
}

pub(crate) fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
}

/// API v3 converts numeric-looking and boolean-looking strings; quoting keeps them strings.
fn needs_v3_quotes(s: &str) -> bool {
    s.trim().parse::<f64>().is_ok() || matches!(s, "true" | "false")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_encoding() {
        let f = Filter::new()
            .eq("device", "xDrip")
            .eq("sgv", "171")
            .gte("date", Timestamp::from_millis(1000))
            .lt("created_at", Timestamp::from_millis(1_714_564_800_000))
            .one_of("type", ["sgv", "mbg"])
            .matches("notes", "^pizza");
        assert_eq!(
            f.v3_pairs(),
            vec![
                ("device$eq".into(), "xDrip".into()),
                ("sgv$eq".into(), "'171'".into()),
                ("date$gte".into(), "1000".into()),
                ("created_at$lt".into(), "2024-05-01T12:00:00.000Z".into()),
                ("type$in".into(), "sgv|mbg".into()),
                ("notes$re".into(), "^pizza".into()),
            ]
        );
    }

    #[test]
    fn v1_encoding() {
        let f = Filter::new()
            .eq("type", "sgv")
            .gte("sgv", 180)
            .none_of("device", ["a", "b"]);
        assert_eq!(
            f.v1_pairs(),
            vec![
                ("find[type]".into(), "sgv".into()),
                ("find[sgv][$gte]".into(), "180".into()),
                ("find[device][$nin][]".into(), "a".into()),
                ("find[device][$nin][]".into(), "b".into()),
            ]
        );
    }
}
